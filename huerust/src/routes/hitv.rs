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
use std::collections::HashMap;
use serde::Serialize;
use crate::{
    AppState,
    hitv::HiTvItem,
};
use tracing::{info, warn};

#[derive(Serialize)]
struct HitvListResponse {
    success: bool,
    items: Vec<HiTvItem>,
    total: u32,
}

#[derive(Serialize)]
struct HitvDetailResponse {
    success: bool,
    item: Option<HiTvItem>,
}

#[derive(Serialize)]
struct StaticListResponse {
    success: bool,
    items: Vec<String>,
}

pub async fn browse_handler(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    let country = params.get("country").map(|s| s.as_str()).unwrap_or("All");
    let category = params.get("category").map(|s| s.as_str()).unwrap_or("Drama");
    let genre = params.get("genre").map(|s| s.as_str()).unwrap_or("All");
    let sort = params.get("sort").map(|s| s.as_str()).unwrap_or("Hottest");
    let year = params.get("year").map(|s| s.as_str());
    let page = params.get("page").and_then(|s| s.parse::<u32>().ok()).unwrap_or(1);
    let page_size = params.get("page_size").and_then(|s| s.parse::<u32>().ok()).unwrap_or(60);

    info!("[hitv] Browse: country={} category={} genre={} sort={} page={}", country, category, genre, sort, page);

    match state.hitv.browse_hitv(page, page_size, country, category, genre, sort, year).await {
        Ok(res) => (StatusCode::OK, serde_json::to_string(&HitvListResponse {
            success: true,
            items: res.items,
            total: res.total,
        }).unwrap()).into_response(),
        Err(e) => {
            warn!("[hitv] Browse failed: {}", e);
            (StatusCode::BAD_GATEWAY, serde_json::to_string(&HitvListResponse {
                success: false,
                items: vec![],
                total: 0,
            }).unwrap()).into_response()
        }
    }
}

pub async fn search_handler(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    let query = params.get("q").map(|s| s.as_str()).unwrap_or("");
    let page = params.get("page").and_then(|s| s.parse::<u32>().ok()).unwrap_or(1);
    let page_size = params.get("page_size").and_then(|s| s.parse::<u32>().ok()).unwrap_or(60);

    if query.is_empty() {
        return (StatusCode::BAD_REQUEST, "Missing 'q' parameter").into_response();
    }

    info!("[hitv] Search: q={} page={}", query, page);

    match state.hitv.search(query, page, page_size).await {
        Ok(res) => (StatusCode::OK, serde_json::to_string(&HitvListResponse {
            success: true,
            items: res.items,
            total: res.total,
        }).unwrap()).into_response(),
        Err(e) => {
            warn!("[hitv] Search failed: {}", e);
            (StatusCode::BAD_GATEWAY, serde_json::to_string(&HitvListResponse {
                success: false,
                items: vec![],
                total: 0,
            }).unwrap()).into_response()
        }
    }
}

pub async fn latest_handler(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    let country = params.get("country").map(|s| s.as_str()).unwrap_or("All");
    let category = params.get("category").map(|s| s.as_str()).unwrap_or("Drama");
    let genre = params.get("genre").map(|s| s.as_str()).unwrap_or("All");
    let page = params.get("page").and_then(|s| s.parse::<u32>().ok()).unwrap_or(1);
    let page_size = params.get("page_size").and_then(|s| s.parse::<u32>().ok()).unwrap_or(60);

    match state.hitv.browse_hitv(page, page_size, country, category, genre, "Newest", None).await {
        Ok(res) => (StatusCode::OK, serde_json::to_string(&HitvListResponse {
            success: true,
            items: res.items,
            total: res.total,
        }).unwrap()).into_response(),
        Err(e) => {
            warn!("[hitv] Latest failed: {}", e);
            (StatusCode::BAD_GATEWAY, serde_json::to_string(&HitvListResponse {
                success: false,
                items: vec![],
                total: 0,
            }).unwrap()).into_response()
        }
    }
}

pub async fn trending_handler(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    let country = params.get("country").map(|s| s.as_str()).unwrap_or("All");
    let category = params.get("category").map(|s| s.as_str()).unwrap_or("Drama");
    let genre = params.get("genre").map(|s| s.as_str()).unwrap_or("All");
    let page = params.get("page").and_then(|s| s.parse::<u32>().ok()).unwrap_or(1);
    let page_size = params.get("page_size").and_then(|s| s.parse::<u32>().ok()).unwrap_or(60);

    match state.hitv.browse_hitv(page, page_size, country, category, genre, "Hottest", None).await {
        Ok(res) => (StatusCode::OK, serde_json::to_string(&HitvListResponse {
            success: true,
            items: res.items,
            total: res.total,
        }).unwrap()).into_response(),
        Err(e) => {
            warn!("[hitv] Trending failed: {}", e);
            (StatusCode::BAD_GATEWAY, serde_json::to_string(&HitvListResponse {
                success: false,
                items: vec![],
                total: 0,
            }).unwrap()).into_response()
        }
    }
}

pub async fn hero_handler(State(state): State<AppState>) -> Response {
    let library = state.hitv.get_library().await;
    let items: Vec<HiTvItem> = library.into_iter().take(10).collect();

    (StatusCode::OK, serde_json::to_string(&HitvListResponse {
        success: true,
        items,
        total: 10,
    }).unwrap()).into_response()
}

pub async fn drama_handler(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    let country = params.get("country").map(|s| s.as_str()).unwrap_or("All");
    let genre = params.get("genre").map(|s| s.as_str()).unwrap_or("All");
    let sort = params.get("sort").map(|s| s.as_str()).unwrap_or("Hottest");
    let page = params.get("page").and_then(|s| s.parse::<u32>().ok()).unwrap_or(1);
    let page_size = params.get("page_size").and_then(|s| s.parse::<u32>().ok()).unwrap_or(60);

    match state.hitv.browse_hitv(page, page_size, country, "Drama", genre, sort, None).await {
        Ok(res) => (StatusCode::OK, serde_json::to_string(&HitvListResponse {
            success: true,
            items: res.items,
            total: res.total,
        }).unwrap()).into_response(),
        Err(e) => {
            warn!("[hitv] Drama browse failed: {}", e);
            (StatusCode::BAD_GATEWAY, serde_json::to_string(&HitvListResponse {
                success: false,
                items: vec![],
                total: 0,
            }).unwrap()).into_response()
        }
    }
}

pub async fn info_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Response {
    info!("[hitv] Info: id={}", id);

    match state.hitv.get_detail(&id).await {
        Ok(Some(item)) => (StatusCode::OK, serde_json::to_string(&HitvDetailResponse {
            success: true,
            item: Some(item),
        }).unwrap()).into_response(),
        Ok(None) => (StatusCode::NOT_FOUND, serde_json::to_string(&HitvDetailResponse {
            success: false,
            item: None,
        }).unwrap()).into_response(),
        Err(e) => {
            warn!("[hitv] Info failed: {}", e);
            (StatusCode::BAD_GATEWAY, serde_json::to_string(&HitvDetailResponse {
                success: false,
                item: None,
            }).unwrap()).into_response()
        }
    }
}

pub async fn play_handler(
    State(state): State<AppState>,
    Path((id, episode)): Path<(String, u32)>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    info!("[hitv] Play: id={} episode={}", id, episode);

    let season = params.get("season").and_then(|s| s.parse::<u32>().ok()).unwrap_or(1);

    // Reverse-map HiTV internal ID to TMDB ID
    let tmdb_id = match state.hitv.get_detail(&id).await {
        Ok(Some(item)) => {
            if let Some(tid) = item.tmdb_id {
                tid.to_string()
            } else {
                let clean_title = item.title
                    .replace(" (TV Series)", "")
                    .replace(" (Movie)", "")
                    .replace(" [Korean Drama]", "")
                    .replace(" [Chinese Drama]", "")
                    .trim()
                    .to_string();

                match state.tmdb.search(&clean_title, "tv", None).await {
                    Ok(results) if !results.is_empty() => results[0].id.to_string(),
                    _ => {
                        warn!("[hitv] Could not map HiTV ID {} to TMDB", id);
                        return (StatusCode::NOT_FOUND, "Could not resolve TMDB ID").into_response();
                    }
                }
            }
        }
        _ => {
            warn!("[hitv] Could not get detail for HiTV ID {}", id);
            return (StatusCode::NOT_FOUND, "HiTV item not found").into_response();
        }
    };

    info!("[hitv] Resolved HiTV {} to TMDB {}", id, tmdb_id);

    // Delegate to the Smart Play gateway logic by forwarding internally
    // We simulate what play::play_handler does but with resolved TMDB ID
    // Since play::play_handler expects Path and Query, we can't easily call it.
    // Instead we redirect the user or we inline the logic.
    // For simplicity, redirect to /api/play/tv/{tmdb_id}?season={season}&episode={episode}

    let redirect_url = format!("{}/api/play/tv/{}?season={}&episode={}", state.proxy_base_url, tmdb_id, season, episode);
    (StatusCode::TEMPORARY_REDIRECT, [("Location", redirect_url)], "").into_response()
}

pub async fn countries_handler() -> Response {
    let countries = vec!["Korea", "China", "Thailand", "Japan"];
    (StatusCode::OK, serde_json::to_string(&StaticListResponse {
        success: true,
        items: countries.into_iter().map(String::from).collect(),
    }).unwrap()).into_response()
}

pub async fn categories_handler() -> Response {
    let categories = vec!["Drama", "Movie", "Variety"];
    (StatusCode::OK, serde_json::to_string(&StaticListResponse {
        success: true,
        items: categories.into_iter().map(String::from).collect(),
    }).unwrap()).into_response()
}

pub async fn genres_handler() -> Response {
    let genres = vec!["All", "Romance", "Comedy", "Action", "Thriller", "Horror", "Fantasy", "Historical", "Mystery", "Family"];
    (StatusCode::OK, serde_json::to_string(&StaticListResponse {
        success: true,
        items: genres.into_iter().map(String::from).collect(),
    }).unwrap()).into_response()
}

pub async fn sorts_handler() -> Response {
    let sorts = vec!["Hottest", "Newest", "TopRated"];
    (StatusCode::OK, serde_json::to_string(&StaticListResponse {
        success: true,
        items: sorts.into_iter().map(String::from).collect(),
    }).unwrap()).into_response()
}

pub async fn dynamic_browse_handler(
    State(state): State<AppState>,
    Path((country, sort, genre)): Path<(String, String, String)>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    let category = params.get("category").map(|s| s.as_str()).unwrap_or("Drama");
    let page = params.get("page").and_then(|s| s.parse::<u32>().ok()).unwrap_or(1);
    let page_size = params.get("page_size").and_then(|s| s.parse::<u32>().ok()).unwrap_or(60);

    info!("[hitv] Dynamic browse: {}/{}/{} category={} page={}", country, sort, genre, category, page);

    match state.hitv.browse_hitv(page, page_size, &country, category, &genre, &sort, None).await {
        Ok(res) => (StatusCode::OK, serde_json::to_string(&HitvListResponse {
            success: true,
            items: res.items,
            total: res.total,
        }).unwrap()).into_response(),
        Err(e) => {
            warn!("[hitv] Dynamic browse failed: {}", e);
            (StatusCode::BAD_GATEWAY, serde_json::to_string(&HitvListResponse {
                success: false,
                items: vec![],
                total: 0,
            }).unwrap()).into_response()
        }
    }
}
