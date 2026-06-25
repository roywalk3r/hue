use crate::clients::tmdb::TmdbClient;
use crate::clients::vidvault::VidVaultClient;
use axum::{
    body::{Body, Bytes},
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::AsyncReadExt;
use tokio_stream::wrappers::ReceiverStream;

struct CleanupGuard {
    path: String,
    armed: bool,
}

impl CleanupGuard {
    fn new(path: String) -> Self {
        Self { path, armed: true }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for CleanupGuard {
    fn drop(&mut self) {
        if self.armed {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

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

fn build_ffmpeg_args(target_url: &str, header_str: &str, output: &str) -> Vec<String> {
    let mut args = vec!["-y".to_string()];
    if !header_str.is_empty() {
        let headers = if header_str.ends_with("\r\n") {
            header_str.to_string()
        } else {
            format!("{}\r\n", header_str)
        };
        args.extend(["-headers".to_string(), headers]);
    }
    args.extend([
        "-i".to_string(),
        target_url.to_string(),
        "-c".to_string(),
        "copy".to_string(),
        output.to_string(),
    ]);
    args
}

/// Validate that `source` is a URL on our own proxy origin. We only accept
/// our own `/m3u8-proxy` or `/ts-proxy` endpoints here; raw upstream URLs
/// must come via an encrypted `h_token` so headers stay attached.
fn validated_proxy_source(source: &str, proxy_base_url: &str) -> Option<String> {
    let source_url = url::Url::parse(source).ok()?;
    let proxy_url = url::Url::parse(proxy_base_url).ok()?;
    let same_origin = source_url.scheme() == proxy_url.scheme()
        && source_url.host_str() == proxy_url.host_str()
        && source_url.port_or_known_default() == proxy_url.port_or_known_default();
    let media_proxy = matches!(source_url.path(), "/m3u8-proxy" | "/ts-proxy");
    (same_origin && media_proxy).then(|| source.to_string())
}

/// Convert a backend proxy URL (`/m3u8-proxy` or `/ts-proxy`) into the real
/// upstream target URL and headers. Supports both the signed `h_token` form
/// and the plain `url`+`headers` form used by anime streams. ffmpeg needs the
/// real upstream URL, not our proxy endpoint.
fn resolve_upstream_from_proxy_url(
    source: &str,
    secret: &str,
) -> Option<(String, Option<HashMap<String, String>>)> {
    let url = url::Url::parse(source).ok()?;

    // Only resolve our own media proxy routes.
    if !matches!(url.path(), "/m3u8-proxy" | "/ts-proxy") {
        return None;
    }

    // Signed h_token form (VidVault / encrypted sources).
    if let Some((_, token)) = url.query_pairs().find(|(k, _)| k == "h_token") {
        let data = crate::security::decrypt_url(&token, secret)?;
        return Some((data.url, data.headers));
    }

    // Plain url+headers form (anime / public proxy sources).
    let upstream = url
        .query_pairs()
        .find(|(k, _)| k == "url")
        .map(|(_, v)| v.to_string())?;
    let headers = url
        .query_pairs()
        .find(|(k, _)| k == "headers")
        .and_then(|(_, v)| serde_json::from_str::<HashMap<String, String>>(&v).ok());
    Some((upstream, headers))
}

fn build_background_download_proxy_url(
    source_url: &str,
    headers: HashMap<String, String>,
    proxy_base_url: &str,
    secret: &str,
) -> String {
    let token = crate::security::encrypt_url(source_url, secret, Some(headers));
    format!(
        "{}/ts-proxy?h_token={}&no_synth=1",
        proxy_base_url.trim_end_matches('/'),
        token
    )
}

#[cfg(test)]
mod tests {
    use super::{build_background_download_proxy_url, build_ffmpeg_args, validated_proxy_source};

    #[test]
    fn ffmpeg_headers_are_input_options() {
        let args = build_ffmpeg_args(
            "https://cdn.example.com/master.m3u8",
            "Referer: https://example.com/\r\n",
            "/tmp/output.mp4",
        );

        let headers = args.iter().position(|arg| arg == "-headers").unwrap();
        let input = args.iter().position(|arg| arg == "-i").unwrap();
        assert!(headers < input, "headers must be applied before the input");
    }

    #[test]
    fn hls_source_accepts_only_our_media_proxy() {
        let base = "https://api.vexli.online";
        assert!(
            validated_proxy_source("https://api.vexli.online/m3u8-proxy?h_token=abc", base,)
                .is_some()
        );
        assert!(validated_proxy_source("https://evil.example/stream.m3u8", base).is_none());
        assert!(validated_proxy_source("https://api.vexli.online/api/admin", base).is_none());
    }

    #[test]
    fn header_protected_downloads_use_the_signed_backend_proxy() {
        let mut headers = std::collections::HashMap::new();
        headers.insert("Referer".to_string(), "https://vidvault.ru/".to_string());
        headers.insert("Origin".to_string(), "https://vidvault.ru".to_string());

        let url = build_background_download_proxy_url(
            "https://mkv.example.workers.dev/d/file",
            headers,
            "https://api.vexli.online",
            "test-secret",
        );

        assert!(url.starts_with("https://api.vexli.online/ts-proxy?h_token="));
        assert!(url.contains("no_synth=1"));
        assert!(!url.contains("mkv.example.workers.dev"));
        assert!(!url.contains("download=1"));
    }
}

pub async fn download_handler(
    State(state): State<DownloadState>,
    request_headers: HeaderMap,
    Path((media_type, tmdb_id)): Path<(String, String)>,
    Query(q): Query<DownloadQuery>,
) -> impl IntoResponse {
    let season = q.season.unwrap_or(1);
    let episode = q.episode.unwrap_or(1);
    let public_proxy_base =
        crate::proxy::dynamic_proxy_base(&request_headers, &state.proxy_base_url);

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

            let vidvault_url = format!("https://vidvault.ru/{}/{}", media_type, tmdb_id);

            let files: Vec<DownloadFileOut> = links
                .files
                .into_iter()
                .map(|f| {
                    let ext = f.format.clone();
                    let year_str = if year > 0 {
                        format!(" ({})", year)
                    } else {
                        String::new()
                    };
                    let base = match media_type.as_str() {
                        "tv" => format!(
                            "{}{} S{:02}E{:02}.{}",
                            show_name, year_str, season, episode, ext
                        ),
                        _ => format!("{}{}.{}", show_name, year_str, ext),
                    };
                    let filename = sanitize_filename(&base);

                    if f.format == "mkv" {
                        let proxied = build_background_download_proxy_url(
                            &f.url,
                            mkv_headers.clone(),
                            &public_proxy_base,
                            &state.secret,
                        );
                        DownloadFileOut {
                            url: proxied,
                            size: f.size,
                            format: f.format,
                            filename,
                            headers: None,
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
                            headers
                                .insert("Referer".to_string(), "https://vidvault.ru/".to_string());
                            headers.insert("Origin".to_string(), "https://vidvault.ru".to_string());
                            let token =
                                crate::security::encrypt_url(&f.url, &state.secret, Some(headers));
                            let proxied = format!(
                                "{}/ts-proxy?h_token={}&download=1&filename={}",
                                public_proxy_base,
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
                Json(serde_json::json!({ "success": true, "files": files, "captions": links.captions })),
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

// ── HLS → MP4 streaming download ─────────────────────────────────────────

#[derive(Deserialize)]
pub struct HlsDownloadQuery {
    pub h_token: Option<String>,
    pub source: Option<String>,
    #[serde(default)]
    pub filename: Option<String>,
}

pub async fn hls_download_handler(
    State(state): State<DownloadState>,
    Query(params): Query<HlsDownloadQuery>,
) -> impl IntoResponse {
    use tokio::process::Command as TokioCommand;

    let (target_url, token_headers) = match params.h_token.as_deref() {
        Some(token) => match crate::security::decrypt_url(token, &state.secret) {
            Some(data) => (data.url, data.headers),
            None => return (StatusCode::FORBIDDEN, "Invalid token").into_response(),
        },
        None => match params.source.as_deref() {
            Some(source) => {
                // If the client sent a proxied URL, decrypt it to the real
                // upstream target so ffmpeg talks to the CDN directly.
                if let Some((url, headers)) =
                    resolve_upstream_from_proxy_url(source, &state.secret)
                {
                    (url, headers)
                } else if validated_proxy_source(source, &state.proxy_base_url).is_some() {
                    (source.to_string(), None)
                } else {
                    return (StatusCode::BAD_REQUEST, "Missing download source").into_response();
                }
            }
            None => return (StatusCode::BAD_REQUEST, "Missing download source").into_response(),
        },
    };

    let header_str = token_headers
        .as_ref()
        .map(|h| {
            h.iter()
                .map(|(k, v)| format!("{}: {}", k, v))
                .collect::<Vec<_>>()
                .join("\r\n")
        })
        .unwrap_or_default();

    let filename = params
        .filename
        .as_deref()
        .unwrap_or("download.mp4")
        .replace(
            |c: char| !c.is_ascii_alphanumeric() && c != '.' && c != '-' && c != '_',
            "_",
        );

    let tmp = format!("/tmp/hue_dl_{}.mp4", ulid::Ulid::new());
    // Ensure temp file cleanup even on early return or panic.
    let mut guard = CleanupGuard::new(tmp.clone());
    let mut cmd = TokioCommand::new("ffmpeg");
    cmd.args(build_ffmpeg_args(&target_url, &header_str, &tmp));
    let out = match cmd
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
    {
        Ok(c) => match c.wait_with_output().await {
            Ok(o) => o,
            Err(e) => {
                tracing::error!("[download/hls] wait: {}", e);
                return (StatusCode::INTERNAL_SERVER_ERROR, "ffmpeg error").into_response();
            }
        },
        Err(e) => {
            tracing::error!("[download/hls] spawn: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "ffmpeg not available").into_response();
        }
    };
    if !out.status.success() {
        let stderr_str = String::from_utf8_lossy(&out.stderr);
        let last = stderr_str.lines().last().unwrap_or("unknown");
        // Log enough of stderr to diagnose failures without dumping
        // the entire manifest in normal operation.
        let preview = stderr_str
            .chars()
            .take(1200)
            .collect::<String>();
        tracing::warn!(
            "[download/hls] ffmpeg failed for {}: {} | stderr preview: {}",
            target_url,
            last,
            preview.replace('\n', " | ")
        );
        let _ = std::fs::remove_file(&tmp);
        return (StatusCode::BAD_GATEWAY, "HLS conversion failed").into_response();
    }
    let mut file = match tokio::fs::File::open(&tmp).await {
        Ok(file) => file,
        Err(e) => {
            tracing::error!("[download/hls] open: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "read failed").into_response();
        }
    };
    let content_length = match file.metadata().await {
        Ok(metadata) => metadata.len(),
        Err(e) => {
            tracing::error!("[download/hls] metadata: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "read failed").into_response();
        }
    };

    let (tx, rx) = tokio::sync::mpsc::channel::<Result<Bytes, std::io::Error>>(8);
    let cleanup_path = tmp.clone();
    tokio::spawn(async move {
        let mut buffer = vec![0_u8; 64 * 1024];
        loop {
            match file.read(&mut buffer).await {
                Ok(0) => break,
                Ok(read) => {
                    if tx
                        .send(Ok(Bytes::copy_from_slice(&buffer[..read])))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                Err(e) => {
                    let _ = tx.send(Err(e)).await;
                    break;
                }
            }
        }
        let _ = tokio::fs::remove_file(cleanup_path).await;
    });
    guard.disarm();

    Response::builder()
        .status(StatusCode::OK)
        .header(axum::http::header::CONTENT_TYPE, "video/mp4")
        .header(axum::http::header::CONTENT_LENGTH, content_length)
        .header(
            axum::http::header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", filename),
        )
        .body(Body::from_stream(ReceiverStream::new(rx)))
        .unwrap()
}
