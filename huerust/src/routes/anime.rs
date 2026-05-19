/*
 * HUE ENGINE ❄
 * System Designation: The Architecture of Aggregation
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */

use axum::{
    extract::{Query, State, Path},
    response::{IntoResponse, Response},
    http::StatusCode,
};
use serde::Deserialize;
use tracing::{info, warn};
use utoipa::ToSchema;

use crate::app::AnimeState;
#[allow(unused_imports)]
use crate::models::anime::{
    AnimeCard, AnimeDetail, AnimeHomeFeed, AnimeSearchResponse,
    ServerList, StreamPayload, SkipRange, SubtitleTrack,
};

#[derive(Deserialize, ToSchema)]
pub struct SearchQuery {
    pub q: String,
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
}

#[derive(Deserialize, ToSchema)]
pub struct StreamQuery {
    pub id: String,
    pub server: Option<String>,
    pub r#type: Option<String>,
    pub episode: Option<u32>,
}

#[derive(Deserialize, ToSchema)]
pub struct EpisodesQuery {
    pub start: Option<u32>,
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

fn hianime_stream_to_payload(
    proxy_base_url: &str,
    secret: &str,
    enable_proxy: bool,
    data: crate::clients::hianime::StreamData,
) -> StreamPayload {
    let proxied = if enable_proxy {
        let mut d = data;
        let referer = d.referer.as_deref().unwrap_or("https://megaplay.buzz/");
        let mut headers = std::collections::HashMap::new();
        headers.insert("Referer".to_string(), referer.to_string());

        let route_for = |url: &str, kind: &str| -> String {
            let normalized = url.to_lowercase();
            if kind == "subtitle" { "/sub-proxy".to_string() }
            else if normalized.contains(".m3u8") { "/m3u8-proxy".to_string() }
            else if normalized.contains(".ts") || normalized.contains(".mp4") || normalized.contains(".m4s") || normalized.contains(".mkv") { "/ts-proxy".to_string() }
            else { "/api/proxy".to_string() }
        };

        d.link.file = {
            let route = route_for(&d.link.file, "stream");
            let token = crate::security::encrypt_url(&d.link.file, secret, Some(headers.clone()));
            format!("{}{}?h_token={}", proxy_base_url, route, urlencoding::encode(&token))
        };

        if let Some(ref mut tracks) = d.tracks {
            for t in tracks.iter_mut() {
                let route = route_for(&t.file, "subtitle");
                let token = crate::security::encrypt_url(&t.file, secret, Some(headers.clone()));
                t.file = format!("{}{}?h_token={}", proxy_base_url, route, urlencoding::encode(&token));
            }
        }
        d
    } else {
        data
    };

    StreamPayload {
        url: proxied.link.file,
        stream_type: proxied.link.link_type,
        subtitles: proxied.tracks.map(|t| t.into_iter().map(|tr| SubtitleTrack {
            file: tr.file,
            label: tr.label,
            kind: tr.kind,
            default: tr.default,
        }).collect()),
        intro: proxied.intro.map(|i| SkipRange { start: i.start, end: i.end }),
        outro: proxied.outro.map(|o| SkipRange { start: o.start, end: o.end }),
        server: proxied.server,
        referer: proxied.referer,
    }
}

#[utoipa::path(
    get,
    path = "/api/anime/search",
    params(
        ("q" = String, Query, description = "Search keyword"),
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
    info!("[anime] search: {}", params.q);
    match state.hianime.search(&params.q).await {
        Ok(raw) => {
            let structured = crate::clients::hianime::transform_search_results(raw, &params.q);
            success_json(structured)
        }
        Err(e) => {
            warn!("[anime] search error: {}", e);
            error_json(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string())
        }
    }
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
    match state.hianime.get_latest(1).await {
        Ok(raw) => {
            let structured = crate::clients::hianime::transform_category_results(raw);
            success_json(structured)
        }
        Err(e) => {
            warn!("[anime] latest error: {}", e);
            error_json(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string())
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/anime/category",
    params(
        ("type" = Option<String>, Query, description = "Content type: tv, movie, ova, ona"),
        ("page" = Option<u32>, Query, description = "Page number"),
        ("limit" = Option<u32>, Query, description = "Items per page"),
    ),
    responses(
        (status = 200, description = "Category items", body = Vec<AnimeCard>),
        (status = 500, description = "Internal error"),
    ),
)]
pub async fn category_handler(
    State(state): State<AnimeState>,
    Query(params): Query<CategoryQuery>,
) -> Response {
    let typ = params.r#type.as_deref().unwrap_or("tv");
    let page = params.page.unwrap_or(1);
    let limit = params.limit.unwrap_or(20);
    match state.hianime.get_by_category(typ, page, limit).await {
        Ok(raw) => {
            let structured = crate::clients::hianime::transform_category_results(raw);
            success_json(structured)
        }
        Err(e) => {
            warn!("[anime] category error: {}", e);
            error_json(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string())
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/anime/home",
    responses(
        (status = 200, description = "Home feed data", body = AnimeHomeFeed),
        (status = 500, description = "Internal error"),
    ),
)]
pub async fn home_handler(State(state): State<AnimeState>) -> Response {
    match state.hianime.get_home().await {
        Ok(raw) => {
            let structured = crate::clients::hianime::transform_home_feed(raw);
            success_json(structured)
        }
        Err(e) => {
            warn!("[anime] home error: {}", e);
            error_json(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string())
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/anime/detail/{slug}",
    params(
        ("slug" = String, Path, description = "Anime slug or hex ID"),
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
) -> Response {
    info!("[anime] detail: {}", slug);
    match state.hianime.get_anime_info(&slug).await {
        Ok(raw) => {
            let detail = crate::clients::hianime::transform_anime_detail(raw);
            if detail.card.id.is_empty() {
                return error_json(StatusCode::NOT_FOUND, "Anime not found");
            }
            success_json(detail)
        }
        Err(e) => {
            warn!("[anime] detail error: {}", e);
            error_json(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string())
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/anime/episodes/{id}",
    params(
        ("id" = String, Path, description = "Anime hex ID"),
        ("start" = Option<u32>, Query, description = "Start episode number for pagination"),
    ),
    responses(
        (status = 200, description = "Episode list", body = AnimeDetail),
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
    info!("[anime] episodes: {} start={}", id, start);
    match state.hianime.get_episodes(&id, start).await {
        Ok(ep_data) => {
            // Build a minimal detail response with just the episodes
            let episodes: Vec<crate::models::anime::EpisodeSummary> = ep_data.episodes
                .unwrap_or_default()
                .into_iter()
                .map(|ep| crate::models::anime::EpisodeSummary {
                    id: ep._id,
                    number: ep.episodeNumber.unwrap_or(0),
                    title: None,
                    sub_count: ep.link.as_ref().and_then(|l| l.sub.as_ref()).map(|v| v.len() as u32).unwrap_or(0),
                    dub_count: ep.link.as_ref().and_then(|l| l.dub.as_ref()).map(|v| v.len() as u32).unwrap_or(0),
                    slug: ep.slugs.as_ref().and_then(|s| s.first().cloned()),
                    slugs: ep.slugs,
                })
                .collect();
            success_json(serde_json::json!({
                "animeId": id,
                "start": start,
                "count": episodes.len(),
                "episodes": episodes,
            }))
        }
        Err(e) => {
            warn!("[anime] episodes error: {}", e);
            error_json(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string())
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/anime/servers",
    params(
        ("id" = String, Query, description = "Anime episode slug"),
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
    info!("[anime] servers for: {}", params.id);
    match state.hianime.get_servers(&params.id).await {
        Ok(raw) => {
            let structured = crate::clients::hianime::transform_server_list(&raw);
            success_json(structured)
        }
        Err(e) => {
            warn!("[anime] servers error: {}", e);
            error_json(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string())
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
    Query(params): Query<StreamQuery>,
) -> Response {
    let episode = params.episode.unwrap_or(1);
    let link_type = params.r#type.as_deref().unwrap_or("sub");
    info!("[anime] stream: {} ep={} type={}", params.id, episode, link_type);

    match state
        .hianime
        .get_stream(&params.id,
            params.server.as_deref(),
            link_type,
            episode,
        )
        .await
    {
        Ok(data) => {
            let payload = hianime_stream_to_payload(
                &state.proxy_base_url,
                &state.secret,
                state.config.enable_proxy,
                data,
            );
            success_json(payload)
        }
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("SERVERS_NOT_FOUND") || msg.contains("SERVER_NOT_FOUND") || msg.contains("EXTRACTION_FAILED") {
                return error_json(StatusCode::NOT_FOUND, &msg);
            }
            warn!("[anime] stream error: {}", e);
            error_json(StatusCode::INTERNAL_SERVER_ERROR, &msg)
        }
    }
}

use axum::extract::Json;

#[derive(Deserialize)]
pub struct FilterBody {
    #[serde(flatten)]
    pub extra: std::collections::HashMap<String, serde_json::Value>,
}

pub async fn filter_handler(
    State(state): State<AnimeState>,
    Json(body): Json<FilterBody>,
) -> Response {
    info!("[anime] filter: {:?}", body.extra);
    let payload = serde_json::to_value(&body.extra).unwrap_or_default();
    match state.hianime.filter(payload).await {
        Ok(raw) => success_json(raw),
        Err(e) => {
            warn!("[anime] filter error: {}", e);
            error_json(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string())
        }
    }
}
