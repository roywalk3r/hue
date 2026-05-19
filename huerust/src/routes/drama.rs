//! Hue Stream API
//! Professional Streaming Aggregator
//!
//! @author roywalk3r <roywalker440@gmail.com>
//! @repository https://github.com/roywalk3r/hue.git
//! @license MIT

use axum::{
    extract::{Query, State, Path},
    response::{IntoResponse, Response},
    http::StatusCode,
};
use serde::Deserialize;

use crate::app::DramaState;

use crate::proxy::wrap::wrap_with_proxy;

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
}

#[derive(Deserialize)]
pub struct MovieBoxSearchQuery {
    pub q: String,
    pub page: Option<u32>,
    pub subject_type: Option<u32>,
}

#[derive(Deserialize)]
pub struct DetailQuery {
    pub q: String,
    pub id: Option<String>,
}

// ── KissKH ─────────────────────────────────────────────────────────

pub async fn kisskh_search_handler(
    State(state): State<DramaState>,
    Query(params): Query<SearchQuery>,
) -> Response {
    tracing::info!("[drama] kisskh_search: q={}", params.q);
    match state.kisskh.search(&params.q).await {
        Ok(items) => {
            tracing::info!("[drama] kisskh_search: {} results for q={}", items.len(), params.q);
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
    Path((id, episode)): Path<(String, u32)>,
    Query(params): Query<KissKhStreamsQuery>,
) -> Response {
    // Prefer numeric ID from path; only fall back to title search when ID is not numeric.
    let lookup = if id.parse::<u64>().is_ok() {
        id.as_str()
    } else {
        params.title.as_ref().map(|t| t.as_str()).unwrap_or(&id)
    };

    match state.kisskh.get_streams_direct(lookup, episode).await {
        Ok(streams) if !streams.is_empty() => {
            let proxied = wrap_with_proxy(streams, &state.proxy_base_url, &state.secret);
            (StatusCode::OK, serde_json::to_string(&proxied).unwrap()).into_response()
        }
        _ => (StatusCode::OK, "[]").into_response(),
    }
}

pub async fn kisskh_show_handler(
    State(state): State<DramaState>,
) -> Response {
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
    let page = params.page.unwrap_or(1);
    let _page_size = params.page_size.unwrap_or(28);
    let content_type = params.content_type.unwrap_or(0);
    let sub = params.sub.unwrap_or(0);
    let country = params.country.unwrap_or(0);
    let status = params.status.unwrap_or(0);
    let order = params.order.unwrap_or(2);

    tracing::info!("[drama] kisskh_browse: page={}", page);
    match state.kisskh.get_list(page, content_type, sub, country, status, order).await {
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
    Path((id, episode)): Path<(String, u32)>,
) -> Response {
    tracing::info!("[drama] dramacool_streams: id={} ep={}", id, episode);
    match state.dramacool.get_streams_normalized(&id, 1, episode).await {
        Ok(streams) => {
            tracing::info!("[drama] dramacool_streams: {} streams", streams.len());
            let proxied = wrap_with_proxy(streams, &state.proxy_base_url, &state.secret);
            (StatusCode::OK, serde_json::to_string(&proxied).unwrap()).into_response()
        }
        Err(e) => {
            tracing::warn!("[drama] dramacool_streams error: {}", e);
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
    tracing::info!("[drama] moviebox_browse: page={} per_page={}", page, per_page);
    match state.moviebox.browse_rails(page, per_page, params.country.as_deref(), params.sort.as_deref()).await {
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
    let subject_type = params.subject_type.unwrap_or(0);
    tracing::info!("[drama] moviebox_search: q={} page={} type={}", params.q, page, subject_type);
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
    Path((title, episode)): Path<(String, u32)>,
) -> Response {
    tracing::info!("[drama] moviebox_streams: title={} ep={}", title, episode);
    match state.moviebox.get_streams_normalized(&title, "tv", None, 1, episode).await {
        Ok(streams) => {
            tracing::info!("[drama] moviebox_streams: {} streams", streams.len());
            let proxied = wrap_with_proxy(streams, &state.proxy_base_url, &state.secret);
            (StatusCode::OK, serde_json::to_string(&proxied).unwrap()).into_response()
        }
        Err(e) => {
            tracing::warn!("[drama] moviebox_streams error: {}", e);
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
    // ── Try Dramacool first (curated drama content, reliable) ──
    match state.dramacool.get_detail(&params.q).await {
        Ok(Some((show, info))) => {
            let episodes = info.episodes.iter().enumerate().map(|(idx, ep)| {
                let number = (idx + 1) as u32;
                serde_json::json!({
                    "id": ep.episode_id.clone(),
                    "number": number,
                    "title": ep.title.clone()
                })
            }).collect::<Vec<_>>();
            let episode_count = episodes.len();

            // Enrich cover via TMDB since Dramacool doesn't provide images
            let mut cover = String::new();
            match state.tmdb.search(&params.q, "multi", None).await {
                Ok(tmdb_results) => {
                    if let Some(first) = tmdb_results.first() {
                        if let Some(poster) = first.poster_path.as_deref() {
                            cover = format!("https://image.tmdb.org/t/p/w500{}", poster);
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("[drama_detail] TMDB search for '{}' failed: {}", params.q, e);
                }
            }

            let year = show.release_year.as_ref().and_then(|y| y.parse::<u32>().ok());

            let response = serde_json::json!({
                "success": true,
                "provider": "dramacool",
                "id": show.id,
                "title": show.title,
                "cover": cover,
                "description": "",
                "year": year,
                "country": "",
                "status": "",
                "genre": "",
                "episodes": episodes,
                "episode_count": episode_count
            });
            return (StatusCode::OK, serde_json::to_string(&response).unwrap()).into_response();
        }
        Ok(None) => {
            tracing::info!("[drama_detail] Dramacool no results for '{}'", params.q);
        }
        Err(e) => {
            tracing::warn!("[drama_detail] Dramacool error for '{}': {}", params.q, e);
        }
    }

    // ── Fallback to KissKH (Cloudflare-challenged) ──
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
                        tracing::warn!("[drama_detail] KissKH get_detail({}) failed: {}", best.id, e);
                        None
                    }
                }
            }
            _ => None,
        }
    };

    if let Some((show_id, detail)) = kisskh_result {
        let episodes = detail["episodes"].as_array()
            .map(|arr| arr.iter().filter_map(|e| {
                let id = e["id"].as_u64()?.to_string();
                let number = e["number"].as_f64()? as u32;
                let title = format!("Episode {}", number);
                Some(serde_json::json!({
                    "id": id,
                    "number": number,
                    "title": title
                }))
            }).collect::<Vec<_>>())
            .unwrap_or_default();

        let episode_count = episodes.len();

        let mut cover = detail["cover"].as_str()
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
                    tracing::warn!("[drama_detail] TMDB search for '{}' failed: {}", params.q, e);
                }
            }
        }

        let title = detail["title"].as_str()
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

    // ── Last resort: MovieBox ──
    match state.moviebox.search(&params.q, 1, 0).await {
        Ok(items) if !items.is_empty() => {
            let best = &items[0];
            let response = serde_json::json!({
                "success": true,
                "provider": "moviebox",
                "id": best.id,
                "title": best.title,
                "cover": best.cover.as_deref().unwrap_or(""),
                "description": best.description.as_deref().unwrap_or(""),
                "year": best.release_date.as_ref().and_then(|d| d.split('-').next()).and_then(|y| y.parse::<u32>().ok()),
                "country": best.genre.as_deref().unwrap_or(""),
                "status": "",
                "genre": best.genre.as_deref().unwrap_or(""),
                "episodes": [],
                "episode_count": 1
            });
            return (StatusCode::OK, serde_json::to_string(&response).unwrap()).into_response();
        }
        _ => {}
    }

    (StatusCode::NOT_FOUND, r#"{"success":false,"error":"No detail found"}"#).into_response()
}
