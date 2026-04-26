mod security;
mod tmdb;
mod moviebox;
mod hitv;
mod providers;
mod models;

use axum::{
    extract::{Query, State, Path},
    response::{IntoResponse, Response},
    routing::get,
    Router,
    middleware::{self, Next},
    http::{StatusCode, Request, header::{REFERER, CONTENT_TYPE, USER_AGENT, ORIGIN}},
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use tower_http::cors::CorsLayer;
use tracing::{info, error, warn};
use url::Url;
use axum::body::Body;
use std::collections::HashMap;
use std::env;
use dotenv::dotenv;
use std::sync::Arc;

use crate::tmdb::TmdbClient;
use crate::moviebox::MovieBoxClient;
use crate::hitv::HiTvClient;
use crate::providers::{
    vixsrc, 
    kisskh::KissKhClient, 
    sflix::SflixClient, 
    dramacool::DramacoolClient,
    nineanime::NineAnimeClient
};
use crate::models::{StreamResult, Subtitle};

#[derive(Clone)]
struct AppState {
    client: Client,
    tmdb: Arc<TmdbClient>,
    moviebox: Arc<MovieBoxClient>,
    kisskh: Arc<KissKhClient>,
    sflix: Arc<SflixClient>,
    dramacool: Arc<DramacoolClient>,
    nineanime: Arc<NineAnimeClient>,
    hitv: Arc<HiTvClient>,
    secret: String,
    proxy_base_url: String,
}

#[derive(Deserialize)]
struct ProxyQuery {
    h_token: String,
}

#[tokio::main]
async fn main() {
    dotenv().ok();
    tracing_subscriber::fmt::init();

    let secret = env::var("APP_SECRET").unwrap_or_else(|_| "hue-streaming-stable-secret-2026".to_string());
    let host = env::var("RUST_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port = env::var("RUST_PORT").unwrap_or_else(|_| "24609".to_string());
    let proxy_base_url = env::var("RUST_PROXY_URL").unwrap_or_else(|_| format!("http://{}:{}", host, port));

    let tmdb_keys = env::var("TMDB_API_KEYS")
        .unwrap_or_else(|_| env::var("TMDB_API_KEY").unwrap_or_default())
        .split(|c| c == ',' || c == ';' || c == '\n')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect::<Vec<String>>();

    let tmdb = Arc::new(TmdbClient::new(tmdb_keys));
    let state = AppState {
        client: Client::builder()
            .tcp_keepalive(std::time::Duration::from_secs(60))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
            .build()
            .unwrap(),
        tmdb: tmdb.clone(),
        moviebox: Arc::new(MovieBoxClient::new(None)),
        kisskh: Arc::new(KissKhClient::new()),
        sflix: Arc::new(SflixClient::new()),
        dramacool: Arc::new(DramacoolClient::new()),
        nineanime: Arc::new(NineAnimeClient::new()),
        hitv: Arc::new(HiTvClient::new(tmdb.clone())),
        secret,
        proxy_base_url,
    };

    // Background Task: Build HiTV Library every 12 hours
    let hitv_bg = state.hitv.clone();
    tokio::spawn(async move {
        loop {
            let _ = hitv_bg.build_library().await;
            tokio::time::sleep(std::time::Duration::from_secs(12 * 3600)).await;
        }
    });

    let app = Router::new()
        .route("/health", get(health_check))
        .route("/m3u8-proxy", get(m3u8_handler))
        .route("/ts-proxy", get(ts_handler))
        .route("/sub-proxy", get(sub_handler))
        .route("/api/hitv/library", get(hitv_library_handler))
        .route("/api/streams/:type/:tmdbId", get(streams_handler))
        .layer(middleware::from_fn_with_state(state.clone(), auth_middleware))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let addr_str = format!("{}:{}", host, port);
    let addr: SocketAddr = addr_str.parse().expect("Invalid RUST_HOST or RUST_PORT");
    info!("🚀 Huerust Engine Active on {}", addr);
    
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn health_check() -> &'static str { "Huerust is Alive 🦀" }

async fn auth_middleware(State(_state): State<AppState>, req: Request<Body>, next: Next) -> Result<Response, StatusCode> {
    let is_local = req.extensions().get::<SocketAddr>().map_or(false, |a| a.ip().is_loopback());
    if is_local { return Ok(next.run(req).await); }
    if let Ok(api_key) = env::var("API_KEY") {
        let auth = req.headers().get("x-api-key").and_then(|v| v.to_str().ok());
        if auth != Some(&api_key) { return Err(StatusCode::FORBIDDEN); }
    }
    Ok(next.run(req).await)
}

async fn hitv_library_handler(State(state): State<AppState>) -> Response {
    let items = state.hitv.get_library().await;
    (StatusCode::OK, serde_json::to_string(&items).unwrap()).into_response()
}

async fn vixsrc_api_handler(
    State(state): State<AppState>,
    Path((m_type, id)): Path<(String, String)>,
) -> Response {
    match vixsrc::get_vixsrc_streams(&state.client, &m_type, &id).await {
        Ok(msg) => (StatusCode::OK, msg).into_response(),
        Err(e) => (StatusCode::BAD_GATEWAY, e.to_string()).into_response()
    }
}

async fn streams_handler(
    State(state): State<AppState>,
    Path((m_type, tmdb_id)): Path<(String, String)>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    let season = params.get("season").and_then(|s| s.parse::<u32>().ok()).unwrap_or(1);
    let episode = params.get("episode").and_then(|e| e.parse::<u32>().ok()).unwrap_or(1);
    
    info!("[api] Rust Streams: {} {} S{} E{}", &m_type, tmdb_id, season, episode);

    let tmdb_info = match state.tmdb.get_details(&m_type, tmdb_id.parse().unwrap_or(0)).await {
        Ok(info) => Some(info),
        Err(_) => None,
    };

    let target_title = tmdb_info.as_ref().map(|i| i.title.clone()).unwrap_or_default();
    if target_title.is_empty() {
        return (StatusCode::NOT_FOUND, "TMDB info not found").into_response();
    }

    let moviebox = state.moviebox.clone();
    let kisskh = state.kisskh.clone();
    let sflix = state.sflix.clone();
    
    let tid_box = tmdb_id.clone();
    let title_kh = target_title.clone();
    let title_sf = target_title.clone();
    let mtype_sf = m_type.clone();
    
    let cinema_task = tokio::spawn(async move {
        moviebox.get_streams_normalized(&tid_box, "", season, episode).await
            .unwrap_or_default()
    });

    let zenith_task = tokio::spawn(async move {
        kisskh.get_streams_normalized(&title_kh, season, episode).await
            .unwrap_or_default()
    });

    let nova_task = tokio::spawn(async move {
        sflix.get_streams_normalized(&title_sf, &mtype_sf, season, episode).await
            .unwrap_or_default()
    });

    let results = tokio::select! {
        res = async {
            let mut all = Vec::new();
            if let Ok(r) = cinema_task.await { all.extend(r); }
            if let Ok(r) = zenith_task.await { all.extend(r); }
            if let Ok(r) = nova_task.await { all.extend(r); }
            all
        } => res,
        _ = tokio::time::sleep(std::time::Duration::from_secs(35)) => {
            warn!("[api] Stream aggregation timed out");
            vec![]
        }
    };

    if results.is_empty() {
        return (StatusCode::NOT_FOUND, "No streams found").into_response();
    }

    (StatusCode::OK, serde_json::to_string(&results).unwrap()).into_response()
}

// --- Proxy Handlers ---
async fn m3u8_handler(State(state): State<AppState>, Query(params): Query<ProxyQuery>) -> Response {
    let data = match security::decrypt_url(&params.h_token, &state.secret) {
        Some(d) => d,
        None => return (StatusCode::FORBIDDEN, "Invalid Token").into_response(),
    };
    let req = state.client.get(&data.url);
    let req = inject_cdn_headers(req, &data.url);
    match req.send().await {
        Ok(resp) => {
            let body = resp.text().await.unwrap_or_default();
            let rewritten = rewrite_playlist(&body, &data.url, &state.proxy_base_url, &state.secret, data.headers);
            Response::builder()
                .header(CONTENT_TYPE, "application/vnd.apple.mpegurl")
                .header("Access-Control-Allow-Origin", "*")
                .body(Body::from(rewritten))
                .unwrap()
        }
        Err(e) => (StatusCode::BAD_GATEWAY, e.to_string()).into_response(),
    }
}

async fn ts_handler(State(state): State<AppState>, Query(params): Query<ProxyQuery>) -> Response {
    let data = match security::decrypt_url(&params.h_token, &state.secret) {
        Some(d) => d,
        None => return (StatusCode::FORBIDDEN, "Invalid Token").into_response(),
    };
    let req = state.client.get(&data.url);
    let req = inject_cdn_headers(req, &data.url);
    match req.send().await {
        Ok(resp) => {
            let status = StatusCode::from_u16(resp.status().as_u16()).unwrap_or(StatusCode::OK);
            let mut builder = Response::builder().status(status);
            if let Some(ct) = resp.headers().get("content-type") {
                if let Ok(ct_str) = ct.to_str() {
                    builder = builder.header("content-type", ct_str);
                }
            }
            builder = builder.header("Access-Control-Allow-Origin", "*");
            builder.body(Body::from_stream(resp.bytes_stream())).unwrap()
        }
        Err(e) => (StatusCode::BAD_GATEWAY, e.to_string()).into_response(),
    }
}

async fn sub_handler(State(state): State<AppState>, Query(params): Query<ProxyQuery>) -> Response {
    let data = match security::decrypt_url(&params.h_token, &state.secret) {
        Some(d) => d,
        None => return (StatusCode::FORBIDDEN, "Invalid Token").into_response(),
    };
    let req = state.client.get(&data.url);
    let req = inject_cdn_headers(req, &data.url);
    match req.send().await {
        Ok(resp) => {
            let body = resp.text().await.unwrap_or_default();
            let final_body = if data.url.to_lowercase().contains(".srt") || body.trim().starts_with("1") { 
                format!("WEBVTT\n\n{}", body) 
            } else { 
                body 
            };
            Response::builder()
                .header("content-type", "text/vtt")
                .header("Access-Control-Allow-Origin", "*")
                .body(Body::from(final_body))
                .unwrap()
        }
        Err(e) => (StatusCode::BAD_GATEWAY, e.to_string()).into_response(),
    }
}

fn inject_cdn_headers(mut req: reqwest::RequestBuilder, target_url: &str) -> reqwest::RequestBuilder {
    let host = match Url::parse(target_url) { 
        Ok(u) => u.host_str().unwrap_or("").to_string(), 
        Err(_) => return req 
    };
    
    let referer = if host.contains("vixsrc.to") { 
        Some("https://vixsrc.to/") 
    } else if host.contains("hakunaymatata.com") || host.contains("aisports.mobi") || host.contains("poocloud.in") { 
        Some("https://moviebox.ph/") 
    } else if host.contains("mewstream.buzz") || host.contains("animeplay.cfd") { 
        Some("https://hianimes.se/") 
    } else { 
        None 
    };

    if let Some(r) = referer { 
        req = req.header("Referer", r); 
        if let Ok(origin_url) = Url::parse(r) { 
            req = req.header("Origin", origin_url.origin().unicode_serialization()); 
        } 
    }
    
    req.header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
}

fn rewrite_playlist(content: &str, target_url: &str, proxy_base: &str, secret: &str, headers: Option<HashMap<String, String>>) -> String {
    let base_url = match Url::parse(target_url) { Ok(u) => u, Err(_) => return content.to_string() };
    let query_str = base_url.query().map(|q| format!("?{}", q)).unwrap_or_default();
    content.lines().map(|line| {
        if line.starts_with("#") {
            if line.contains("URI=\"") {
                let parts: Vec<&str> = line.split("URI=\"").collect();
                if parts.len() > 1 {
                    let uri_parts: Vec<&str> = parts[1].split("\"").collect();
                    let relative_uri = uri_parts[0];
                    let abs_url = base_url.join(relative_uri).map(|u| u.to_string()).unwrap_or_else(|_| relative_uri.to_string());
                    let final_url = if abs_url.contains('?') { abs_url } else { format!("{}{}", abs_url, query_str) };
                    let token = security::encrypt_url(&final_url, secret, headers.clone());
                    let is_media = line.contains("EXT-X-MEDIA");
                    let route = if is_media { "m3u8-proxy" } else { "ts-proxy" };
                    return line.replace(relative_uri, &format!("{}/{}?h_token={}", proxy_base, route, token));
                }
            }
            line.to_string()
        } else if !line.trim().is_empty() {
            let abs_url = base_url.join(line.trim()).map(|u| u.to_string()).unwrap_or_else(|_| line.trim().to_string());
            let final_url = if abs_url.contains('?') { abs_url.clone() } else { format!("{}{}", abs_url, query_str) };
            let token = security::encrypt_url(&final_url, secret, headers.clone());
            let is_hls = abs_url.contains(".m3u8");
            let route = if is_hls { "m3u8-proxy" } else { "ts-proxy" };
            format!("{}/{}?h_token={}", proxy_base, route, token)
        } else { line.to_string() }
    }).collect::<Vec<_>>().join("\n")
}
