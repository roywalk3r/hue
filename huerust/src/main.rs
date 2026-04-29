//! Hue Stream API
//! Professional Streaming Aggregator
//!
//! @author roywalk3r <roywalker440@gmail.com>
//! @repository https://github.com/roywalk3r/hue.git
//! @license MIT

mod security;
mod tmdb;
mod moviebox;
mod hitv;
mod providers;
mod models;
mod cache;
mod registry;
mod filters;
mod circuit_breaker;
mod routes;
mod microservice;
mod config;
mod proxy;
mod user_config;
mod coalesce;
mod metrics;
mod openapi;

use axum::{
    extract::State,
    response::{IntoResponse, Response},
    routing::get,
    Router,
    middleware::{self, Next},
    http::{StatusCode, Request},
};
use reqwest::Client;
use std::net::SocketAddr;
use tower_http::cors::CorsLayer;
use tracing::info;
use axum::body::Body;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;
use std::env;
use dotenv::dotenv;
use std::sync::Arc;
use std::time::Duration;
use dashmap::DashMap;
use std::time::Instant;

use crate::tmdb::TmdbClient;
use crate::moviebox::MovieBoxClient;
use crate::hitv::HiTvClient;
use crate::cache::StreamCache;
use crate::coalesce::FetchCoalescer;
use crate::registry::ProviderRegistry;
use crate::filters::FilterConfig;
use crate::circuit_breaker::CircuitBreaker;
use crate::metrics::Metrics;
use crate::providers::{

    moviebox_provider::MovieBoxProvider,
    kisskh_provider::KissKhProvider,
    sflix_provider::SflixProvider,
    dramacool_provider::DramacoolProvider,
    vixsrc_provider::VixsrcProvider,
    mp4hydra_provider::Mp4HydraProvider,
    vixsrc::VixsrcClient,
    kisskh::KissKhClient,
    dramacool::DramacoolClient,
};

#[derive(Clone)]
struct RateLimitState {
    store: Arc<DashMap<String, (usize, Instant)>>,
    max_requests: usize,
    window_secs: u64,
}

impl RateLimitState {
    fn new(max_requests: usize, window_secs: u64) -> Self {
        Self {
            store: Arc::new(DashMap::new()),
            max_requests,
            window_secs,
        }
    }

    fn check(&self, key: &str) -> bool {
        let now = Instant::now();
        let window = Duration::from_secs(self.window_secs);

        if let Some(mut entry) = self.store.get_mut(key) {
            let (count, start) = *entry.value();
            if now.duration_since(start) > window {
                // New window
                *entry.value_mut() = (1, now);
                return true;
            }
            if count >= self.max_requests {
                return false;
            }
            *entry.value_mut() = (count + 1, start);
            true
        } else {
            self.store.insert(key.to_string(), (1, now));
            true
        }
    }
}

#[derive(Clone)]
struct AppState {
    client: Client,
    tmdb: Arc<TmdbClient>,
    registry: Arc<ProviderRegistry>,
    cache: Arc<StreamCache>,
    filter_config: Arc<FilterConfig>,
    hitv: Arc<HiTvClient>,
    kisskh: Arc<KissKhClient>,
    dramacool: Arc<DramacoolClient>,
    secret: String,
    proxy_base_url: String,
    config: Arc<crate::config::AppConfig>,
    rate_limiter: RateLimitState,
    coalescer: Arc<FetchCoalescer>,
    metrics: Arc<Metrics>,
}

#[tokio::main]
async fn main() {
    dotenv().ok();

    // CLI handling
    let args: Vec<String> = env::args().collect();
    if args.len() > 1 && args[1] == "--encrypt" {
        if args.len() < 3 {
            println!("Usage: huerust --encrypt <url> [referer]");
            return;
        }
        let url = &args[2];
        let referer = args.get(3).cloned();
        let cfg = config::AppConfig::from_env();
        let mut headers = std::collections::HashMap::new();
        if let Some(r) = referer {
            headers.insert("Referer".to_string(), r);
        }
        let token = security::encrypt_url(url, &cfg.secret, if headers.is_empty() { None } else { Some(headers) });
        println!("{}", token);
        return;
    }

    tracing_subscriber::fmt::init();

    let cfg = config::AppConfig::from_env();

    let tmdb = Arc::new(TmdbClient::new(cfg.tmdb_api_keys.clone()));
    let cache = Arc::new(StreamCache::new_from_config(&cfg,
    ).await);
    let circuit_breaker = Arc::new(CircuitBreaker::new());
    let metrics = Arc::new(Metrics::new(&["Cinema", "Nova", "Zenith", "Orient", "Aura", "Hydra"]));
    circuit_breaker.set_metrics(metrics.clone());
    let filter_config = Arc::new(FilterConfig::default());

    let mut registry = ProviderRegistry::new(circuit_breaker.clone());
    let moviebox_client = Arc::new(MovieBoxClient::new(None));
    let kisskh_client = Arc::new(crate::providers::kisskh::KissKhClient::new());
    let sflix_client = Arc::new(crate::providers::sflix::SflixClient::new());
    let dramacool_client = Arc::new(crate::providers::dramacool::DramacoolClient::new());
    let vixsrc_client = Arc::new(VixsrcClient::new(
        Client::builder()
            .tcp_keepalive(Duration::from_secs(60))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
            .build()
            .unwrap(),
    ));

    registry.register(Arc::new(MovieBoxProvider::new(moviebox_client.clone())));
    registry.register(Arc::new(KissKhProvider::new(kisskh_client.clone())));
    registry.register(Arc::new(SflixProvider::new(sflix_client.clone())));
    registry.register(Arc::new(DramacoolProvider::new(dramacool_client.clone())));
    registry.register(Arc::new(VixsrcProvider::new(vixsrc_client)));
    registry.register(Arc::new(Mp4HydraProvider::new()));

    let state = AppState {
        client: Client::builder()
            .tcp_keepalive(Duration::from_secs(60))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
            .build()
            .unwrap(),
        tmdb: tmdb.clone(),
        registry: Arc::new(registry),
        cache: cache.clone(),
        filter_config: filter_config.clone(),
        hitv: Arc::new(HiTvClient::new(tmdb.clone())),
        kisskh: kisskh_client,
        dramacool: dramacool_client,
        secret: cfg.secret.clone(),
        proxy_base_url: cfg.proxy_base_url.clone(),
        config: Arc::new(cfg.clone()),
        rate_limiter: RateLimitState::new(120, 60),
        coalescer: Arc::new(FetchCoalescer::new()),
        metrics: metrics.clone(),
    };

    // Background Task: Build HiTV Library every 12 hours
    let hitv_bg = state.hitv.clone();
    tokio::spawn(async move {
        loop {
            let _ = hitv_bg.build_library().await;
            tokio::time::sleep(Duration::from_secs(12 * 3600)).await;
        }
    });

    let app = Router::new()
        .route("/health", get(health_check))
        .route("/metrics", get(metrics_handler))
        .route("/m3u8-proxy", get(proxy::m3u8_handler))
        .route("/ts-proxy", get(proxy::ts_handler))
        .route("/sub-proxy", get(proxy::sub_handler))
        .route("/api/hitv/library", get(hitv_library_handler))
        .route("/api/hitv/browse", get(routes::hitv::browse_handler))
        .route("/api/hitv/search", get(routes::hitv::search_handler))
        .route("/api/hitv/latest", get(routes::hitv::latest_handler))
        .route("/api/hitv/trending", get(routes::hitv::trending_handler))
        .route("/api/hitv/hero", get(routes::hitv::hero_handler))
        .route("/api/hitv/drama", get(routes::hitv::drama_handler))
        .route("/api/hitv/info/:id", get(routes::hitv::info_handler))
        .route("/api/hitv/play/:id/:episode", get(routes::hitv::play_handler))
        .route("/api/hitv/countries", get(routes::hitv::countries_handler))
        .route("/api/hitv/categories", get(routes::hitv::categories_handler))
        .route("/api/hitv/genres", get(routes::hitv::genres_handler))
        .route("/api/hitv/sorts", get(routes::hitv::sorts_handler))
        .route("/api/hitv/:country/:sort/:genre", get(routes::hitv::dynamic_browse_handler))
        .route("/api/streams/:type/:tmdbId", get(routes::streams::streams_handler))
        .route("/api/streams/:type/:tmdbId/live", get(routes::sse::sse_streams_handler))
        .route("/api/providers", get(routes::providers::providers_handler))
        .route("/api/admin/reload", axum::routing::post(admin_reload_handler))
        .route("/api/play/:type/:tmdbId", get(routes::play::play_handler))
        .route("/api/drama/kisskh/search", get(routes::drama::kisskh_search_handler))
        .route("/api/drama/kisskh/streams/:id/:episode", get(routes::drama::kisskh_streams_handler))
        .route("/api/drama/dramacool/search", get(routes::drama::dramacool_search_handler))
        .route("/api/drama/dramacool/streams/:id/:episode", get(routes::drama::dramacool_streams_handler))
        .merge(SwaggerUi::new("/api/docs").url("/api/docs/openapi.json", openapi::ApiDoc::openapi()))
        .layer(middleware::from_fn_with_state(state.clone(), rate_limit_middleware))
        .layer(middleware::from_fn_with_state(state.clone(), auth_middleware))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let addr_str = format!("{}:{}", cfg.host, cfg.port);
    let addr: SocketAddr = addr_str.parse().expect("Invalid RUST_HOST or RUST_PORT");
    info!("Huerust Engine Active on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    info!("Huerust Engine listening on {}", addr);

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();

    info!("Huerust Engine shutdown complete");
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    info!("Shutdown signal received, starting graceful shutdown...");
}

async fn health_check() -> &'static str { "Huerust is Alive" }

async fn auth_middleware(State(_state): State<AppState>, req: Request<Body>, next: Next) -> Result<Response, StatusCode> {
    let is_local = req.extensions().get::<SocketAddr>().map_or(false, |a| a.ip().is_loopback());
    if is_local { return Ok(next.run(req).await); }
    if let Ok(api_key) = env::var("API_KEY") {
        let auth = req.headers().get("x-api-key").and_then(|v| v.to_str().ok());
        if auth != Some(&api_key) { return Err(StatusCode::FORBIDDEN); }
    }
    Ok(next.run(req).await)
}

async fn rate_limit_middleware(State(state): State<AppState>, req: Request<Body>, next: Next) -> Result<Response, StatusCode> {
    let ip = req.extensions()
        .get::<axum::extract::ConnectInfo<SocketAddr>>()
        .map(|c| c.0.ip().to_string())
        .or_else(|| req.headers().get("x-forwarded-for").and_then(|v| v.to_str().ok()).map(|s| s.split(',').next().unwrap_or(s).trim().to_string()))
        .unwrap_or_else(|| "unknown".to_string());

    if !state.rate_limiter.check(&ip) {
        return Ok((StatusCode::TOO_MANY_REQUESTS, "Rate limit exceeded").into_response());
    }
    Ok(next.run(req).await)
}

async fn metrics_handler(State(state): State<AppState>) -> Response {
    let body = state.metrics.render_prometheus();
    (
        StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        body,
    ).into_response()
}

async fn hitv_library_handler(State(state): State<AppState>) -> Response {
    let items = state.hitv.get_library().await;
    (StatusCode::OK, serde_json::to_string(&items).unwrap()).into_response()
}

async fn admin_reload_handler() -> Response {
    crate::user_config::HueUserConfig::invalidate();
    (StatusCode::OK, "{\"ok\":true,\"message\":\"Config cache invalidated — next request reloads from disk\"}").into_response()
}

// --- Handlers moved to src/routes/ ---
