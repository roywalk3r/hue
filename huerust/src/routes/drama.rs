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
    match state.kisskh.search(&params.q).await {
        Ok(items) => (StatusCode::OK, serde_json::to_string(&items).unwrap()).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
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
    match state.kisskh.get_show_list().await {
        Ok(items) => (StatusCode::OK, serde_json::to_string(&items).unwrap()).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
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

    match state.kisskh.get_list(page, content_type, sub, country, status, order).await {
        Ok(data) => (StatusCode::OK, serde_json::to_string(&data).unwrap()).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

pub async fn kisskh_upcoming_handler(State(state): State<DramaState>) -> Response {
    match state.kisskh.get_upcoming().await {
        Ok(items) => (StatusCode::OK, serde_json::to_string(&items).unwrap()).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

pub async fn kisskh_top_rating_handler(State(state): State<DramaState>) -> Response {
    match state.kisskh.get_top_rating().await {
        Ok(items) => (StatusCode::OK, serde_json::to_string(&items).unwrap()).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

// ── Dramacool ──────────────────────────────────────────────────────

pub async fn dramacool_search_handler(
    State(state): State<DramaState>,
    Query(params): Query<SearchQuery>,
) -> Response {
    match state.dramacool.search(&params.q).await {
        Ok(items) => (StatusCode::OK, serde_json::to_string(&items).unwrap()).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

pub async fn dramacool_streams_handler(
    State(state): State<DramaState>,
    Path((id, episode)): Path<(String, u32)>,
) -> Response {
    match state.dramacool.get_streams_normalized(&id, 1, episode).await {
        Ok(streams) => {
            let proxied = wrap_with_proxy(streams, &state.proxy_base_url, &state.secret);
            (StatusCode::OK, serde_json::to_string(&proxied).unwrap()).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

// ── MovieBox ───────────────────────────────────────────────────────

pub async fn moviebox_browse_handler(
    State(state): State<DramaState>,
    Query(params): Query<MovieBoxBrowseQuery>,
) -> Response {
    let page = params.page.unwrap_or(1);
    let per_page = params.per_page.unwrap_or(28);
    match state.moviebox.browse_rails(page, per_page, params.country.as_deref()).await {
        Ok(rails) => (StatusCode::OK, serde_json::to_string(&rails).unwrap()).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

pub async fn moviebox_search_handler(
    State(state): State<DramaState>,
    Query(params): Query<MovieBoxSearchQuery>,
) -> Response {
    let page = params.page.unwrap_or(1);
    let subject_type = params.subject_type.unwrap_or(0);
    match state.moviebox.search(&params.q, page, subject_type).await {
        Ok(items) => (StatusCode::OK, serde_json::to_string(&items).unwrap()).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

pub async fn moviebox_streams_handler(
    State(state): State<DramaState>,
    Path((title, episode)): Path<(String, u32)>,
) -> Response {
    match state.moviebox.get_streams_normalized(&title, "tv", None, 1, episode).await {
        Ok(streams) => {
            let proxied = wrap_with_proxy(streams, &state.proxy_base_url, &state.secret);
            (StatusCode::OK, serde_json::to_string(&proxied).unwrap()).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

// ── Unified Detail ─────────────────────────────────────────────────

pub async fn drama_detail_handler(
    State(state): State<DramaState>,
    Query(params): Query<DetailQuery>,
) -> Response {
    // ── Try KissKH first ──
    // If a numeric id is provided, use it directly; otherwise search by title
    let kisskh_show_id = params.id.as_ref().and_then(|id| id.parse::<u64>().ok());

    let kisskh_result = if let Some(id) = kisskh_show_id {
        // Direct ID path: get detail without searching
        match state.kisskh.get_detail(id).await {
            Ok(detail) => Some((id, detail)),
            Err(e) => {
                tracing::warn!("[drama_detail] KissKH get_detail({}) failed: {}", id, e);
                None
            }
        }
    } else {
        // Search path
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

        // Resolve cover: try "cover", then "thumbnail", then TMDB enrichment
        let mut cover = detail["cover"].as_str()
            .filter(|s| !s.is_empty())
            .or_else(|| detail["thumbnail"].as_str().filter(|s| !s.is_empty()))
            .unwrap_or("")
            .to_string();

        if cover.is_empty() {
            match state.tmdb.search(&params.q, "multi", None).await {
                Ok(tmdb_results) => {
                    tracing::info!("[drama_detail] TMDB search for '{}' returned {} results", params.q, tmdb_results.len());
                    if let Some(first) = tmdb_results.first() {
                        tracing::info!("[drama_detail] First result poster_path: {:?}", first.poster_path);
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

    // ── Fallback to MovieBox ──
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
