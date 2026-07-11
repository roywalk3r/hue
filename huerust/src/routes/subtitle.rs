//! Subtitle routes — Xyra-backed OpenSubtitles search and download.
//!
//! Endpoints:
//!   GET /api/subtitle/search?imdbid|tmdb_id|query[&type=&lang=&season=&episode=]
//!   GET /api/subtitle/download?url=...

use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Deserialize;

use crate::app::AppState;

#[derive(Debug, Deserialize)]
pub struct SubtitleSearchQuery {
    pub imdbid: Option<String>,
    #[serde(rename = "tmdb_id")]
    pub tmdb_id: Option<u64>,
    pub query: Option<String>,
    #[serde(rename = "type")]
    pub content_type: Option<String>,
    pub lang: Option<String>,
    pub season: Option<u32>,
    pub episode: Option<u32>,
}

#[utoipa::path(
    get,
    path = "/api/subtitle/search",
    params(
        ("imdbid" = Option<String>, Query, description = "IMDb ID (with or without 'tt' prefix)"),
        ("tmdb_id" = Option<u64>, Query, description = "TMDB numeric ID"),
        ("query" = Option<String>, Query, description = "Free-text search query"),
        ("type" = Option<String>, Query, description = "Content type: movie or tv"),
        ("lang" = Option<String>, Query, description = "Language code, e.g. eng"),
        ("season" = Option<u32>, Query, description = "Season number (tv only)"),
        ("episode" = Option<u32>, Query, description = "Episode number (tv only)"),
    ),
    responses(
        (status = 200, description = "Subtitle search results", body = serde_json::Value),
        (status = 500, description = "Internal error"),
    ),
)]
pub async fn subtitle_search_handler(
    State(state): State<AppState>,
    Query(params): Query<SubtitleSearchQuery>,
) -> Response {
    tracing::info!(
        "[subtitle] search: imdbid={:?} tmdb_id={:?} query={:?} type={:?} lang={:?} s={:?} e={:?}",
        params.imdbid,
        params.tmdb_id,
        params.query,
        params.content_type,
        params.lang,
        params.season,
        params.episode,
    );
    match state
        .xyra_subtitle
        .search(
            params.imdbid.as_deref(),
            params.tmdb_id,
            params.query.as_deref(),
            params.content_type.as_deref(),
            params.lang.as_deref(),
            params.season,
            params.episode,
        )
        .await
    {
        Ok(items) => {
            tracing::info!("[subtitle] search: {} results", items.len());
            (StatusCode::OK, Json(items)).into_response()
        }
        Err(e) => {
            tracing::warn!("[subtitle] search error: {}", e);
            let body = serde_json::json!({ "success": false, "error": e.to_string() });
            (StatusCode::INTERNAL_SERVER_ERROR, body.to_string()).into_response()
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct SubtitleDownloadQuery {
    pub url: String,
}

#[utoipa::path(
    get,
    path = "/api/subtitle/download",
    params(
        ("url" = String, Query, description = "Subtitle download URL returned from /api/subtitle/search"),
    ),
    responses(
        (status = 200, description = "Raw subtitle text (SRT/VTT)", body = String),
        (status = 500, description = "Internal error"),
    ),
)]
pub async fn subtitle_download_handler(
    State(state): State<AppState>,
    Query(params): Query<SubtitleDownloadQuery>,
) -> Response {
    tracing::info!("[subtitle] download: url={}", params.url);
    match state.xyra_subtitle.download(&params.url).await {
        Ok(text) => {
            // xyra decompresses gzip and returns raw subtitle text. Pass
            // through to the client as-is. Default content type is fine —
            // SRT/VTT are both plain text.
            (
                StatusCode::OK,
                [(
                    axum::http::header::CONTENT_TYPE,
                    "text/plain; charset=utf-8",
                )],
                text,
            )
                .into_response()
        }
        Err(e) => {
            tracing::warn!("[subtitle] download error: {}", e);
            let body = serde_json::json!({ "success": false, "error": e.to_string() });
            (StatusCode::INTERNAL_SERVER_ERROR, body.to_string()).into_response()
        }
    }
}
