//! Hue Stream API
//! Professional Streaming Aggregator
//!
//! @author roywalk3r <roywalker440@gmail.com>
//! @repository https://github.com/roywalk3r/hue.git
//! @license MIT

use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use serde::Deserialize;

use crate::app::kisskh_enabled;
use crate::app::DramaState;

use crate::proxy::dynamic_proxy_base;
use crate::proxy::wrap::wrap_with_proxy;
use crate::security::encrypt_stream_result_urls;

fn kisskh_disabled_response() -> Response {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        r#"{"success":false,"error":"KissKH is disabled"}"#,
    )
        .into_response()
}

fn moviebox_episode_number(ep: &serde_json::Value, fallback: u64) -> u64 {
    ep["number"]
        .as_u64()
        .or_else(|| ep["episodeNumber"].as_u64())
        .or_else(|| ep["ep"].as_u64())
        .or_else(|| ep["idx"].as_u64())
        .or_else(|| ep["index"].as_u64())
        .unwrap_or(fallback)
}

fn moviebox_episode_title(ep: &serde_json::Value, number: u64) -> String {
    ep["title"]
        .as_str()
        .or_else(|| ep["name"].as_str())
        .or_else(|| ep["displayName"].as_str())
        .map(str::to_string)
        .unwrap_or_else(|| format!("Episode {}", number))
}

fn moviebox_detail_episodes(detail_data: &serde_json::Value) -> Vec<serde_json::Value> {
    let inner = &detail_data["data"];
    let episode_arr = inner["episodes"]
        .as_array()
        .or_else(|| inner["episodeList"].as_array())
        .or_else(|| inner["episodeVoList"].as_array())
        .or_else(|| detail_data["episodes"].as_array())
        .or_else(|| detail_data["episodeList"].as_array());

    if let Some(arr) = episode_arr {
        let mut seen = std::collections::HashSet::new();
        return arr
            .iter()
            .enumerate()
            .filter_map(|(idx, ep)| {
                let number = moviebox_episode_number(ep, idx as u64 + 1);
                if !seen.insert(number) {
                    return None;
                }
                let id = ep["id"]
                    .as_str()
                    .or_else(|| ep["episodeId"].as_str())
                    .map(str::to_string)
                    .unwrap_or_else(|| number.to_string());
                Some(serde_json::json!({
                    "id": id,
                    "number": number,
                    "title": moviebox_episode_title(ep, number)
                }))
            })
            .collect();
    }

    let seasons = inner["resource"]["seasons"]
        .as_array()
        .or_else(|| inner["seasonList"].as_array())
        .or_else(|| detail_data["resource"]["seasons"].as_array());

    let Some(season) = seasons.and_then(|items| {
        items
            .iter()
            .find(|season| season["se"].as_u64().unwrap_or(1) == 1)
            .or_else(|| items.first())
    }) else {
        return Vec::new();
    };

    if let Some(episodes) = season["episodes"].as_array() {
        return episodes
            .iter()
            .enumerate()
            .map(|(idx, ep)| {
                let number = moviebox_episode_number(ep, idx as u64 + 1);
                let id = ep["id"]
                    .as_str()
                    .or_else(|| ep["episodeId"].as_str())
                    .map(str::to_string)
                    .unwrap_or_else(|| number.to_string());
                serde_json::json!({
                    "id": id,
                    "number": number,
                    "title": moviebox_episode_title(ep, number)
                })
            })
            .collect();
    }

    let max_ep = season["maxEp"]
        .as_u64()
        .or_else(|| season["episodeCount"].as_u64())
        .or_else(|| season["epNum"].as_u64())
        .unwrap_or(0);

    (1..=max_ep)
        .map(|number| {
            serde_json::json!({
                "id": number.to_string(),
                "number": number,
                "title": format!("Episode {}", number)
            })
        })
        .collect()
}

#[derive(Deserialize)]
pub struct SearchQuery {
    pub q: String,
}

#[derive(Deserialize)]
pub struct KissKhBrowseQuery {
    pub page: Option<u32>,
    pub page_size: Option<u32>,
    pub content_type: Option<u32>,
    pub sub: Option<u32>,
    pub country: Option<u32>,
    pub status: Option<u32>,
    pub order: Option<u32>,
}

#[derive(Deserialize)]
pub struct MovieBoxBrowseQuery {
    pub page: Option<u32>,
    pub per_page: Option<u32>,
    pub country: Option<String>,
    pub sort: Option<String>,
    pub channel_id: Option<u32>,
}

#[derive(Deserialize)]
pub struct MovieBoxSearchQuery {
    pub q: String,
    pub page: Option<u32>,
    pub subject_type: Option<u32>,
}

#[derive(Deserialize)]
pub struct MovieBoxDirectStreamQuery {
    pub detail_path: String,
    pub episode: Option<u32>,
    pub subject_id: Option<String>,
    pub media_type: Option<String>,
}

#[derive(Deserialize)]
pub struct DetailQuery {
    pub q: String,
    pub id: Option<String>,
}

#[derive(Deserialize)]
pub struct DetailRecQuery {
    pub id: String,
    pub page: Option<u32>,
    pub per_page: Option<u32>,
}

// ── KissKH ─────────────────────────────────────────────────────────

#[utoipa::path(
    get,
    path = "/api/drama/kisskh/search",
    params(
        ("q" = String, Query, description = "Search keyword"),
    ),
    responses(
        (status = 200, description = "Search results", body = serde_json::Value),
        (status = 500, description = "Internal error"),
    ),
)]
pub async fn kisskh_search_handler(
    State(state): State<DramaState>,
    Query(params): Query<SearchQuery>,
) -> Response {
    if !kisskh_enabled() {
        return kisskh_disabled_response();
    }
    tracing::info!("[drama] kisskh_search: q={}", params.q);
    match state.kisskh.search(&params.q).await {
        Ok(items) => {
            tracing::info!(
                "[drama] kisskh_search: {} results for q={}",
                items.len(),
                params.q
            );
            (StatusCode::OK, Json(items)).into_response()
        }
        Err(e) => {
            tracing::warn!("[drama] kisskh_search error for '{}': {}", params.q, e);
            let body = serde_json::json!({ "success": false, "error": e.to_string() });
            (StatusCode::INTERNAL_SERVER_ERROR, body.to_string()).into_response()
        }
    }
}

#[derive(Deserialize)]
pub struct KissKhStreamsQuery {
    pub title: Option<String>,
}

#[utoipa::path(
    get,
    path = "/api/drama/kisskh/streams/{id}/{episode}",
    params(
        ("id" = String, Path, description = "KissKH show id (numeric) or title slug"),
        ("episode" = u32, Path, description = "Episode number"),
        ("title" = Option<String>, Query, description = "Optional title fallback when id is non-numeric"),
    ),
    responses(
        (status = 200, description = "Stream results (possibly empty array)", body = serde_json::Value),
    ),
)]
pub async fn kisskh_streams_handler(
    State(state): State<DramaState>,
    headers: HeaderMap,
    Path((id, episode)): Path<(String, u32)>,
    Query(params): Query<KissKhStreamsQuery>,
) -> Response {
    if !kisskh_enabled() {
        return kisskh_disabled_response();
    }
    // Prefer numeric ID from path; only fall back to title search when ID is not numeric.
    let lookup = if id.parse::<u64>().is_ok() {
        id.as_str()
    } else {
        params.title.as_ref().map(|t| t.as_str()).unwrap_or(&id)
    };

    let proxy_base = dynamic_proxy_base(&headers, &state.proxy_base_url);
    match state.kisskh.get_streams_direct(lookup, episode).await {
        Ok(streams) if !streams.is_empty() => {
            let mut proxied = wrap_with_proxy(streams, &proxy_base, &state.secret);
            encrypt_stream_result_urls(&mut proxied, &state.kkey);
            (StatusCode::OK, Json(proxied)).into_response()
        }
        _ => (StatusCode::OK, "[]").into_response(),
    }
}

#[derive(Deserialize)]
pub struct KissKhMostViewQuery {
    pub c: Option<u32>,
}

#[utoipa::path(
    get,
    path = "/api/drama/kisskh/most-view",
    params(
        ("c" = Option<u32>, Query, description = "Country code filter"),
    ),
    responses(
        (status = 200, description = "Most-viewed shows", body = serde_json::Value),
        (status = 500, description = "Internal error"),
    ),
)]
pub async fn kisskh_most_view_handler(
    State(state): State<DramaState>,
    Query(params): Query<KissKhMostViewQuery>,
) -> Response {
    if !kisskh_enabled() {
        return kisskh_disabled_response();
    }
    let country = params.c.unwrap_or(0);
    tracing::info!("[drama] kisskh_most_view: c={}", country);
    match state.kisskh.get_most_view(country).await {
        Ok(items) => (StatusCode::OK, Json(items)).into_response(),
        Err(e) => {
            tracing::warn!("[drama] kisskh_most_view error (c={}): {}", country, e);
            let body = serde_json::json!({ "success": false, "error": e.to_string() });
            (StatusCode::INTERNAL_SERVER_ERROR, body.to_string()).into_response()
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/drama/kisskh/last-update",
    responses(
        (status = 200, description = "Recently updated shows", body = serde_json::Value),
        (status = 500, description = "Internal error"),
    ),
)]
pub async fn kisskh_last_update_handler(State(state): State<DramaState>) -> Response {
    if !kisskh_enabled() {
        return kisskh_disabled_response();
    }
    tracing::info!("[drama] kisskh_last_update");
    match state.kisskh.get_last_update().await {
        Ok(items) => (StatusCode::OK, Json(items)).into_response(),
        Err(e) => {
            tracing::warn!("[drama] kisskh_last_update error: {}", e);
            let body = serde_json::json!({ "success": false, "error": e.to_string() });
            (StatusCode::INTERNAL_SERVER_ERROR, body.to_string()).into_response()
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/drama/kisskh/detail/{id}",
    params(
        ("id" = u64, Path, description = "KissKH show id"),
    ),
    responses(
        (status = 200, description = "Show detail", body = serde_json::Value),
        (status = 500, description = "Internal error"),
    ),
)]
pub async fn kisskh_detail_handler(
    State(state): State<DramaState>,
    Path(id): Path<u64>,
) -> Response {
    if !kisskh_enabled() {
        return kisskh_disabled_response();
    }
    tracing::info!("[drama] kisskh_detail: id={}", id);
    match state.kisskh.get_detail(id).await {
        Ok(data) => (StatusCode::OK, Json(data)).into_response(),
        Err(e) => {
            tracing::warn!("[drama] kisskh_detail error for {}: {}", id, e);
            let body = serde_json::json!({ "success": false, "error": e.to_string() });
            (StatusCode::INTERNAL_SERVER_ERROR, body.to_string()).into_response()
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/drama/kisskh/show",
    responses(
        (status = 200, description = "Curated show list", body = serde_json::Value),
        (status = 500, description = "Internal error"),
    ),
)]
pub async fn kisskh_show_handler(State(state): State<DramaState>) -> Response {
    if !kisskh_enabled() {
        return kisskh_disabled_response();
    }
    tracing::info!("[drama] kisskh_show");
    match state.kisskh.get_show_list().await {
        Ok(items) => {
            tracing::info!("[drama] kisskh_show: {} results", items.len());
            (StatusCode::OK, Json(items)).into_response()
        }
        Err(e) => {
            tracing::warn!("[drama] kisskh_show error: {}", e);
            let body = serde_json::json!({ "success": false, "error": e.to_string() });
            (StatusCode::INTERNAL_SERVER_ERROR, body.to_string()).into_response()
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/drama/kisskh/browse",
    params(
        ("page" = Option<u32>, Query, description = "Page number"),
        ("page_size" = Option<u32>, Query, description = "Items per page (clamped 1-60)"),
        ("content_type" = Option<u32>, Query, description = "Content type filter"),
        ("sub" = Option<u32>, Query, description = "Subtitle filter"),
        ("country" = Option<u32>, Query, description = "Country filter"),
        ("status" = Option<u32>, Query, description = "Status filter"),
        ("order" = Option<u32>, Query, description = "Order (default 2)"),
    ),
    responses(
        (status = 200, description = "Browse page", body = serde_json::Value),
        (status = 500, description = "Internal error"),
    ),
)]
pub async fn kisskh_browse_handler(
    State(state): State<DramaState>,
    Query(params): Query<KissKhBrowseQuery>,
) -> Response {
    if !kisskh_enabled() {
        return kisskh_disabled_response();
    }
    let page = params.page.unwrap_or(1);
    // Clamp to a sane range; kisskh honors &pageSize so this controls grid density.
    let page_size = params.page_size.unwrap_or(28).clamp(1, 60);
    let content_type = params.content_type.unwrap_or(0);
    let sub = params.sub.unwrap_or(0);
    let country = params.country.unwrap_or(0);
    let status = params.status.unwrap_or(0);
    let order = params.order.unwrap_or(2);

    tracing::info!("[drama] kisskh_browse: page={} size={}", page, page_size);
    match state
        .kisskh
        .get_list(page, page_size, content_type, sub, country, status, order)
        .await
    {
        Ok(data) => {
            tracing::info!("[drama] kisskh_browse: got data");
            (StatusCode::OK, Json(data)).into_response()
        }
        Err(e) => {
            tracing::warn!("[drama] kisskh_browse error: {}", e);
            let body = serde_json::json!({ "success": false, "error": e.to_string() });
            (StatusCode::INTERNAL_SERVER_ERROR, body.to_string()).into_response()
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/drama/kisskh/upcoming",
    responses(
        (status = 200, description = "Upcoming shows", body = serde_json::Value),
        (status = 500, description = "Internal error"),
    ),
)]
pub async fn kisskh_upcoming_handler(State(state): State<DramaState>) -> Response {
    if !kisskh_enabled() {
        return kisskh_disabled_response();
    }
    tracing::info!("[drama] kisskh_upcoming");
    match state.kisskh.get_upcoming().await {
        Ok(items) => {
            tracing::info!("[drama] kisskh_upcoming: {} items", items.len());
            (StatusCode::OK, Json(items)).into_response()
        }
        Err(e) => {
            tracing::warn!("[drama] kisskh_upcoming error: {}", e);
            let body = serde_json::json!({ "success": false, "error": e.to_string() });
            (StatusCode::INTERNAL_SERVER_ERROR, body.to_string()).into_response()
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/drama/kisskh/top-rating",
    responses(
        (status = 200, description = "Top-rated shows", body = serde_json::Value),
        (status = 500, description = "Internal error"),
    ),
)]
pub async fn kisskh_top_rating_handler(State(state): State<DramaState>) -> Response {
    if !kisskh_enabled() {
        return kisskh_disabled_response();
    }
    tracing::info!("[drama] kisskh_top_rating");
    match state.kisskh.get_top_rating().await {
        Ok(items) => {
            tracing::info!("[drama] kisskh_top_rating: {} items", items.len());
            (StatusCode::OK, Json(items)).into_response()
        }
        Err(e) => {
            tracing::warn!("[drama] kisskh_top_rating error: {}", e);
            let body = serde_json::json!({ "success": false, "error": e.to_string() });
            (StatusCode::INTERNAL_SERVER_ERROR, body.to_string()).into_response()
        }
    }
}

// ── Dramacool ──────────────────────────────────────────────────────

#[utoipa::path(
    get,
    path = "/api/drama/dramacool/search",
    params(
        ("q" = String, Query, description = "Search keyword"),
    ),
    responses(
        (status = 200, description = "Search results", body = serde_json::Value),
        (status = 500, description = "Internal error"),
    ),
)]
pub async fn dramacool_search_handler(
    State(state): State<DramaState>,
    Query(params): Query<SearchQuery>,
) -> Response {
    tracing::info!("[drama] dramacool_search: q={}", params.q);
    match state.dramacool.search(&params.q).await {
        Ok(items) => {
            tracing::info!("[drama] dramacool_search: {} results", items.len());
            (StatusCode::OK, Json(items)).into_response()
        }
        Err(e) => {
            tracing::warn!("[drama] dramacool_search error: {}", e);
            let body = serde_json::json!({ "success": false, "error": e.to_string() });
            (StatusCode::INTERNAL_SERVER_ERROR, body.to_string()).into_response()
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/drama/dramacool/streams/{id}/{episode}",
    params(
        ("id" = String, Path, description = "Dramacool show id"),
        ("episode" = u32, Path, description = "Episode number"),
    ),
    responses(
        (status = 200, description = "Stream results", body = serde_json::Value),
        (status = 500, description = "Internal error"),
    ),
)]
pub async fn dramacool_streams_handler(
    State(state): State<DramaState>,
    headers: HeaderMap,
    Path((id, episode)): Path<(String, u32)>,
) -> Response {
    tracing::info!("[drama] dramacool_streams: id={} ep={}", id, episode);
    let proxy_base = dynamic_proxy_base(&headers, &state.proxy_base_url);
    match state
        .dramacool
        .get_streams_normalized(&id, 1, episode)
        .await
    {
        Ok(streams) => {
            tracing::info!("[drama] dramacool_streams: {} streams", streams.len());
            let mut proxied = wrap_with_proxy(streams, &proxy_base, &state.secret);
            encrypt_stream_result_urls(&mut proxied, &state.kkey);
            (StatusCode::OK, Json(proxied)).into_response()
        }
        Err(e) => {
            tracing::warn!("[drama] dramacool_streams error: {}", e);
            let body = serde_json::json!({ "success": false, "error": e.to_string() });
            (StatusCode::INTERNAL_SERVER_ERROR, body.to_string()).into_response()
        }
    }
}

// ── KissKH (xyra) ─────────────────────────────────────────────────

#[utoipa::path(
    get,
    path = "/api/drama/kisskh_xyra/search",
    params(
        ("q" = String, Query, description = "Search keyword"),
    ),
    responses(
        (status = 200, description = "Search results", body = serde_json::Value),
        (status = 500, description = "Internal error"),
    ),
)]
pub async fn kisskh_xyra_search_handler(
    State(state): State<DramaState>,
    Query(params): Query<SearchQuery>,
) -> Response {
    tracing::info!("[drama] kisskh_xyra_search: q={}", params.q);
    match state.kisskh_xyra.search(&params.q).await {
        Ok(items) => {
            tracing::info!("[drama] kisskh_xyra_search: {} results", items.len());
            (StatusCode::OK, Json(items)).into_response()
        }
        Err(e) => {
            tracing::warn!("[drama] kisskh_xyra_search error: {}", e);
            let body = serde_json::json!({ "success": false, "error": e.to_string() });
            (StatusCode::INTERNAL_SERVER_ERROR, body.to_string()).into_response()
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/drama/kisskh_xyra/streams/{id}/{episode}",
    params(
        ("id" = String, Path, description = "KissKH Xyra show id"),
        ("episode" = u32, Path, description = "Episode number"),
    ),
    responses(
        (status = 200, description = "Stream results (possibly empty array)", body = serde_json::Value),
    ),
)]
pub async fn kisskh_xyra_streams_handler(
    State(state): State<DramaState>,
    headers: HeaderMap,
    Path((id, episode)): Path<(String, u32)>,
) -> Response {
    tracing::info!("[drama] kisskh_xyra_streams: id={} ep={}", id, episode);
    let proxy_base = dynamic_proxy_base(&headers, &state.proxy_base_url);
    match state.kisskh_xyra.get_streams_by_id(&id, episode).await {
        Ok(streams) => {
            tracing::info!("[drama] kisskh_xyra_streams: {} streams", streams.len());
            let mut proxied = wrap_with_proxy(streams, &proxy_base, &state.secret);
            encrypt_stream_result_urls(&mut proxied, &state.kkey);
            (StatusCode::OK, Json(proxied)).into_response()
        }
        Err(e) => {
            tracing::info!(
                "[drama] kisskh_xyra_streams: no match for {} ({}), returning []",
                id,
                e
            );
            (StatusCode::OK, "[]").into_response()
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/drama/kisskh_xyra/home",
    responses(
        (status = 200, description = "Home payload", body = serde_json::Value),
    ),
)]
pub async fn kisskh_xyra_home_handler(State(state): State<DramaState>) -> Response {
    tracing::info!("[drama] kisskh_xyra_home");
    match state.kisskh_xyra.home().await {
        Ok(value) => (StatusCode::OK, Json(value)).into_response(),
        Err(e) => {
            tracing::warn!("[drama] kisskh_xyra_home error: {}", e);
            (StatusCode::OK, "{}").into_response()
        }
    }
}

// ── KissAsian ──────────────────────────────────────────────────────

#[utoipa::path(
    get,
    path = "/api/drama/kissasian/search",
    params(
        ("q" = String, Query, description = "Search keyword"),
    ),
    responses(
        (status = 200, description = "Search results", body = serde_json::Value),
        (status = 500, description = "Internal error"),
    ),
)]
pub async fn kissasian_search_handler(
    State(state): State<DramaState>,
    Query(params): Query<SearchQuery>,
) -> Response {
    tracing::info!("[drama] kissasian_search: q={}", params.q);
    match state.kissasian.search(&params.q).await {
        Ok(items) => {
            tracing::info!("[drama] kissasian_search: {} results", items.len());
            (StatusCode::OK, Json(items)).into_response()
        }
        Err(e) => {
            tracing::warn!("[drama] kissasian_search error: {}", e);
            let body = serde_json::json!({ "success": false, "error": e.to_string() });
            (StatusCode::INTERNAL_SERVER_ERROR, body.to_string()).into_response()
        }
    }
}

// ── KissAsian (kissasian.top) ──────────────────────────────────────

#[utoipa::path(
    get,
    path = "/api/drama/kissasian_top/search",
    params(
        ("q" = String, Query, description = "Search keyword"),
    ),
    responses(
        (status = 200, description = "Search results", body = serde_json::Value),
        (status = 500, description = "Internal error"),
    ),
)]
pub async fn kissasian_top_search_handler(
    State(state): State<DramaState>,
    Query(params): Query<SearchQuery>,
) -> Response {
    tracing::info!("[drama] kissasian_top_search: q={}", params.q);
    match state.kissasian_top.search(&params.q).await {
        Ok(items) => {
            tracing::info!("[drama] kissasian_top_search: {} results", items.len());
            (StatusCode::OK, Json(items)).into_response()
        }
        Err(e) => {
            tracing::warn!("[drama] kissasian_top_search error: {}", e);
            let body = serde_json::json!({ "success": false, "error": e.to_string() });
            (StatusCode::INTERNAL_SERVER_ERROR, body.to_string()).into_response()
        }
    }
}

#[derive(Deserialize)]
pub struct KATopBrowseQuery {
    pub page: Option<u32>,
    pub per_page: Option<u32>,
    pub kind: Option<String>,
    pub country: Option<String>,
}

#[utoipa::path(
    get,
    path = "/api/drama/kissasian_top/browse",
    params(
        ("page" = Option<u32>, Query, description = "Page number"),
        ("per_page" = Option<u32>, Query, description = "Items per page (default 28)"),
        ("kind" = Option<String>, Query, description = "Kind filter"),
        ("country" = Option<String>, Query, description = "Country filter"),
    ),
    responses(
        (status = 200, description = "Browse items", body = serde_json::Value),
        (status = 500, description = "Internal error"),
    ),
)]
pub async fn kissasian_top_browse_handler(
    State(state): State<DramaState>,
    Query(params): Query<KATopBrowseQuery>,
) -> Response {
    let page = params.page.unwrap_or(1);
    let per_page = params.per_page.unwrap_or(28);
    tracing::info!(
        "[drama] kissasian_top_browse: page={} per_page={} kind={:?} country={:?}",
        page,
        per_page,
        params.kind,
        params.country
    );
    match state
        .kissasian_top
        .browse(
            page,
            per_page,
            params.kind.as_deref(),
            params.country.as_deref(),
        )
        .await
    {
        Ok(items) => {
            tracing::info!("[drama] kissasian_top_browse: {} items", items.len());
            (StatusCode::OK, Json(items)).into_response()
        }
        Err(e) => {
            tracing::warn!("[drama] kissasian_top_browse error: {}", e);
            let body = serde_json::json!({ "success": false, "error": e.to_string() });
            (StatusCode::INTERNAL_SERVER_ERROR, body.to_string()).into_response()
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/drama/kissasian_top/info/{slug}",
    params(
        ("slug" = String, Path, description = "Show slug"),
    ),
    responses(
        (status = 200, description = "Show info", body = serde_json::Value),
        (status = 404, description = "Not found"),
        (status = 500, description = "Internal error"),
    ),
)]
pub async fn kissasian_top_info_handler(
    State(state): State<DramaState>,
    Path(slug): Path<String>,
) -> Response {
    tracing::info!("[drama] kissasian_top_info: slug={}", slug);
    match state.kissasian_top.info(&slug).await {
        Ok(Some(info)) => (StatusCode::OK, Json(info)).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            r#"{"success":false,"error":"not found"}"#,
        )
            .into_response(),
        Err(e) => {
            tracing::warn!("[drama] kissasian_top_info error: {}", e);
            let body = serde_json::json!({ "success": false, "error": e.to_string() });
            (StatusCode::INTERNAL_SERVER_ERROR, body.to_string()).into_response()
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/drama/kissasian_top/streams/{slug}/{episode}",
    params(
        ("slug" = String, Path, description = "Show slug"),
        ("episode" = u32, Path, description = "Episode number"),
    ),
    responses(
        (status = 200, description = "Stream results", body = serde_json::Value),
        (status = 500, description = "Internal error"),
    ),
)]
pub async fn kissasian_top_streams_handler(
    State(state): State<DramaState>,
    headers: HeaderMap,
    Path((slug, episode)): Path<(String, u32)>,
) -> Response {
    tracing::info!(
        "[drama] kissasian_top_streams: slug={} ep={}",
        slug,
        episode
    );
    let proxy_base = dynamic_proxy_base(&headers, &state.proxy_base_url);
    match state
        .kissasian_top
        .get_streams_normalized(&slug, episode)
        .await
    {
        Ok(streams) => {
            tracing::info!("[drama] kissasian_top_streams: {} streams", streams.len());
            let mut proxied = wrap_with_proxy(streams, &proxy_base, &state.secret);
            encrypt_stream_result_urls(&mut proxied, &state.kkey);
            (StatusCode::OK, Json(proxied)).into_response()
        }
        Err(e) => {
            tracing::warn!("[drama] kissasian_top_streams error: {}", e);
            let body = serde_json::json!({ "success": false, "error": e.to_string() });
            (StatusCode::INTERNAL_SERVER_ERROR, body.to_string()).into_response()
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/drama/kissasian/streams/{id}/{episode}",
    params(
        ("id" = String, Path, description = "Show slug/id"),
        ("episode" = u32, Path, description = "Episode number"),
    ),
    responses(
        (status = 200, description = "Stream results", body = serde_json::Value),
        (status = 500, description = "Internal error"),
    ),
)]
pub async fn kissasian_streams_handler(
    State(state): State<DramaState>,
    headers: HeaderMap,
    Path((id, episode)): Path<(String, u32)>,
) -> Response {
    tracing::info!("[drama] kissasian_streams: id={} ep={}", id, episode);
    let proxy_base = dynamic_proxy_base(&headers, &state.proxy_base_url);
    match state.kissasian.get_streams_by_slug(&id, episode).await {
        Ok(streams) => {
            tracing::info!("[drama] kissasian_streams: {} streams", streams.len());
            let mut proxied = wrap_with_proxy(streams, &proxy_base, &state.secret);
            encrypt_stream_result_urls(&mut proxied, &state.kkey);
            (StatusCode::OK, Json(proxied)).into_response()
        }
        Err(e) => {
            tracing::warn!("[drama] kissasian_streams error: {}", e);
            let body = serde_json::json!({ "success": false, "error": e.to_string() });
            (StatusCode::INTERNAL_SERVER_ERROR, body.to_string()).into_response()
        }
    }
}

// ── MovieBox ───────────────────────────────────────────────────────

#[utoipa::path(
    get,
    path = "/api/drama/moviebox/browse",
    params(
        ("page" = Option<u32>, Query, description = "Page number"),
        ("per_page" = Option<u32>, Query, description = "Items per page (default 28)"),
        ("country" = Option<String>, Query, description = "Country filter"),
        ("sort" = Option<String>, Query, description = "Sort key"),
        ("channel_id" = Option<u32>, Query, description = "Channel id"),
    ),
    responses(
        (status = 200, description = "Browse rails", body = serde_json::Value),
        (status = 500, description = "Internal error"),
    ),
)]
pub async fn moviebox_browse_handler(
    State(state): State<DramaState>,
    Query(params): Query<MovieBoxBrowseQuery>,
) -> Response {
    let page = params.page.unwrap_or(1);
    let per_page = params.per_page.unwrap_or(28);
    tracing::info!(
        "[drama] moviebox_browse: page={} per_page={}",
        page,
        per_page
    );
    match state
        .moviebox
        .browse_rails(
            page,
            per_page,
            params.country.as_deref(),
            params.sort.as_deref(),
            params.channel_id,
        )
        .await
    {
        Ok(rails) => {
            tracing::info!("[drama] moviebox_browse: {} rails", rails.len());
            (StatusCode::OK, Json(rails)).into_response()
        }
        Err(e) => {
            tracing::warn!("[drama] moviebox_browse error: {}", e);
            let body = serde_json::json!({ "success": false, "error": e.to_string() });
            (StatusCode::INTERNAL_SERVER_ERROR, body.to_string()).into_response()
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/drama/moviebox/search",
    params(
        ("q" = String, Query, description = "Search keyword"),
        ("page" = Option<u32>, Query, description = "Page number"),
        ("subject_type" = Option<u32>, Query, description = "Subject type (default 2)"),
    ),
    responses(
        (status = 200, description = "Search results", body = serde_json::Value),
        (status = 500, description = "Internal error"),
    ),
)]
pub async fn moviebox_search_handler(
    State(state): State<DramaState>,
    Query(params): Query<MovieBoxSearchQuery>,
) -> Response {
    let page = params.page.unwrap_or(1);
    let subject_type = params.subject_type.unwrap_or(2);
    tracing::info!(
        "[drama] moviebox_search: q={} page={} type={}",
        params.q,
        page,
        subject_type
    );
    match state.moviebox.search(&params.q, page, subject_type).await {
        Ok(items) => {
            tracing::info!("[drama] moviebox_search: {} results", items.len());
            (StatusCode::OK, Json(items)).into_response()
        }
        Err(e) => {
            tracing::warn!("[drama] moviebox_search error: {}", e);
            let body = serde_json::json!({ "success": false, "error": e.to_string() });
            (StatusCode::INTERNAL_SERVER_ERROR, body.to_string()).into_response()
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/drama/moviebox/streams/{title}/{episode}",
    params(
        ("title" = String, Path, description = "Title slug"),
        ("episode" = u32, Path, description = "Episode number"),
    ),
    responses(
        (status = 200, description = "Stream results", body = serde_json::Value),
        (status = 500, description = "Internal error"),
    ),
)]
pub async fn moviebox_streams_handler(
    State(state): State<DramaState>,
    headers: HeaderMap,
    Path((title, episode)): Path<(String, u32)>,
) -> Response {
    tracing::info!("[drama] moviebox_streams: title={} ep={}", title, episode);
    let proxy_base = dynamic_proxy_base(&headers, &state.proxy_base_url);
    match state
        .moviebox
        .get_streams_normalized(&title, "tv", None, 1, episode)
        .await
    {
        Ok(streams) => {
            tracing::info!("[drama] moviebox_streams: {} streams", streams.len());
            let mut proxied = wrap_with_proxy(streams, &proxy_base, &state.secret);
            encrypt_stream_result_urls(&mut proxied, &state.kkey);
            (StatusCode::OK, Json(proxied)).into_response()
        }
        Err(e) => {
            tracing::warn!("[drama] moviebox_streams error: {}", e);
            let body = serde_json::json!({ "success": false, "error": e.to_string() });
            (StatusCode::INTERNAL_SERVER_ERROR, body.to_string()).into_response()
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/drama/moviebox/streams-direct",
    params(
        ("detail_path" = String, Query, description = "MovieBox detail path"),
        ("episode" = Option<u32>, Query, description = "Episode number (default 1)"),
        ("subject_id" = Option<String>, Query, description = "Optional subject id to skip detail lookup"),
        ("media_type" = Option<String>, Query, description = "Media type override ('movie' or 'tv')"),
    ),
    responses(
        (status = 200, description = "Stream results", body = serde_json::Value),
        (status = 400, description = "Missing detail_path"),
        (status = 404, description = "Subject not found"),
        (status = 500, description = "Internal error"),
    ),
)]
pub async fn moviebox_streams_direct_handler(
    State(state): State<DramaState>,
    headers: HeaderMap,
    Query(params): Query<MovieBoxDirectStreamQuery>,
) -> Response {
    let detail_path = params.detail_path.trim();
    if detail_path.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            r#"{"success":false,"error":"detail_path query required"}"#,
        )
            .into_response();
    }

    let episode = params.episode.unwrap_or(1);
    let start = std::time::Instant::now();
    tracing::info!(
        "[drama] moviebox_streams_direct: path={} ep={}",
        detail_path,
        episode
    );

    let mut subject_type: Option<u32> = None;
    let subject_id = match params
        .subject_id
        .as_deref()
        .map(str::trim)
        .filter(|id| !id.is_empty())
    {
        Some(id) => {
            tracing::info!(
                "[drama] moviebox_streams_direct: using provided subject_id in {}ms",
                start.elapsed().as_millis()
            );
            id.to_string()
        }
        None => match state.moviebox.get_detail(detail_path).await {
            Ok(detail) => {
                subject_type = detail["data"]["subject"]["subjectType"]
                    .as_u64()
                    .or_else(|| detail["data"]["subjectType"].as_u64())
                    .or_else(|| detail["subject"]["subjectType"].as_u64())
                    .or_else(|| detail["subjectType"].as_u64())
                    .map(|value| value as u32);
                let sid = detail["data"]["subject"]["subjectId"]
                    .as_str()
                    .or_else(|| detail["data"]["subjectId"].as_str())
                    .or_else(|| detail["subject"]["subjectId"].as_str())
                    .or_else(|| detail["subjectId"].as_str());
                tracing::info!(
                    "[drama] moviebox detail: subjectType={:?} subjectId={:?}",
                    subject_type,
                    sid
                );
                sid.map(str::to_string).unwrap_or_default()
            }
            Err(e) => {
                tracing::warn!(
                    "[drama] moviebox_streams_direct detail error for {}: {}",
                    detail_path,
                    e
                );
                String::new()
            }
        },
    };

    if subject_id.is_empty() {
        return (
            StatusCode::NOT_FOUND,
            r#"{"success":false,"error":"MovieBox subjectId not found"}"#,
        )
            .into_response();
    }

    let media_type = params
        .media_type
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| {
            let mt = if subject_type == Some(1) {
                "movie"
            } else {
                "tv"
            };
            tracing::info!(
                "[drama] moviebox media_type inferred: {} (subject_type={:?})",
                mt,
                subject_type
            );
            mt
        });

    let proxy_base = dynamic_proxy_base(&headers, &state.proxy_base_url);
    let streams_start = std::time::Instant::now();
    match state
        .moviebox
        .get_streams_for_detail(&subject_id, detail_path, media_type, 1, episode)
        .await
    {
        Ok(streams) => {
            let streams_ms = streams_start.elapsed().as_millis();
            tracing::info!(
                "[drama] moviebox_streams_direct: {} streams fetched in {}ms",
                streams.len(),
                streams_ms
            );
            let wrap_start = std::time::Instant::now();
            let mut proxied = wrap_with_proxy(streams, &proxy_base, &state.secret);
            encrypt_stream_result_urls(&mut proxied, &state.kkey);
            tracing::info!(
                "[drama] moviebox_streams_direct: wrapped+encrypted in {}ms (total {}ms)",
                wrap_start.elapsed().as_millis(),
                start.elapsed().as_millis()
            );
            (StatusCode::OK, Json(proxied)).into_response()
        }
        Err(e) => {
            tracing::warn!("[drama] moviebox_streams_direct error: {}", e);
            let body = serde_json::json!({ "success": false, "error": e.to_string() });
            (StatusCode::INTERNAL_SERVER_ERROR, body.to_string()).into_response()
        }
    }
}

// ── Unified Detail ─────────────────────────────────────────────────

#[utoipa::path(
    get,
    path = "/api/drama/detail",
    params(
        ("q" = String, Query, description = "Title to look up"),
        ("id" = Option<String>, Query, description = "Optional existing show id"),
    ),
    responses(
        (status = 200, description = "Unified detail payload", body = serde_json::Value),
        (status = 404, description = "No detail found"),
    ),
)]
pub async fn drama_detail_handler(
    State(state): State<DramaState>,
    Query(params): Query<DetailQuery>,
) -> Response {
    // ── Curated source: MovieBox (primary drama content) ──
    let moviebox_items = match state.moviebox.search(&params.q, 1, 2).await {
        Ok(items) if !items.is_empty() => items,
        Ok(_) => {
            tracing::info!("[drama_detail] MovieBox no results for '{}'", params.q);
            Vec::new()
        }
        Err(e) => {
            tracing::warn!(
                "[drama_detail] MovieBox search error for '{}': {}",
                params.q,
                e
            );
            Vec::new()
        }
    };

    if !moviebox_items.is_empty() {
        let best = &moviebox_items[0];
        let mut cover = best.cover.clone().unwrap_or_default();
        let detail_path = best.detail_path.clone().unwrap_or_default();

        // Try to enrich episodes from MovieBox detail endpoint
        let mut episodes: Vec<serde_json::Value> = Vec::new();
        if !detail_path.is_empty() {
            match state.moviebox.get_detail(&detail_path).await {
                Ok(detail_data) => {
                    episodes = moviebox_detail_episodes(&detail_data);
                }
                Err(e) => {
                    tracing::warn!(
                        "[drama_detail] MovieBox get_detail({}) failed: {}",
                        detail_path,
                        e
                    );
                }
            }
        }

        // Fallback: if no cover, try TMDB
        if cover.is_empty() {
            match state.tmdb.search(&params.q, "multi", None).await {
                Ok(tmdb_results) => {
                    if let Some(first) = tmdb_results.first() {
                        if let Some(poster) = first.poster_path.as_deref() {
                            cover = format!("https://image.tmdb.org/t/p/w500{}", poster);
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        "[drama_detail] TMDB search for '{}' failed: {}",
                        params.q,
                        e
                    );
                }
            }
        }

        let episode_count = if episodes.is_empty() {
            1
        } else {
            episodes.len()
        };

        let response = serde_json::json!({
            "success": true,
            "provider": "moviebox",
            "id": best.id,
            "title": best.title,
            "cover": cover,
            "description": best.description.as_deref().unwrap_or(""),
            "year": best.release_date.as_ref().and_then(|d| d.split('-').next()).and_then(|y| y.parse::<u32>().ok()),
            "country": "",
            "status": "",
            "genres": best.genres.clone().unwrap_or_default(),
            "episodes": episodes,
            "episode_count": episode_count
        });
        return (StatusCode::OK, Json(response)).into_response();
    }

    // ── Optional fallback to KissKH ──
    if !kisskh_enabled() {
        return (
            StatusCode::NOT_FOUND,
            r#"{"success":false,"error":"No MovieBox detail found"}"#,
        )
            .into_response();
    }

    let kisskh_show_id = params.id.as_ref().and_then(|id| id.parse::<u64>().ok());

    let kisskh_result = if let Some(id) = kisskh_show_id {
        match state.kisskh.get_detail(id).await {
            Ok(detail) => Some((id, detail)),
            Err(e) => {
                tracing::warn!("[drama_detail] KissKH get_detail({}) failed: {}", id, e);
                None
            }
        }
    } else {
        match state.kisskh.search(&params.q).await {
            Ok(results) if !results.is_empty() => {
                let best = &results[0];
                match state.kisskh.get_detail(best.id).await {
                    Ok(detail) => Some((best.id, detail)),
                    Err(e) => {
                        tracing::warn!(
                            "[drama_detail] KissKH get_detail({}) failed: {}",
                            best.id,
                            e
                        );
                        None
                    }
                }
            }
            _ => None,
        }
    };

    if let Some((show_id, detail)) = kisskh_result {
        let episodes = detail["episodes"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|e| {
                        let id = e["id"].as_u64()?.to_string();
                        let number = e["number"].as_f64()? as u32;
                        let title = format!("Episode {}", number);
                        Some(serde_json::json!({
                            "id": id,
                            "number": number,
                            "title": title
                        }))
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let episode_count = episodes.len();

        let mut cover = detail["cover"]
            .as_str()
            .filter(|s| !s.is_empty())
            .or_else(|| detail["thumbnail"].as_str().filter(|s| !s.is_empty()))
            .unwrap_or("")
            .to_string();

        if cover.is_empty() {
            match state.tmdb.search(&params.q, "multi", None).await {
                Ok(tmdb_results) => {
                    if let Some(first) = tmdb_results.first() {
                        if let Some(poster) = first.poster_path.as_deref() {
                            cover = format!("https://image.tmdb.org/t/p/w500{}", poster);
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        "[drama_detail] TMDB search for '{}' failed: {}",
                        params.q,
                        e
                    );
                }
            }
        }

        let title = detail["title"]
            .as_str()
            .or_else(|| detail["name"].as_str())
            .unwrap_or(&params.q)
            .to_string();

        let response = serde_json::json!({
            "success": true,
            "provider": "kisskh",
            "id": show_id.to_string(),
            "title": title,
            "cover": cover,
            "description": detail["description"].as_str().unwrap_or(""),
            "year": detail["year"].as_u64().map(|y| y as u32),
            "country": detail["country"].as_str().unwrap_or(""),
            "status": detail["status"].as_str().unwrap_or(""),
            "genre": detail["genre"].as_str().unwrap_or(""),
            "episodes": episodes,
            "episode_count": episode_count
        });
        return (StatusCode::OK, Json(response)).into_response();
    }

    (
        StatusCode::NOT_FOUND,
        r#"{"success":false,"error":"No detail found"}"#,
    )
        .into_response()
}

// ── MovieBox Detail Rec ──────────────────────────────────────────────

#[utoipa::path(
    get,
    path = "/api/drama/moviebox/detail-rec",
    params(
        ("id" = String, Query, description = "Subject id to fetch recommendations for"),
        ("page" = Option<u32>, Query, description = "Page number (default 1)"),
        ("per_page" = Option<u32>, Query, description = "Items per page (default 12)"),
    ),
    responses(
        (status = 200, description = "Recommended items", body = serde_json::Value),
        (status = 500, description = "Internal error"),
    ),
)]
pub async fn moviebox_detail_rec_handler(
    State(state): State<DramaState>,
    Query(params): Query<DetailRecQuery>,
) -> Response {
    let page = params.page.unwrap_or(1);
    let per_page = params.per_page.unwrap_or(12);
    tracing::info!(
        "[drama] moviebox_detail_rec: id={} page={} per_page={}",
        params.id,
        page,
        per_page
    );
    match state
        .moviebox
        .get_detail_rec(&params.id, page, per_page)
        .await
    {
        Ok(items) => {
            tracing::info!("[drama] moviebox_detail_rec: {} items", items.len());
            let body = serde_json::json!({ "success": true, "items": items });
            (StatusCode::OK, Json(body)).into_response()
        }
        Err(e) => {
            tracing::warn!("[drama] moviebox_detail_rec error: {}", e);
            let body = serde_json::json!({ "success": false, "error": e.to_string() });
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(body),
            )
                .into_response()
        }
    }
}
