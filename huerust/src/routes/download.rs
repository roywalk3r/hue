use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use crate::clients::tmdb::TmdbClient;
use crate::clients::vidvault::VidVaultClient;

#[derive(Clone)]
pub struct DownloadState {
    pub vidvault: Arc<VidVaultClient>,
    pub tmdb: Arc<TmdbClient>,
    pub proxy_base_url: String,
    pub secret: String,
}

impl axum::extract::FromRef<crate::app::AppState> for DownloadState {
    fn from_ref(s: &crate::app::AppState) -> Self {
        Self {
            vidvault: s.vidvault.clone(),
            tmdb: s.tmdb.clone(),
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

#[derive(Serialize)]
struct DownloadFileOut {
    url: String,
    size: String,
    format: String,
    filename: String,
    headers: Option<HashMap<String, String>>,
    downloadable: bool,
}

fn sanitize_filename(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c => c,
        })
        .collect()
}

pub async fn download_handler(
    State(state): State<DownloadState>,
    Path((media_type, tmdb_id)): Path<(String, String)>,
    Query(q): Query<DownloadQuery>,
) -> impl IntoResponse {
    let season = q.season.unwrap_or(1);
    let episode = q.episode.unwrap_or(1);

    let (show_name, year) = state
        .tmdb
        .get_details(&media_type, tmdb_id.parse().unwrap_or(0))
        .await
        .map(|info| (info.title, info.year.unwrap_or(0)))
        .unwrap_or_else(|_| ("download".to_string(), 0));

    let result = state
        .vidvault
        .get_download_files(&tmdb_id, &media_type, season, episode)
        .await;

    match result {
        Ok(links) => {
            let mut mkv_headers = HashMap::new();
            mkv_headers.insert("Referer".to_string(), "https://vidvault.ru/".to_string());
            mkv_headers.insert("Origin".to_string(), "https://vidvault.ru".to_string());

            // hakunaymatata.com CDN blocks direct/proxied downloads (403).
            // MKV files from workers.dev CDN work with direct URL + headers.
            // MP4 files from blocked CDNs link to VidVault's own page instead.
            let blocked_hosts = ["hakunaymatata.com", "vidsrc.xyz"];

            let vidvault_url = format!(
                "https://vidvault.ru/{}/{}",
                media_type, tmdb_id
            );

            let files: Vec<DownloadFileOut> = links
                .files
                .into_iter()
                .map(|f| {
                    let ext = f.format.clone();
                    let year_str = if year > 0 { format!(" ({})", year) } else { String::new() };
                    let base = match media_type.as_str() {
                        "tv" => format!(
                            "{}{} S{:02}E{:02}.{}",
                            show_name, year_str, season, episode, ext
                        ),
                        _ => format!("{}{}.{}", show_name, year_str, ext),
                    };
                    let filename = sanitize_filename(&base);

                    if f.format == "mkv" {
                        DownloadFileOut {
                            url: f.url,
                            size: f.size,
                            format: f.format,
                            filename,
                            headers: Some(mkv_headers.clone()),
                            downloadable: true,
                        }
                    } else {
                        let url_lower = f.url.to_lowercase();
                        let is_blocked = blocked_hosts.iter().any(|h| url_lower.contains(h));
                        if is_blocked {
                            // CDN blocks direct access — link to VidVault page instead
                            DownloadFileOut {
                                url: vidvault_url.clone(),
                                size: f.size,
                                format: f.format,
                                filename,
                                headers: None,
                                downloadable: false,
                            }
                        } else {
                            // MP4 from accessible CDN — proxy through ts-proxy
                            let mut headers = HashMap::new();
                            headers.insert("Referer".to_string(), "https://vidvault.ru/".to_string());
                            headers.insert("Origin".to_string(), "https://vidvault.ru".to_string());
                            let token = crate::security::encrypt_url(
                                &f.url,
                                &state.secret,
                                Some(headers),
                            );
                            let proxied = format!(
                                "{}/ts-proxy?h_token={}&download=1&filename={}",
                                state.proxy_base_url,
                                token,
                                urlencoding::encode(&filename),
                            );
                            DownloadFileOut {
                                url: proxied,
                                size: f.size,
                                format: f.format,
                                filename,
                                headers: None,
                                downloadable: true,
                            }
                        }
                    }
                })
                .collect();

            (
                StatusCode::OK,
                Json(serde_json::json!({ "success": true, "files": files })),
            )
                .into_response()
        }
        Err(e) => (
            StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({
                "success": false,
                "error": e.to_string(),
            })),
        )
            .into_response(),
    }
}
