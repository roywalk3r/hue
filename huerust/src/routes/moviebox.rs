//! Hue Stream API
//! MovieBox Browse Routes
//!
//! @author roywalk3r <roywalker440@gmail.com>
//! @repository https://github.com/roywalk3r/hue.git
//! @license MIT

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
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
    pub channel_id: Option<u32>,
    #[allow(dead_code)]
    pub genre: Option<String>,
    pub country: Option<String>,
    #[allow(dead_code)]
    pub year: Option<String>,
    pub sort: Option<String>,
}

#[derive(Deserialize)]
pub struct InfoQuery {
    pub path: String,
}

#[derive(Deserialize)]
pub struct DetailQuery {
    pub q: Option<String>,
    pub path: Option<String>,
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
        return (
            StatusCode::BAD_REQUEST,
            r#"{"success":false,"error":"title query required"}"#,
        )
            .into_response();
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
    if let Ok(items) = trending {
        all.extend(items);
    }
    if let Ok(items) = kdrama {
        all.extend(items);
    }
    if let Ok(items) = cdrama {
        all.extend(items);
    }

    // Deduplicate by id
    let mut seen = std::collections::HashSet::new();
    all.retain(|item| {
        if seen.contains(&item.id) {
            false
        } else {
            seen.insert(item.id.clone());
            true
        }
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
        Err(e) => {
            tracing::warn!("MovieBox bollywood failed: {}", e);
            let resp = serde_json::json!({ "success": false, "items": [], "error": e.to_string() });
            (StatusCode::OK, serde_json::to_string(&resp).unwrap()).into_response()
        }
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
        Err(e) => {
            tracing::warn!("MovieBox shorts failed: {}", e);
            let resp = serde_json::json!({ "success": false, "items": [], "error": e.to_string() });
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
    let sort = params.sort.as_deref();
    let channel_id = params.channel_id;
    match state
        .moviebox
        .filter(page, per_page, country, sort, channel_id)
        .await
    {
        Ok(items) => {
            let resp = serde_json::json!({ "success": true, "page": page, "items": items });
            (StatusCode::OK, serde_json::to_string(&resp).unwrap()).into_response()
        }
        Err(e) => {
            tracing::warn!("MovieBox filter failed: {}", e);
            let resp = serde_json::json!({ "success": false, "page": page, "items": [], "error": e.to_string() });
            (StatusCode::OK, serde_json::to_string(&resp).unwrap()).into_response()
        }
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

fn moviebox_detail_cast(detail_data: &serde_json::Value) -> Vec<serde_json::Value> {
    let stars = detail_data["data"]["stars"].as_array()
        .or_else(|| detail_data["data"]["subject"]["stars"].as_array());

    let Some(list) = stars else { return Vec::new(); };

    list.iter()
        .filter_map(|star| {
            let name = star["name"].as_str()?.trim();
            if name.is_empty() { return None; }
            let avatar = star["avatarUrl"].as_str().unwrap_or("");
            let character = star["character"].as_str().unwrap_or("");
            Some(serde_json::json!({
                "name": name,
                "character": character,
                "avatar": avatar,
            }))
        })
        .collect()
}

fn moviebox_detail_text(detail_data: &serde_json::Value, keys: &[&str]) -> String {
    let candidates = [
        &detail_data["data"]["subject"],
        &detail_data["data"]["metadata"],
        &detail_data["data"]["resource"],
        &detail_data["data"],
        detail_data,
    ];

    for value in candidates {
        for key in keys {
            if let Some(text) = value[*key].as_str().filter(|text| !text.trim().is_empty()) {
                return text.to_string();
            }
        }
    }

    String::new()
}

/// MovieBox detail endpoint returns `cover` as either a plain string or a nested object `{"url": "..."}`.
fn moviebox_detail_cover(detail_data: &serde_json::Value, keys: &[&str]) -> String {
    let candidates = [
        &detail_data["data"]["subject"],
        &detail_data["data"]["metadata"],
        &detail_data["data"]["resource"],
        &detail_data["data"],
        detail_data,
    ];

    for value in candidates {
        for key in keys {
            let field = &value[*key];
            // Nested object: cover.url
            if let Some(url) = field["url"].as_str().filter(|u| !u.trim().is_empty()) {
                return url.to_string();
            }
            // Plain string
            if let Some(text) = field.as_str().filter(|t| !t.trim().is_empty()) {
                return text.to_string();
            }
        }
    }

    String::new()
}

fn moviebox_detail_response(
    id: &str,
    title: String,
    detail_data: &serde_json::Value,
    fallback: Option<&crate::clients::moviebox::MovieBoxItem>,
) -> serde_json::Value {
    let episodes = moviebox_detail_episodes(detail_data);
    let episode_count = if episodes.is_empty() {
        1
    } else {
        episodes.len()
    };

    serde_json::json!({
        "success": true,
        "provider": "moviebox",
        "id": id,
        "title": title,
        "cover": fallback
            .and_then(|item| item.cover.clone())
            .filter(|cover| !cover.is_empty())
            .unwrap_or_else(|| moviebox_detail_cover(detail_data, &["cover", "poster", "image", "coverUrl", "imgUrl", "img", "backdrop"])),
        "description": fallback
            .and_then(|item| item.description.clone())
            .filter(|description| !description.is_empty())
            .unwrap_or_else(|| moviebox_detail_text(detail_data, &["description", "desc", "introduction", "summary", "plot", "synopsis", "story", "content"])),
        "year": fallback
            .and_then(|item| item.release_date.as_ref())
            .and_then(|date| date.split('-').next())
            .and_then(|year| year.parse::<u32>().ok()),
        "country": fallback
            .and_then(|item| item.genre.clone())
            .unwrap_or_else(|| moviebox_detail_text(detail_data, &["country", "area"])),
        "status": moviebox_detail_text(detail_data, &["status"]),
        "rating": moviebox_detail_text(detail_data, &["imdbRatingValue", "rate", "rating", "score"]),
        "genre": fallback
            .and_then(|item| item.genre.clone())
            .unwrap_or_else(|| moviebox_detail_text(detail_data, &["genre", "category"])),
        "episodes": episodes,
        "episode_count": episode_count,
        "cast": moviebox_detail_cast(detail_data),
    })
}

pub async fn detail_handler(
    State(state): State<MovieBoxRouteState>,
    Query(params): Query<DetailQuery>,
) -> Response {
    let clean_path = params
        .path
        .as_deref()
        .map(str::trim)
        .filter(|path| !path.is_empty());
    let clean_query = params.q.as_deref().map(str::trim).filter(|q| !q.is_empty());

    if let Some(path) = clean_path {
        match state.moviebox.get_detail(path).await {
            Ok(detail_data) => {
                let title = clean_query
                    .map(str::to_string)
                    .filter(|title| !title.is_empty())
                    .unwrap_or_else(|| {
                        moviebox_detail_text(&detail_data, &["title", "name", "subjectTitle"])
                    });
                let response = moviebox_detail_response(path, title, &detail_data, None);
                return (StatusCode::OK, response.to_string()).into_response();
            }
            Err(e) => {
                tracing::warn!(
                    "[moviebox_detail] direct detailPath lookup failed for '{}': {}",
                    path,
                    e
                );
            }
        }
    }

    let Some(query) = clean_query else {
        return (
            StatusCode::BAD_REQUEST,
            r#"{"success":false,"error":"q or path query required"}"#,
        )
            .into_response();
    };

    let item = match state.moviebox.search(query, 1, 2).await {
        Ok(items) => items
            .into_iter()
            .next()
            .and_then(|item| item.detail_path.clone().map(|path| (item, path))),
        Err(e) => {
            tracing::warn!("[moviebox_detail] search error for '{}': {}", query, e);
            None
        }
    };

    let Some((item, detail_path)) = item else {
        return (
            StatusCode::NOT_FOUND,
            r#"{"success":false,"error":"MovieBox detail not found"}"#,
        )
            .into_response();
    };

    let detail_data = match state.moviebox.get_detail(&detail_path).await {
        Ok(data) => data,
        Err(e) => return (StatusCode::BAD_GATEWAY, e.to_string()).into_response(),
    };
    let id = item.detail_path.as_deref().unwrap_or(&item.id);
    let response = moviebox_detail_response(id, item.title.clone(), &detail_data, Some(&item));
    (StatusCode::OK, response.to_string()).into_response()
}

pub async fn search_handler(
    State(state): State<MovieBoxRouteState>,
    Query(params): Query<SearchQuery>,
) -> Response {
    let page = params.page.unwrap_or(1);
    let subject_type = params.r#type.unwrap_or(0);
    match state
        .moviebox
        .search(&params.keyword, page, subject_type)
        .await
    {
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
