//! Hue Stream API
//! Professional Streaming Aggregator
//!
//! @author roywalk3r <roywalker440@gmail.com>
//! @repository https://github.com/roywalk3r/hue.git
//! @license MIT

use crate::app::AppState;
use crate::models::anime::StreamPayload;
use crate::providers::{
    animekill_provider::AnimeKillSearch, animexin_provider::AnimexinSearch,
};
use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use std::time::Duration;
use tokio::time::timeout;

#[derive(Deserialize)]
pub struct DonghuaSearchQuery {
    pub title: String,
    pub episode: u32,
}

const DONGHUA_TIMEOUT_SECS: u64 = 12;

#[utoipa::path(
    get,
    path = "/api/streams/animexin/search",
    params(
        ("title" = String, Query, description = "Donghua title to search"),
        ("episode" = u32, Query, description = "Episode number"),
    ),
    responses(
        (status = 200, description = "Stream payload for the resolved episode", body = StreamPayload),
        (status = 400, description = "Missing title or episode"),
        (status = 404, description = "No donghua match or episode not found"),
        (status = 504, description = "Upstream search timed out"),
        (status = 500, description = "Upstream error"),
    ),
)]
pub async fn animexin_search_handler(
    State(state): State<AppState>,
    Query(params): Query<DonghuaSearchQuery>,
) -> Response {
    if params.title.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "title is required" })),
        )
            .into_response();
    }

    let client = state.animexin.clone();
    match timeout(
        Duration::from_secs(DONGHUA_TIMEOUT_SECS),
        async move {
            let searcher = AnimexinSearch::new(client);
            searcher.search(&params.title, params.episode).await
        },
    )
    .await
    {
        Ok(Ok(payload)) => (StatusCode::OK, Json(payload)).into_response(),
        Ok(Err(e)) => {
            let msg = e.to_string();
            if msg.contains("no donghua match") || msg.contains("episode") && msg.contains("not found") {
                (
                    StatusCode::NOT_FOUND,
                    Json(serde_json::json!({ "error": msg })),
                )
                    .into_response()
            } else {
                tracing::warn!("[animexin] search error: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "error": msg })),
                )
                    .into_response()
            }
        }
        Err(_) => (
            StatusCode::GATEWAY_TIMEOUT,
            Json(serde_json::json!({
                "error": format!("animexin search timed out after {}s", DONGHUA_TIMEOUT_SECS)
            })),
        )
            .into_response(),
    }
}

#[utoipa::path(
    get,
    path = "/api/streams/animekill/search",
    params(
        ("title" = String, Query, description = "Donghua title to search"),
        ("episode" = u32, Query, description = "Episode number"),
    ),
    responses(
        (status = 200, description = "Stream payload for the resolved episode", body = StreamPayload),
        (status = 400, description = "Missing title or episode"),
        (status = 404, description = "No donghua match or episode not found"),
        (status = 504, description = "Upstream search timed out"),
        (status = 500, description = "Upstream error"),
    ),
)]
pub async fn animekill_search_handler(
    State(state): State<AppState>,
    Query(params): Query<DonghuaSearchQuery>,
) -> Response {
    if params.title.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "title is required" })),
        )
            .into_response();
    }

    let client = state.animekill.clone();
    match timeout(
        Duration::from_secs(DONGHUA_TIMEOUT_SECS),
        async move {
            let searcher = AnimeKillSearch::new(client);
            searcher.search(&params.title, params.episode).await
        },
    )
    .await
    {
        Ok(Ok(payload)) => (StatusCode::OK, Json(payload)).into_response(),
        Ok(Err(e)) => {
            let msg = e.to_string();
            if msg.contains("no donghua match") || msg.contains("episode") && msg.contains("not found") {
                (
                    StatusCode::NOT_FOUND,
                    Json(serde_json::json!({ "error": msg })),
                )
                    .into_response()
            } else {
                tracing::warn!("[animekill] search error: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "error": msg })),
                )
                    .into_response()
            }
        }
        Err(_) => (
            StatusCode::GATEWAY_TIMEOUT,
            Json(serde_json::json!({
                "error": format!("animekill search timed out after {}s", DONGHUA_TIMEOUT_SECS)
            })),
        )
            .into_response(),
    }
}
