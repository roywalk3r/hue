use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::clients::vidvault::VidVaultClient;

#[derive(Clone)]
pub struct DownloadState {
    pub vidvault: Arc<VidVaultClient>,
}

impl axum::extract::FromRef<crate::app::AppState> for DownloadState {
    fn from_ref(s: &crate::app::AppState) -> Self {
        Self { vidvault: s.vidvault.clone() }
    }
}

#[derive(Deserialize)]
pub struct DownloadQuery {
    pub season: Option<u32>,
    pub episode: Option<u32>,
}

pub async fn download_handler(
    State(state): State<DownloadState>,
    Path((media_type, tmdb_id)): Path<(String, String)>,
    Query(q): Query<DownloadQuery>,
) -> impl IntoResponse {
    let season = q.season.unwrap_or(1);
    let episode = q.episode.unwrap_or(1);

    match state.vidvault.get_download_files(&tmdb_id, &media_type, season, episode).await {
        Ok(links) => (StatusCode::OK, Json(serde_json::json!({
            "success": true,
            "files": links.files,
        }))).into_response(),
        Err(e) => (StatusCode::BAD_GATEWAY, Json(serde_json::json!({
            "success": false,
            "error": e.to_string(),
        }))).into_response(),
    }
}
