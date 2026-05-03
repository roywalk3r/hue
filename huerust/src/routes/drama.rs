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
    Json,
};
use serde::Deserialize;

use crate::{
    AppState,
    models::ErrorResponse,
};

#[derive(Deserialize)]
pub struct SearchQuery {
    pub q: String,
}

pub async fn kisskh_search_handler(
    State(state): State<AppState>,
    Query(params): Query<SearchQuery>,
) -> Response {
    match state.kisskh.search(&params.q).await {
        Ok(items) => (StatusCode::OK, serde_json::to_string(&items).unwrap()).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
            success: false,
            error: "INTERNAL_ERROR".to_string(),
            message: Some(e.to_string()),
        })).into_response(),
    }
}

pub async fn kisskh_streams_handler(
    State(state): State<AppState>,
    Path((id, episode)): Path<(String, u32)>,
) -> Response {
    match state.kisskh.get_streams_normalized(&id, 1, episode).await {
        Ok(streams) => (StatusCode::OK, serde_json::to_string(&streams).unwrap()).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
            success: false,
            error: "INTERNAL_ERROR".to_string(),
            message: Some(e.to_string()),
        })).into_response(),
    }
}

pub async fn dramacool_search_handler(
    State(state): State<AppState>,
    Query(params): Query<SearchQuery>,
) -> Response {
    match state.dramacool.search(&params.q).await {
        Ok(items) => (StatusCode::OK, serde_json::to_string(&items).unwrap()).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
            success: false,
            error: "INTERNAL_ERROR".to_string(),
            message: Some(e.to_string()),
        })).into_response(),
    }
}

pub async fn dramacool_streams_handler(
    State(state): State<AppState>,
    Path((id, episode)): Path<(String, u32)>,
) -> Response {
    match state.dramacool.get_streams_normalized(&id, 1, episode).await {
        Ok(streams) => (StatusCode::OK, serde_json::to_string(&streams).unwrap()).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
            success: false,
            error: "INTERNAL_ERROR".to_string(),
            message: Some(e.to_string()),
        })).into_response(),
    }
}
