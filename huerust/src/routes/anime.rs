/*
 * HUE ENGINE ❄
 * System Designation: The Architecture of Aggregation
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use tracing::{info, warn};
use utoipa::ToSchema;

use crate::app::AnimeState;
#[allow(unused_imports)]
use crate::models::anime::{
    AnimeCard, AnimeDetail, AnimeHomeFeed, AnimeSearchResponse, EpisodeListResponse, FilterParams,
    PaginatedResponse, ScheduleEntry, ServerList, SkipRange, StreamPayload, SubtitleTrack,
};
use crate::proxy::{dynamic_proxy_base, proxy_anime_stream};

#[derive(Deserialize, ToSchema)]
pub struct SearchQuery {
    pub q: String,
    pub page: Option<u32>,
    pub limit: Option<u32>,
}

#[derive(Deserialize, ToSchema)]
pub struct CategoryQuery {
    pub r#type: Option<String>,
    pub page: Option<u32>,
    pub limit: Option<u32>,
}

#[derive(Deserialize, ToSchema)]
pub struct ServersQuery {
    pub id: String,
    pub episode: Option<u32>,
    pub r#type: Option<String>,
    /// Force a specific provider source. Use `animexin` for Chinese donghua.
    pub source: Option<String>,
}

#[derive(Deserialize, ToSchema)]
pub struct StreamQuery {
    pub id: String,
    pub server: Option<String>,
    pub r#type: Option<String>,
    pub episode: Option<u32>,
    /// Force a specific provider source. Use `animexin` for Chinese donghua.
    pub source: Option<String>,
}

#[derive(Deserialize, ToSchema)]
pub struct EpisodesQuery {
    pub start: Option<u32>,
    /// Force a specific provider source. Use `animexin` for Chinese donghua.
    pub source: Option<String>,
}

#[derive(Deserialize, ToSchema)]
pub struct DetailQuery {
    /// Force a specific provider source. Use `animexin` for Chinese donghua.
    pub source: Option<String>,
}

#[derive(Deserialize, ToSchema)]
pub struct HomeQuery {
    /// Force a specific provider source. Use `animexin` for Chinese donghua catalogs.
    pub source: Option<String>,
}

#[derive(serde::Serialize, ToSchema)]
struct ApiResponse<T: serde::Serialize> {
    success: bool,
    data: T,
}

#[derive(serde::Serialize, ToSchema)]
struct ErrorResponse {
    success: bool,
    error: String,
}

fn success_json(data: impl serde::Serialize) -> Response {
    (
        StatusCode::OK,
        axum::Json(ApiResponse {
            success: true,
            data,
        }),
    )
        .into_response()
}

fn error_json(status: StatusCode, error: &str) -> Response {
    (
        status,
        axum::Json(ErrorResponse {
            success: false,
            error: error.to_string(),
        }),
    )
        .into_response()
}

fn proxy_stream_payload(
    proxy_base_url: &str,
    secret: &str,
    enable_proxy: bool,
    payload: StreamPayload,
) -> StreamPayload {
    // Thin shim — actual logic lives in `proxy::proxy_anime_stream`.
    proxy_anime_stream(proxy_base_url, secret, enable_proxy, payload)
}

#[utoipa::path(
    get,
    path = "/api/anime/search",
    params(
        ("q" = String, Query, description = "Search keyword"),
        ("page" = Option<u32>, Query, description = "Page number"),
        ("limit" = Option<u32>, Query, description = "Items per page"),
    ),
    responses(
        (status = 200, description = "Search results", body = AnimeSearchResponse),
        (status = 400, description = "Missing query"),
        (status = 500, description = "Internal error"),
    ),
)]
pub async fn search_handler(
    State(state): State<AnimeState>,
    Query(params): Query<SearchQuery>,
) -> Response {
    let page = params.page.unwrap_or(1).max(1);
    let limit = params.limit.unwrap_or(50).clamp(1, 50);
    info!("[anime] search: {} page={} limit={}", params.q, page, limit);
    let response = state.anime_agg.search(&params.q, page, limit).await;
    success_json(response)
}

#[utoipa::path(
    get,
    path = "/api/anime/latest",
    responses(
        (status = 200, description = "Latest anime list", body = Vec<AnimeCard>),
        (status = 500, description = "Internal error"),
    ),
)]
pub async fn latest_handler(State(state): State<AnimeState>) -> Response {
    warn!("[anime] /api/anime/latest is deprecated; use /api/anime/discover?sort=latest");
    let fp = FilterParams {
        sort: Some("latest".into()),
        page: Some(1),
        limit: Some(20),
        ..Default::default()
    };
    do_discover(&state, fp).await
}

#[utoipa::path(
    get,
    path = "/api/anime/home",
    params(
        ("source" = Option<String>, Query, description = "Provider source override, e.g. animexin"),
    ),
    responses(
        (status = 200, description = "Home feed", body = AnimeHomeFeed),
        (status = 500, description = "Internal error"),
    ),
)]
pub async fn home_handler(
    State(state): State<AnimeState>,
    Query(params): Query<HomeQuery>,
) -> Response {
    info!("[anime] home source={:?}", params.source);
    let feed = state.anime_agg.get_home(params.source.as_deref()).await;
    success_json(feed)
}

#[utoipa::path(
    get,
    path = "/api/anime/detail/{slug}",
    params(
        ("slug" = String, Path, description = "Anime slug or hex ID"),
        ("source" = Option<String>, Query, description = "Provider source override, e.g. animexin"),
    ),
    responses(
        (status = 200, description = "Anime detail", body = AnimeDetail),
        (status = 404, description = "Anime not found"),
        (status = 500, description = "Internal error"),
    ),
)]
pub async fn detail_handler(
    State(state): State<AnimeState>,
    Path(slug): Path<String>,
    Query(params): Query<DetailQuery>,
) -> Response {
    info!("[anime] detail: {} source={:?}", slug, params.source);
    match state
        .anime_agg
        .get_detail(&slug, params.source.as_deref())
        .await
    {
        Some(detail) => {
            if detail.card.id.is_empty() {
                return error_json(StatusCode::NOT_FOUND, "Anime not found");
            }
            success_json(detail)
        }
        None => error_json(StatusCode::NOT_FOUND, "Anime not found"),
    }
}

#[utoipa::path(
    get,
    path = "/api/anime/episodes/{id}",
    params(
        ("id" = String, Path, description = "Anime hex ID"),
        ("start" = Option<u32>, Query, description = "Start episode number for pagination"),
        ("source" = Option<String>, Query, description = "Provider source override, e.g. animexin"),
    ),
    responses(
        (status = 200, description = "Episode list", body = EpisodeListResponse),
        (status = 404, description = "Anime not found"),
        (status = 500, description = "Internal error"),
    ),
)]
pub async fn episodes_handler(
    State(state): State<AnimeState>,
    Path(id): Path<String>,
    Query(params): Query<EpisodesQuery>,
) -> Response {
    let start = params.start.unwrap_or(0);
    info!(
        "[anime] episodes: {} start={} source={:?}",
        id, start, params.source
    );
    match state
        .anime_agg
        .episodes_response(&id, start, params.source.as_deref())
        .await
    {
        Some(resp) => success_json(resp),
        None => {
            warn!("[anime] episodes not found: {}", id);
            error_json(StatusCode::NOT_FOUND, "Episodes not found")
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/anime/servers",
    params(
        ("id" = String, Query, description = "Anime episode slug"),
        ("episode" = Option<u32>, Query, description = "Episode number"),
        ("type" = Option<String>, Query, description = "Audio type: sub or dub"),
        ("source" = Option<String>, Query, description = "Provider source override, e.g. animexin"),
    ),
    responses(
        (status = 200, description = "Server list", body = ServerList),
        (status = 400, description = "Missing ID"),
        (status = 500, description = "Internal error"),
    ),
)]
pub async fn servers_handler(
    State(state): State<AnimeState>,
    Query(params): Query<ServersQuery>,
) -> Response {
    let episode = params.episode.unwrap_or(1);
    let link_type = params.r#type.as_deref();
    info!(
        "[anime] servers for: {} ep={} type={:?} source={:?}",
        params.id, episode, link_type, params.source
    );
    match state
        .anime_agg
        .get_servers(
            &params.id,
            Some(episode),
            link_type,
            params.source.as_deref(),
        )
        .await
    {
        Some(servers) => success_json(servers),
        None => {
            warn!("[anime] servers not found: {}", params.id);
            error_json(StatusCode::NOT_FOUND, "Servers not found")
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/anime/stream",
    params(
        ("id" = String, Query, description = "Anime hex ID or episode slug"),
        ("server" = Option<String>, Query, description = "Target server name (e.g., HD-1)"),
        ("type" = Option<String>, Query, description = "Audio type: sub or dub"),
        ("episode" = Option<u32>, Query, description = "Episode number"),
        ("source" = Option<String>, Query, description = "Provider source override, e.g. animexin"),
    ),
    responses(
        (status = 200, description = "Stream data", body = StreamPayload),
        (status = 400, description = "Missing ID"),
        (status = 404, description = "Stream not found"),
        (status = 500, description = "Internal error"),
    ),
)]
pub async fn stream_handler(
    State(state): State<AnimeState>,
    headers: HeaderMap,
    Query(params): Query<StreamQuery>,
) -> Response {
    let episode = params.episode.unwrap_or(1);
    let link_type = params.r#type.as_deref().unwrap_or("sub");
    info!(
        "[anime] stream: {} ep={} type={} source={:?}",
        params.id, episode, link_type, params.source
    );

    let proxy_base = dynamic_proxy_base(&headers, &state.proxy_base_url);
    match state
        .anime_agg
        .get_stream(
            &params.id,
            params.server.as_deref(),
            link_type,
            episode,
            params.source.as_deref(),
        )
        .await
    {
        Some(payload) => {
            let proxied = proxy_stream_payload(
                &proxy_base,
                &state.secret,
                state.config.enable_proxy,
                payload,
            );
            success_json(proxied)
        }
        None => {
            warn!("[anime] stream not found: {}", params.id);
            error_json(StatusCode::NOT_FOUND, "Stream not found")
        }
    }
}

use axum::extract::Json;

#[derive(Deserialize)]
pub struct LegacyFilterBody {
    #[serde(flatten)]
    pub extra: std::collections::HashMap<String, serde_json::Value>,
}

/// Discover anime with a typed filter. Backed by aggregator fan-out.
#[utoipa::path(
    get,
    path = "/api/anime/discover",
    params(
        ("type" = Option<String>, Query, description = "TV, movie, ova, ona, special, music"),
        ("genre" = Option<String>, Query, description = "Single genre slug (e.g. Action, Sci-Fi)"),
        ("status" = Option<String>, Query, description = "airing | finished | upcoming"),
        ("year" = Option<u32>, Query, description = "Release year"),
        ("season" = Option<String>, Query, description = "winter | spring | summer | fall"),
        ("sort" = Option<String>, Query, description = "popularity | score | trending | latest | alphabetical"),
        ("language" = Option<String>, Query, description = "japanese | english | chinese | korean"),
        ("country" = Option<String>, Query, description = "CN | JP | KR | US"),
        ("score" = Option<String>, Query, description = "1-10 or all"),
        ("rating" = Option<String>, Query, description = "G | PG | PG-13 | R | R+ | Rx"),
        ("page" = Option<u32>, Query, description = "Page number"),
        ("limit" = Option<u32>, Query, description = "Items per page"),
        ("source" = Option<String>, Query, description = "Provider source override, e.g. animexin"),
    ),
    responses(
        (status = 200, description = "Discover results", body = PaginatedResponse<AnimeCard>),
        (status = 400, description = "Invalid filter parameters"),
        (status = 500, description = "Internal error"),
    ),
)]
pub async fn discover_get_handler(
    State(state): State<AnimeState>,
    Query(params): Query<FilterParams>,
) -> Response {
    info!("[anime] discover GET: {:?}", params);
    do_discover(&state, params).await
}

#[utoipa::path(
    post,
    path = "/api/anime/discover",
    request_body = FilterParams,
    responses(
        (status = 200, description = "Discover results", body = PaginatedResponse<AnimeCard>),
        (status = 400, description = "Invalid filter parameters"),
        (status = 500, description = "Internal error"),
    ),
)]
pub async fn discover_post_handler(
    State(state): State<AnimeState>,
    Json(params): Json<FilterParams>,
) -> Response {
    info!("[anime] discover POST: {:?}", params);
    do_discover(&state, params).await
}

async fn do_discover(state: &AnimeState, params: FilterParams) -> Response {
    let resp = state.anime_agg.discover(&params).await;
    success_json(resp)
}

/// Static metadata for the discover screen.
#[utoipa::path(
    get,
    path = "/api/anime/filters",
    responses(
        (status = 200, description = "Available filter options (genres, years, statuses, etc.)"),
        (status = 500, description = "Internal error"),
    ),
)]
pub async fn filter_options_handler(State(state): State<AnimeState>) -> Response {
    success_json(state.anime_agg.filter_options().await)
}

#[derive(Deserialize)]
pub struct ScheduleQuery {
    /// YYYY-MM-DD. Defaults to today (UTC).
    pub date: Option<String>,
    /// IANA timezone, e.g. "America/Los_Angeles". Currently informational;
    /// AniList returns UTC timestamps and the client should localize.
    pub tz: Option<String>,
}

#[utoipa::path(
    get,
    path = "/api/anime/schedule",
    params(
        ("date" = Option<String>, Query, description = "YYYY-MM-DD. Defaults to today (UTC)"),
        ("tz" = Option<String>, Query, description = "IANA timezone, e.g. America/Los_Angeles (informational)"),
    ),
    responses(
        (status = 200, description = "Daily schedule entries", body = Vec<ScheduleEntry>),
        (status = 500, description = "Internal error"),
    ),
)]
pub async fn schedule_handler(
    State(state): State<AnimeState>,
    Query(params): Query<ScheduleQuery>,
) -> Response {
    let date = params
        .date
        .as_deref()
        .and_then(|d| chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").ok());
    let entries = state.anime_agg.schedule(date, params.tz.as_deref()).await;
    success_json(entries)
}

// ── Deprecated aliases (kept for backward compat) ───────────────────────────

#[utoipa::path(
    get,
    path = "/api/anime/category",
    params(
        ("type" = Option<String>, Query, description = "TV, movie, ova, ona, special, music"),
        ("page" = Option<u32>, Query, description = "Page number"),
        ("limit" = Option<u32>, Query, description = "Items per page"),
    ),
    responses(
        (status = 200, description = "Deprecated — use /api/anime/discover", body = PaginatedResponse<AnimeCard>),
        (status = 500, description = "Internal error"),
    ),
)]
pub async fn category_handler(
    State(state): State<AnimeState>,
    Query(params): Query<CategoryQuery>,
) -> Response {
    warn!("[anime] /api/anime/category is deprecated; use /api/anime/discover");
    let fp = FilterParams {
        r#type: params.r#type,
        page: params.page,
        limit: params.limit,
        ..Default::default()
    };
    do_discover(&state, fp).await
}

#[utoipa::path(
    post,
    path = "/api/anime/filter",
    request_body = LegacyFilterBody,
    responses(
        (status = 200, description = "Deprecated — use POST /api/anime/discover", body = PaginatedResponse<AnimeCard>),
        (status = 500, description = "Internal error"),
    ),
)]
pub async fn filter_handler(
    State(state): State<AnimeState>,
    Json(body): Json<LegacyFilterBody>,
) -> Response {
    warn!("[anime] POST /api/anime/filter is deprecated; use POST /api/anime/discover");
    // Map legacy keys (genres[], type, status, sort, page, limit) onto the
    // typed struct. Unknown keys are ignored.
    let fp = FilterParams {
        r#type: body
            .extra
            .get("type")
            .and_then(|v| v.as_str())
            .map(String::from),
        genre: body
            .extra
            .get("genre")
            .and_then(|v| v.as_str())
            .map(String::from)
            .or_else(|| {
                body.extra
                    .get("genres")
                    .and_then(|v| v.as_array())
                    .and_then(|a| a.first().and_then(|g| g.as_str().map(String::from)))
            }),
        status: body
            .extra
            .get("status")
            .and_then(|v| v.as_str())
            .map(String::from),
        sort: body
            .extra
            .get("sort")
            .and_then(|v| v.as_str())
            .map(String::from),
        year: body
            .extra
            .get("year")
            .and_then(|v| v.as_u64())
            .map(|y| y as u32),
        page: body
            .extra
            .get("page")
            .and_then(|v| v.as_u64())
            .map(|p| p as u32),
        limit: body
            .extra
            .get("limit")
            .and_then(|v| v.as_u64())
            .map(|l| l as u32),
        ..Default::default()
    };
    do_discover(&state, fp).await
}
