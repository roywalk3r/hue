//! Hue Stream API
//! Professional Streaming Aggregator
//!
//! @author roywalk3r <roywalker440@gmail.com>
//! @repository https://github.com/roywalk3r/hue.git
//! @license MIT

use axum::{
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
            (StatusCode::OK, serde_json::to_string(&items).unwrap()).into_response()
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
            (StatusCode::OK, serde_json::to_string(&proxied).unwrap()).into_response()
        }
        _ => (StatusCode::OK, "[]").into_response(),
    }
}

pub async fn kisskh_show_handler(State(state): State<DramaState>) -> Response {
    if !kisskh_enabled() {
        return kisskh_disabled_response();
    }
    tracing::info!("[drama] kisskh_show");
    match state.kisskh.get_show_list().await {
        Ok(items) => {
            tracing::info!("[drama] kisskh_show: {} results", items.len());
            (StatusCode::OK, serde_json::to_string(&items).unwrap()).into_response()
        }
        Err(e) => {
            tracing::warn!("[drama] kisskh_show error: {}", e);
            let body = serde_json::json!({ "success": false, "error": e.to_string() });
            (StatusCode::INTERNAL_SERVER_ERROR, body.to_string()).into_response()
        }
    }
}

pub async fn kisskh_browse_handler(
    State(state): State<DramaState>,
    Query(params): Query<KissKhBrowseQuery>,
) -> Response {
    if !kisskh_enabled() {
        return kisskh_disabled_response();
    }
    let page = params.page.unwrap_or(1);
    let _page_size = params.page_size.unwrap_or(28);
    let content_type = params.content_type.unwrap_or(0);
    let sub = params.sub.unwrap_or(0);
    let country = params.country.unwrap_or(0);
    let status = params.status.unwrap_or(0);
    let order = params.order.unwrap_or(2);

    tracing::info!("[drama] kisskh_browse: page={}", page);
    match state
        .kisskh
        .get_list(page, content_type, sub, country, status, order)
        .await
    {
        Ok(data) => {
            tracing::info!("[drama] kisskh_browse: got data");
            (StatusCode::OK, serde_json::to_string(&data).unwrap()).into_response()
        }
        Err(e) => {
            tracing::warn!("[drama] kisskh_browse error: {}", e);
            let body = serde_json::json!({ "success": false, "error": e.to_string() });
            (StatusCode::INTERNAL_SERVER_ERROR, body.to_string()).into_response()
        }
    }
}

pub async fn kisskh_upcoming_handler(State(state): State<DramaState>) -> Response {
    if !kisskh_enabled() {
        return kisskh_disabled_response();
    }
    tracing::info!("[drama] kisskh_upcoming");
    match state.kisskh.get_upcoming().await {
        Ok(items) => {
            tracing::info!("[drama] kisskh_upcoming: {} items", items.len());
            (StatusCode::OK, serde_json::to_string(&items).unwrap()).into_response()
        }
        Err(e) => {
            tracing::warn!("[drama] kisskh_upcoming error: {}", e);
            let body = serde_json::json!({ "success": false, "error": e.to_string() });
            (StatusCode::INTERNAL_SERVER_ERROR, body.to_string()).into_response()
        }
    }
}

pub async fn kisskh_top_rating_handler(State(state): State<DramaState>) -> Response {
    if !kisskh_enabled() {
        return kisskh_disabled_response();
    }
    tracing::info!("[drama] kisskh_top_rating");
    match state.kisskh.get_top_rating().await {
        Ok(items) => {
            tracing::info!("[drama] kisskh_top_rating: {} items", items.len());
            (StatusCode::OK, serde_json::to_string(&items).unwrap()).into_response()
        }
        Err(e) => {
            tracing::warn!("[drama] kisskh_top_rating error: {}", e);
            let body = serde_json::json!({ "success": false, "error": e.to_string() });
            (StatusCode::INTERNAL_SERVER_ERROR, body.to_string()).into_response()
        }
    }
}

// ── Dramacool ──────────────────────────────────────────────────────

pub async fn dramacool_search_handler(
    State(state): State<DramaState>,
    Query(params): Query<SearchQuery>,
) -> Response {
    tracing::info!("[drama] dramacool_search: q={}", params.q);
    match state.dramacool.search(&params.q).await {
        Ok(items) => {
            tracing::info!("[drama] dramacool_search: {} results", items.len());
            (StatusCode::OK, serde_json::to_string(&items).unwrap()).into_response()
        }
        Err(e) => {
            tracing::warn!("[drama] dramacool_search error: {}", e);
            let body = serde_json::json!({ "success": false, "error": e.to_string() });
            (StatusCode::INTERNAL_SERVER_ERROR, body.to_string()).into_response()
        }
    }
}

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
            (StatusCode::OK, serde_json::to_string(&proxied).unwrap()).into_response()
        }
        Err(e) => {
            tracing::warn!("[drama] dramacool_streams error: {}", e);
            let body = serde_json::json!({ "success": false, "error": e.to_string() });
            (StatusCode::INTERNAL_SERVER_ERROR, body.to_string()).into_response()
        }
    }
}

// ── KissKH (xyra) ─────────────────────────────────────────────────

pub async fn kisskh_xyra_search_handler(
    State(state): State<DramaState>,
    Query(params): Query<SearchQuery>,
) -> Response {
    tracing::info!("[drama] kisskh_xyra_search: q={}", params.q);
    match state.kisskh_xyra.search(&params.q).await {
        Ok(items) => {
            tracing::info!("[drama] kisskh_xyra_search: {} results", items.len());
            (StatusCode::OK, serde_json::to_string(&items).unwrap()).into_response()
        }
        Err(e) => {
            tracing::warn!("[drama] kisskh_xyra_search error: {}", e);
            let body = serde_json::json!({ "success": false, "error": e.to_string() });
            (StatusCode::INTERNAL_SERVER_ERROR, body.to_string()).into_response()
        }
    }
}

pub async fn kisskh_xyra_streams_handler(
    State(state): State<DramaState>,
    headers: HeaderMap,
    Path((id, episode)): Path<(String, u32)>,
) -> Response {
    tracing::info!("[drama] kisskh_xyra_streams: id={} ep={}", id, episode);
    let proxy_base = dynamic_proxy_base(&headers, &state.proxy_base_url);
    match state
        .kisskh_xyra
        .get_streams_by_id(&id, episode)
        .await
    {
        Ok(streams) => {
            tracing::info!("[drama] kisskh_xyra_streams: {} streams", streams.len());
            let mut proxied = wrap_with_proxy(streams, &proxy_base, &state.secret);
            encrypt_stream_result_urls(&mut proxied, &state.kkey);
            (StatusCode::OK, serde_json::to_string(&proxied).unwrap()).into_response()
        }
        Err(e) => {
            tracing::info!("[drama] kisskh_xyra_streams: no match for {} ({}), returning []", id, e);
            (StatusCode::OK, "[]").into_response()
        }
    }
}

pub async fn kisskh_xyra_home_handler(State(state): State<DramaState>) -> Response {
    tracing::info!("[drama] kisskh_xyra_home");
    match state.kisskh_xyra.home().await {
        Ok(value) => (StatusCode::OK, serde_json::to_string(&value).unwrap()).into_response(),
        Err(e) => {
            tracing::warn!("[drama] kisskh_xyra_home error: {}", e);
            (StatusCode::OK, "{}").into_response()
        }
    }
}

// ── KissAsian ──────────────────────────────────────────────────────

pub async fn kissasian_search_handler(
    State(state): State<DramaState>,
    Query(params): Query<SearchQuery>,
) -> Response {
    tracing::info!("[drama] kissasian_search: q={}", params.q);
    match state.kissasian.search(&params.q).await {
        Ok(items) => {
            tracing::info!("[drama] kissasian_search: {} results", items.len());
            (StatusCode::OK, serde_json::to_string(&items).unwrap()).into_response()
        }
        Err(e) => {
            tracing::warn!("[drama] kissasian_search error: {}", e);
            let body = serde_json::json!({ "success": false, "error": e.to_string() });
            (StatusCode::INTERNAL_SERVER_ERROR, body.to_string()).into_response()
        }
    }
}

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
            (StatusCode::OK, serde_json::to_string(&proxied).unwrap()).into_response()
        }
        Err(e) => {
            tracing::warn!("[drama] kissasian_streams error: {}", e);
            let body = serde_json::json!({ "success": false, "error": e.to_string() });
            (StatusCode::INTERNAL_SERVER_ERROR, body.to_string()).into_response()
        }
    }
}

// ── MovieBox ───────────────────────────────────────────────────────

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
            (StatusCode::OK, serde_json::to_string(&rails).unwrap()).into_response()
        }
        Err(e) => {
            tracing::warn!("[drama] moviebox_browse error: {}", e);
            let body = serde_json::json!({ "success": false, "error": e.to_string() });
            (StatusCode::INTERNAL_SERVER_ERROR, body.to_string()).into_response()
        }
    }
}

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
            (StatusCode::OK, serde_json::to_string(&items).unwrap()).into_response()
        }
        Err(e) => {
            tracing::warn!("[drama] moviebox_search error: {}", e);
            let body = serde_json::json!({ "success": false, "error": e.to_string() });
            (StatusCode::INTERNAL_SERVER_ERROR, body.to_string()).into_response()
        }
    }
}

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
            (StatusCode::OK, serde_json::to_string(&proxied).unwrap()).into_response()
        }
        Err(e) => {
            tracing::warn!("[drama] moviebox_streams error: {}", e);
            let body = serde_json::json!({ "success": false, "error": e.to_string() });
            (StatusCode::INTERNAL_SERVER_ERROR, body.to_string()).into_response()
        }
    }
}

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
            tracing::info!("[drama] moviebox_streams_direct: using provided subject_id in {}ms", start.elapsed().as_millis());
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
            (StatusCode::OK, serde_json::to_string(&proxied).unwrap()).into_response()
        }
        Err(e) => {
            tracing::warn!("[drama] moviebox_streams_direct error: {}", e);
            let body = serde_json::json!({ "success": false, "error": e.to_string() });
            (StatusCode::INTERNAL_SERVER_ERROR, body.to_string()).into_response()
        }
    }
}

// ── Unified Detail ─────────────────────────────────────────────────

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
        return (StatusCode::OK, serde_json::to_string(&response).unwrap()).into_response();
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
        return (StatusCode::OK, serde_json::to_string(&response).unwrap()).into_response();
    }

    (
        StatusCode::NOT_FOUND,
        r#"{"success":false,"error":"No detail found"}"#,
    )
        .into_response()
}

// ── MovieBox Detail Rec ──────────────────────────────────────────────

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
            (StatusCode::OK, serde_json::to_string(&body).unwrap()).into_response()
        }
        Err(e) => {
            tracing::warn!("[drama] moviebox_detail_rec error: {}", e);
            let body = serde_json::json!({ "success": false, "error": e.to_string() });
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                serde_json::to_string(&body).unwrap(),
            )
                .into_response()
        }
    }
}
