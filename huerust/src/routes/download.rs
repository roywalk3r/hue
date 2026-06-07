use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use crate::clients::vidvault::VidVaultClient;

#[derive(Clone)]
pub struct DownloadState {
    pub vidvault: Arc<VidVaultClient>,
    pub proxy_base_url: String,
    pub secret: String,
}

impl axum::extract::FromRef<crate::app::AppState> for DownloadState {
    fn from_ref(s: &crate::app::AppState) -> Self {
        Self {
            vidvault: s.vidvault.clone(),
            proxy_base_url: s.proxy_base_url.clone(),
            secret: s.secret.clone(),
        }
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
        Ok(links) => {
            let mut headers = HashMap::new();
            headers.insert("Referer".to_string(), "https://vidvault.ru/".to_string());
            headers.insert("Origin".to_string(), "https://vidvault.ru".to_string());

            let files: Vec<serde_json::Value> = links.files.into_iter().map(|f| {
                let token = crate::security::encrypt_url(&f.url, &state.secret, Some(headers.clone()));
                let proxied = format!("{}/ts-proxy?h_token={}", state.proxy_base_url, token);
                serde_json::json!({ "url": proxied, "size": f.size, "format": f.format })
            }).collect();

            (StatusCode::OK, Json(serde_json::json!({ "success": true, "files": files }))).into_response()
        }
        Err(e) => (StatusCode::BAD_GATEWAY, Json(serde_json::json!({
            "success": false,
            "error": e.to_string(),
        }))).into_response(),
    }
}
