//! Hue Stream API
//! TMDB Grouped Home Routes
//!
//! @author roywalk3r <roywalker440@gmail.com>
//! @repository https://github.com/roywalk3r/hue.git
//! @license MIT

use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::app::TmdbState;
use crate::clients::tmdb::TmdbNormalizedInfo;

#[derive(Serialize, utoipa::ToSchema)]
pub struct TmdbListResponse {
    success: bool,
    items: Vec<TmdbNormalizedInfo>,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct TmdbHomeResponse {
    success: bool,
    shelves: HashMap<String, Vec<TmdbNormalizedInfo>>,
}

#[derive(Deserialize)]
pub struct HomeQuery {
    #[serde(rename = "type")]
    pub media_type: Option<String>,
}

#[derive(Deserialize)]
pub struct PageQuery {
    pub page: Option<u32>,
}

#[derive(Deserialize)]
pub struct TrendingQuery {
    #[serde(rename = "type")]
    pub media_type: Option<String>,
    pub page: Option<u32>,
}

#[derive(Deserialize)]
pub struct DiscoverQuery {
    #[serde(rename = "type")]
    pub media_type: Option<String>,
    pub genre: Option<String>,
    pub page: Option<u32>,
}

#[derive(Deserialize)]
pub struct SearchQuery {
    pub q: String,
    #[serde(rename = "type")]
    pub media_type: Option<String>,
    pub year: Option<u32>,
}

fn list_response(items: Vec<TmdbNormalizedInfo>) -> Response {
    (
        StatusCode::OK,
        Json(TmdbListResponse {
            success: true,
            items,
        }),
    )
        .into_response()
}

fn error_response() -> Response {
    (
        StatusCode::BAD_GATEWAY,
        Json(TmdbListResponse {
            success: false,
            items: vec![],
        }),
    )
        .into_response()
}

#[utoipa::path(
    get,
    path = "/api/tmdb/home",
    params(
        ("type" = Option<String>, Query, description = "Media type: 'movie' (default) or 'tv'"),
    ),
    responses(
        (status = 200, description = "Grouped home shelves", body = TmdbHomeResponse),
        (status = 500, description = "Internal error"),
    ),
)]
pub async fn home_handler(
    State(state): State<TmdbState>,
    Query(params): Query<HomeQuery>,
) -> Response {
    let m_type = params.media_type.as_deref().unwrap_or("movie");
    let page = 1u32;

    if m_type == "tv" {
        let (popular, top_rated, on_the_air, trending, action, comedy, drama, horror, sci_fi) = tokio::join!(
            state.tmdb.tv_popular(page),
            state.tmdb.tv_top_rated(page),
            state.tmdb.tv_on_the_air(page),
            state.tmdb.trending("tv", page),
            state.tmdb.discover("tv", "10759", page),
            state.tmdb.discover("tv", "35", page),
            state.tmdb.discover("tv", "18", page),
            state.tmdb.discover("tv", "9648", page),
            state.tmdb.discover("tv", "10765", page),
        );

        let mut shelves = HashMap::new();
        shelves.insert("popular".to_string(), popular.unwrap_or_default());
        shelves.insert("top_rated".to_string(), top_rated.unwrap_or_default());
        shelves.insert("on_the_air".to_string(), on_the_air.unwrap_or_default());
        shelves.insert("trending".to_string(), trending.unwrap_or_default());
        shelves.insert("action".to_string(), action.unwrap_or_default());
        shelves.insert("comedy".to_string(), comedy.unwrap_or_default());
        shelves.insert("drama".to_string(), drama.unwrap_or_default());
        shelves.insert("horror".to_string(), horror.unwrap_or_default());
        shelves.insert("sci_fi".to_string(), sci_fi.unwrap_or_default());

        (
            StatusCode::OK,
            Json(TmdbHomeResponse {
                success: true,
                shelves,
            }),
        )
            .into_response()
    } else {
        let (popular, top_rated, now_playing, trending, action, comedy, drama, horror, sci_fi) = tokio::join!(
            state.tmdb.movie_popular(page),
            state.tmdb.movie_top_rated(page),
            state.tmdb.movie_now_playing(page),
            state.tmdb.trending("movie", page),
            state.tmdb.discover("movie", "28", page),
            state.tmdb.discover("movie", "35", page),
            state.tmdb.discover("movie", "18", page),
            state.tmdb.discover("movie", "27", page),
            state.tmdb.discover("movie", "878", page),
        );

        let mut shelves = HashMap::new();
        shelves.insert("popular".to_string(), popular.unwrap_or_default());
        shelves.insert("top_rated".to_string(), top_rated.unwrap_or_default());
        shelves.insert("now_playing".to_string(), now_playing.unwrap_or_default());
        shelves.insert("trending".to_string(), trending.unwrap_or_default());
        shelves.insert("action".to_string(), action.unwrap_or_default());
        shelves.insert("comedy".to_string(), comedy.unwrap_or_default());
        shelves.insert("drama".to_string(), drama.unwrap_or_default());
        shelves.insert("horror".to_string(), horror.unwrap_or_default());
        shelves.insert("sci_fi".to_string(), sci_fi.unwrap_or_default());

        (
            StatusCode::OK,
            Json(TmdbHomeResponse {
                success: true,
                shelves,
            }),
        )
            .into_response()
    }
}

#[utoipa::path(
    get,
    path = "/api/tmdb/movie/popular",
    params(
        ("page" = Option<u32>, Query, description = "Page number (default 1)"),
    ),
    responses(
        (status = 200, description = "Popular movies", body = TmdbListResponse),
        (status = 500, description = "Internal error"),
    ),
)]
pub async fn movie_popular_handler(
    State(state): State<TmdbState>,
    Query(params): Query<PageQuery>,
) -> Response {
    match state.tmdb.movie_popular(params.page.unwrap_or(1)).await {
        Ok(items) => list_response(items),
        Err(_) => error_response(),
    }
}

#[utoipa::path(
    get,
    path = "/api/tmdb/movie/top-rated",
    params(
        ("page" = Option<u32>, Query, description = "Page number (default 1)"),
    ),
    responses(
        (status = 200, description = "Top rated movies", body = TmdbListResponse),
        (status = 500, description = "Internal error"),
    ),
)]
pub async fn movie_top_rated_handler(
    State(state): State<TmdbState>,
    Query(params): Query<PageQuery>,
) -> Response {
    match state.tmdb.movie_top_rated(params.page.unwrap_or(1)).await {
        Ok(items) => list_response(items),
        Err(_) => error_response(),
    }
}

#[utoipa::path(
    get,
    path = "/api/tmdb/movie/now-playing",
    params(
        ("page" = Option<u32>, Query, description = "Page number (default 1)"),
    ),
    responses(
        (status = 200, description = "Now playing movies", body = TmdbListResponse),
        (status = 500, description = "Internal error"),
    ),
)]
pub async fn movie_now_playing_handler(
    State(state): State<TmdbState>,
    Query(params): Query<PageQuery>,
) -> Response {
    match state.tmdb.movie_now_playing(params.page.unwrap_or(1)).await {
        Ok(items) => list_response(items),
        Err(_) => error_response(),
    }
}

#[utoipa::path(
    get,
    path = "/api/tmdb/tv/popular",
    params(
        ("page" = Option<u32>, Query, description = "Page number (default 1)"),
    ),
    responses(
        (status = 200, description = "Popular TV shows", body = TmdbListResponse),
        (status = 500, description = "Internal error"),
    ),
)]
pub async fn tv_popular_handler(
    State(state): State<TmdbState>,
    Query(params): Query<PageQuery>,
) -> Response {
    match state.tmdb.tv_popular(params.page.unwrap_or(1)).await {
        Ok(items) => list_response(items),
        Err(_) => error_response(),
    }
}

#[utoipa::path(
    get,
    path = "/api/tmdb/tv/top-rated",
    params(
        ("page" = Option<u32>, Query, description = "Page number (default 1)"),
    ),
    responses(
        (status = 200, description = "Top rated TV shows", body = TmdbListResponse),
        (status = 500, description = "Internal error"),
    ),
)]
pub async fn tv_top_rated_handler(
    State(state): State<TmdbState>,
    Query(params): Query<PageQuery>,
) -> Response {
    match state.tmdb.tv_top_rated(params.page.unwrap_or(1)).await {
        Ok(items) => list_response(items),
        Err(_) => error_response(),
    }
}

#[utoipa::path(
    get,
    path = "/api/tmdb/tv/on-the-air",
    params(
        ("page" = Option<u32>, Query, description = "Page number (default 1)"),
    ),
    responses(
        (status = 200, description = "On the air TV shows", body = TmdbListResponse),
        (status = 500, description = "Internal error"),
    ),
)]
pub async fn tv_on_the_air_handler(
    State(state): State<TmdbState>,
    Query(params): Query<PageQuery>,
) -> Response {
    match state.tmdb.tv_on_the_air(params.page.unwrap_or(1)).await {
        Ok(items) => list_response(items),
        Err(_) => error_response(),
    }
}

#[utoipa::path(
    get,
    path = "/api/tmdb/trending",
    params(
        ("type" = Option<String>, Query, description = "Media type: 'movie' (default) or 'tv'"),
        ("page" = Option<u32>, Query, description = "Page number (default 1)"),
    ),
    responses(
        (status = 200, description = "Trending items", body = TmdbListResponse),
        (status = 500, description = "Internal error"),
    ),
)]
pub async fn trending_handler(
    State(state): State<TmdbState>,
    Query(params): Query<TrendingQuery>,
) -> Response {
    let m_type = params.media_type.as_deref().unwrap_or("movie");
    match state.tmdb.trending(m_type, params.page.unwrap_or(1)).await {
        Ok(items) => list_response(items),
        Err(_) => error_response(),
    }
}

#[utoipa::path(
    get,
    path = "/api/tmdb/discover",
    params(
        ("type" = Option<String>, Query, description = "Media type: 'movie' (default) or 'tv'"),
        ("genre" = Option<String>, Query, description = "Genre id (required)"),
        ("page" = Option<u32>, Query, description = "Page number (default 1)"),
    ),
    responses(
        (status = 200, description = "Discovered items", body = TmdbListResponse),
        (status = 400, description = "Missing 'genre' parameter"),
        (status = 500, description = "Internal error"),
    ),
)]
pub async fn discover_handler(
    State(state): State<TmdbState>,
    Query(params): Query<DiscoverQuery>,
) -> Response {
    let m_type = params.media_type.as_deref().unwrap_or("movie");
    let genre_id = params.genre.as_deref().unwrap_or("");
    if genre_id.is_empty() {
        return (StatusCode::BAD_REQUEST, "Missing 'genre' parameter").into_response();
    }
    match state
        .tmdb
        .discover(m_type, genre_id, params.page.unwrap_or(1))
        .await
    {
        Ok(items) => list_response(items),
        Err(_) => error_response(),
    }
}

#[utoipa::path(
    get,
    path = "/api/tmdb/search",
    params(
        ("q" = String, Query, description = "Search query"),
        ("type" = Option<String>, Query, description = "Media type: 'movie' (default) or 'tv'"),
        ("year" = Option<u32>, Query, description = "Release/first-air year filter"),
    ),
    responses(
        (status = 200, description = "Search results", body = TmdbListResponse),
        (status = 500, description = "Internal error"),
    ),
)]
pub async fn search_handler(
    State(state): State<TmdbState>,
    Query(params): Query<SearchQuery>,
) -> Response {
    let m_type = params.media_type.as_deref().unwrap_or("movie");
    match state.tmdb.search(&params.q, m_type, params.year).await {
        Ok(items) => list_response(items),
        Err(_) => error_response(),
    }
}
