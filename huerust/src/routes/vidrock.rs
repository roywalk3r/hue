/*
 * HUE ENGINE ❄
 * System Designation: The Architecture of Aggregation
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */

//! VidRock route handlers — direct access to AES-encrypted VidRock API

use axum::{
    extract::{Query, State, Path},
    response::{IntoResponse, Response},
    http::StatusCode,
};
use std::collections::HashMap;
use crate::app::AppState;
use tracing::{info, warn};

pub async fn streams_handler(
    State(state): State<AppState>,
    Path((media_type, tmdb_id)): Path<(String, String)>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    let season = params.get("season").and_then(|s| s.parse::<u32>().ok());
    let episode = params.get("episode").and_then(|s| s.parse::<u32>().ok());

    info!("[vidrock] Streams: type={} tmdb={} s={:?} e={:?}", media_type, tmdb_id, season, episode);

    match state.vidrock.get_streams(&tmdb_id, &media_type, season, episode).await {
        Ok(streams) => {
            (StatusCode::OK, serde_json::json!({
                "success": true,
                "count": streams.len(),
                "streams": streams,
            }).to_string()).into_response()
        }
        Err(e) => {
            warn!("[vidrock] Failed: {}", e);
            (StatusCode::BAD_GATEWAY, serde_json::json!({
                "success": false,
                "error": e.to_string(),
                "streams": []
            }).to_string()).into_response()
        }
    }
}
