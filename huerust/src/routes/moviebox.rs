//! Hue Stream API
//! MovieBox Browse Routes
//!
//! @author roywalk3r <roywalker440@gmail.com>
//! @repository https://github.com/roywalk3r/hue.git
//! @license MIT

use axum::{
    extract::{Query, State},
    response::{IntoResponse, Response},
    http::StatusCode,
};
use serde::Deserialize;

use crate::app::MovieBoxRouteState;

#[derive(Deserialize)]
pub struct SectionQuery {
    pub title: Option<String>,
}

#[derive(Deserialize)]
pub struct FilterQuery {
    pub page: Option<u32>,
    pub per_page: Option<u32>,
    #[allow(dead_code)]
    pub channel_id: Option<u32>,
    #[allow(dead_code)]
    pub genre: Option<String>,
    pub country: Option<String>,
    #[allow(dead_code)]
    pub year: Option<String>,
    #[allow(dead_code)]
    pub sort: Option<String>,
}

#[derive(Deserialize)]
pub struct InfoQuery {
    pub path: String,
}

#[derive(Deserialize)]
pub struct SearchQuery {
    pub keyword: String,
    pub page: Option<u32>,
    pub r#type: Option<u32>,
}

#[derive(Deserialize)]
pub struct BollywoodQuery {
    pub r#type: Option<String>,
    pub page: Option<u32>,
}

#[derive(Deserialize)]
pub struct ShortsQuery {
    pub page: Option<u32>,
}

// ── Home / Discover ──────────────────────────────────────────────────────────

pub async fn home_handler(State(state): State<MovieBoxRouteState>) -> Response {
    match state.moviebox.get_home().await {
        Ok(data) => (StatusCode::OK, serde_json::to_string(&data).unwrap()).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

pub async fn discover_handler(State(state): State<MovieBoxRouteState>) -> Response {
    match state.moviebox.get_discover().await {
        Ok(sections) => {
            let resp = serde_json::json!({ "success": true, "sections": sections });
            (StatusCode::OK, serde_json::to_string(&resp).unwrap()).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

pub async fn section_handler(
    State(state): State<MovieBoxRouteState>,
    Query(params): Query<SectionQuery>,
) -> Response {
    let title = params.title.as_deref().unwrap_or("");
    if title.is_empty() {
        return (StatusCode::BAD_REQUEST, r#"{"success":false,"error":"title query required"}"#).into_response();
    }
    match state.moviebox.get_section(title).await {
        Ok(items) => {
            let resp = serde_json::json!({ "success": true, "items": items });
            (StatusCode::OK, serde_json::to_string(&resp).unwrap()).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

pub async fn trending_handler(State(state): State<MovieBoxRouteState>) -> Response {
    match state.moviebox.get_trending(1).await {
        Ok(items) => {
            let resp = serde_json::json!({ "success": true, "items": items });
            (StatusCode::OK, serde_json::to_string(&resp).unwrap()).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

// ── Named Sections (thin wrappers around get_section) ────────────────────────

pub async fn popular_movie_handler(State(state): State<MovieBoxRouteState>) -> Response {
    match state.moviebox.get_section("Popular Movie").await {
        Ok(items) => {
            let resp = serde_json::json!({ "success": true, "items": items });
            (StatusCode::OK, serde_json::to_string(&resp).unwrap()).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

pub async fn popular_series_handler(State(state): State<MovieBoxRouteState>) -> Response {
    match state.moviebox.get_section("Popular Series").await {
        Ok(items) => {
            let resp = serde_json::json!({ "success": true, "items": items });
            (StatusCode::OK, serde_json::to_string(&resp).unwrap()).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

pub async fn hot_shorts_handler(State(state): State<MovieBoxRouteState>) -> Response {
    match state.moviebox.get_section("Hot Short TV").await {
        Ok(items) => {
            let resp = serde_json::json!({ "success": true, "items": items });
            (StatusCode::OK, serde_json::to_string(&resp).unwrap()).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

pub async fn sa_handler(State(state): State<MovieBoxRouteState>) -> Response {
    match state.moviebox.get_section("SA Drama").await {
        Ok(items) => {
            let resp = serde_json::json!({ "success": true, "items": items });
            (StatusCode::OK, serde_json::to_string(&resp).unwrap()).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

pub async fn thai_handler(State(state): State<MovieBoxRouteState>) -> Response {
    match state.moviebox.get_section("Thai-Drama").await {
        Ok(items) => {
            let resp = serde_json::json!({ "success": true, "items": items });
            (StatusCode::OK, serde_json::to_string(&resp).unwrap()).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

pub async fn turkish_handler(State(state): State<MovieBoxRouteState>) -> Response {
    match state.moviebox.get_section("Turkish Drama").await {
        Ok(items) => {
            let resp = serde_json::json!({ "success": true, "items": items });
            (StatusCode::OK, serde_json::to_string(&resp).unwrap()).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

pub async fn western_handler(State(state): State<MovieBoxRouteState>) -> Response {
    match state.moviebox.get_section("Western TV").await {
        Ok(items) => {
            let resp = serde_json::json!({ "success": true, "items": items });
            (StatusCode::OK, serde_json::to_string(&resp).unwrap()).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

pub async fn animation_handler(State(state): State<MovieBoxRouteState>) -> Response {
    match state.moviebox.get_section("Animation Collection").await {
        Ok(items) => {
            let resp = serde_json::json!({ "success": true, "items": items });
            (StatusCode::OK, serde_json::to_string(&resp).unwrap()).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

pub async fn black_shows_handler(State(state): State<MovieBoxRouteState>) -> Response {
    match state.moviebox.get_section("Must-watch Black Shows").await {
        Ok(items) => {
            let resp = serde_json::json!({ "success": true, "items": items });
            (StatusCode::OK, serde_json::to_string(&resp).unwrap()).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

// ── Hero ─────────────────────────────────────────────────────────────────────

pub async fn hero_handler(State(state): State<MovieBoxRouteState>) -> Response {
    // Simplified hero: combine trending + K-Drama + C-Drama, dedupe, sort by rating
    let (trending, kdrama, cdrama) = tokio::join!(
        state.moviebox.get_trending(1),
        state.moviebox.get_section("K-Drama"),
        state.moviebox.get_section("C-Drama"),
    );

    let mut all = Vec::new();
    if let Ok(items) = trending { all.extend(items); }
    if let Ok(items) = kdrama { all.extend(items); }
    if let Ok(items) = cdrama { all.extend(items); }

    // Deduplicate by id
    let mut seen = std::collections::HashSet::new();
    all.retain(|item| {
        if seen.contains(&item.id) { false }
        else { seen.insert(item.id.clone()); true }
    });

    // Sort by rating desc
    all.sort_by(|a, b| {
        let ra = b.rating.unwrap_or(0.0);
        let rb = a.rating.unwrap_or(0.0);
        ra.partial_cmp(&rb).unwrap_or(std::cmp::Ordering::Equal)
    });

    let resp = serde_json::json!({
        "success": true,
        "count": all.len().min(12),
        "items": all.into_iter().take(12).collect::<Vec<_>>()
    });
    (StatusCode::OK, serde_json::to_string(&resp).unwrap()).into_response()
}

// ── Bollywood / Shorts ───────────────────────────────────────────────────────

pub async fn bollywood_handler(
    State(state): State<MovieBoxRouteState>,
    Query(params): Query<BollywoodQuery>,
) -> Response {
    let media_type = params.r#type.as_deref().unwrap_or("movie");
    let page = params.page.unwrap_or(1);
    match state.moviebox.get_indian_content(media_type, page).await {
        Ok(items) => {
            let resp = serde_json::json!({ "success": true, "items": items });
            (StatusCode::OK, serde_json::to_string(&resp).unwrap()).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

pub async fn shorts_handler(
    State(state): State<MovieBoxRouteState>,
    Query(params): Query<ShortsQuery>,
) -> Response {
    let page = params.page.unwrap_or(1);
    match state.moviebox.get_shorts(page).await {
        Ok(items) => {
            let resp = serde_json::json!({ "success": true, "items": items });
            (StatusCode::OK, serde_json::to_string(&resp).unwrap()).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

// ── Filter ───────────────────────────────────────────────────────────────────

pub async fn filter_handler(
    State(state): State<MovieBoxRouteState>,
    Query(params): Query<FilterQuery>,
) -> Response {
    let page = params.page.unwrap_or(1);
    let per_page = params.per_page.unwrap_or(28);
    let country = params.country.as_deref();
    match state.moviebox.filter(page, per_page, country).await {
        Ok(items) => {
            let resp = serde_json::json!({ "success": true, "page": page, "items": items });
            (StatusCode::OK, serde_json::to_string(&resp).unwrap()).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

// ── Info / Search ────────────────────────────────────────────────────────────

pub async fn info_handler(
    State(state): State<MovieBoxRouteState>,
    Query(params): Query<InfoQuery>,
) -> Response {
    match state.moviebox.get_detail(&params.path).await {
        Ok(data) => {
            let resp = serde_json::json!({ "success": true, "data": data });
            (StatusCode::OK, serde_json::to_string(&resp).unwrap()).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

pub async fn search_handler(
    State(state): State<MovieBoxRouteState>,
    Query(params): Query<SearchQuery>,
) -> Response {
    let page = params.page.unwrap_or(1);
    let subject_type = params.r#type.unwrap_or(0);
    match state.moviebox.search(&params.keyword, page, subject_type).await {
        Ok(items) => {
            let resp = serde_json::json!({
                "success": true,
                "page": page,
                "items": items
            });
            (StatusCode::OK, serde_json::to_string(&resp).unwrap()).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}
