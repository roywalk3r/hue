//! Hue Stream API — Sports Routes
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
use serde_json::json;
use tracing::{info, warn};
use utoipa::ToSchema;
use validator::Validate;

use crate::app::AppState;

#[derive(serde::Serialize, ToSchema)]
pub struct LeaguesResponse {
    pub success: bool,
    pub list: Vec<crate::clients::aisports::League>,
}

#[derive(Deserialize, ToSchema, Validate)]
pub struct SportsStreamQuery {
    #[validate(length(min = 1, max = 200))]
    pub q: String,
}

#[derive(serde::Serialize, ToSchema)]
pub struct SportsStreamsResponse {
    pub success: bool,
    pub query: String,
    pub count: usize,
    pub streams: Vec<crate::clients::aisports::SportsStream>,
}

#[utoipa::path(
    get,
    path = "/api/aisports/leagues",
    responses(
        (status = 200, description = "AISports leagues", body = LeaguesResponse),
    ),
)]
pub async fn leagues_handler(
    State(state): State<AppState>,
) -> Response {
    match state.aisports.get_leagues().await {
        Ok(list) => {
            (
                StatusCode::OK,
                serde_json::to_string(&LeaguesResponse { success: true, list }).unwrap(),
            )
                .into_response()
        }
        Err(e) => {
            warn!("[api] AiSports leagues failed: {}", e);
            (
                StatusCode::OK,
                serde_json::to_string(&LeaguesResponse { success: true, list: vec![] }).unwrap(),
            )
                .into_response()
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/sports/streams",
    params(
        ("q" = String, Query, description = "Match name or team name to search for"),
    ),
    responses(
        (status = 200, description = "Sports stream results", body = SportsStreamsResponse),
        (status = 404, description = "No streams found"),
    ),
)]
pub async fn streams_handler(
    State(state): State<AppState>,
    Query(params): Query<SportsStreamQuery>,
) -> Response {
    if let Err(e) = params.validate() {
        return (
            StatusCode::BAD_REQUEST,
            json!({"success": false, "error": format!("Invalid query: {}", e)}).to_string(),
        )
            .into_response();
    }

    let query = params.q.trim();
    if query.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            json!({"success": false, "error": "Query parameter 'q' is required"}).to_string(),
        )
            .into_response();
    }

    info!("[api] Sports Streams: search='{}'", query);

    match state.aisports.search_streams(query).await {
        Ok(streams) if !streams.is_empty() => {
            (
                StatusCode::OK,
                serde_json::to_string(&SportsStreamsResponse {
                    success: true,
                    query: query.to_string(),
                    count: streams.len(),
                    streams,
                })
                .unwrap(),
            )
                .into_response()
        }
        Ok(_) => {
            info!("[api] No sports streams found for '{}'", query);
            (
                StatusCode::OK,
                serde_json::to_string(&SportsStreamsResponse {
                    success: true,
                    query: query.to_string(),
                    count: 0,
                    streams: vec![],
                })
                .unwrap(),
            )
                .into_response()
        }
        Err(e) => {
            warn!("[api] AiSports search failed: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                json!({"success": false, "error": e.to_string()}).to_string(),
            )
                .into_response()
        }
    }
}
