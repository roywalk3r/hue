use axum::body::Body;
use axum::{
    extract::State,
    http::{Request, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use reqwest_middleware::ClientWithMiddleware;
use std::env;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tower_http::compression::CompressionLayer;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use utoipa::OpenApi;
use utoipa_scalar::Scalar;

use crate::clients::hitv::HiTvClient;
use crate::clients::moviebox::MovieBoxClient;
use crate::clients::tmdb::TmdbClient;
use crate::config::AppConfig;
use crate::filters::FilterConfig;
use crate::infra::cache::StreamCache;
use crate::infra::circuit_breaker::CircuitBreaker;
use crate::infra::coalesce::FetchCoalescer;
use crate::infra::metrics::Metrics;
use crate::infra::provider_score::ProviderScoreCache;
use crate::infra::rate_limit::RateLimitState;
use crate::providers::registry::ProviderRegistry;
use crate::providers::{
    dramacool::DramacoolClient,
    dramacool_provider::DramacoolProvider,
    kissasian::KissAsianClient,
    kissasian_provider::KissAsianProvider,
    kissasian_top::KissAsianTopClient,
    kissasian_top_provider::KissAsianTopProvider,
    kisskh::KissKhClient,
    kisskh_provider::KissKhProvider,
    kisskh_xyra::KissKhXyraClient,
    kisskh_xyra_provider::KissKhXyraProvider,
    // lordflix_provider::LordflixProvider, // disabled — upstream requires Cloudflare attestation
    moviebox_provider::MovieBoxProvider,
    notorrent_provider::NoTorrentProvider,
    // mp4hydra_provider::Mp4HydraProvider, // disabled — endpoint times out
    sflix_provider::SflixProvider,
    // streamhub::StreamHubClient,      // disabled — upstream returns Aura duplicates
    // streamhub_provider::StreamHubProvider,
    vidlink_provider::VidlinkProvider,
    // videasy::VideasyClient,       // disabled — upstream 500
    // videasy_provider::VideasyProvider,
    vidrock_provider::VidrockProvider,
    vidsrcembed_provider::VidsrcEmbedProvider,
    // vidvault_provider::VidvaultProvider, // disabled — hakunaymatata CDN 403s all URLs, even freshly resolved
    vidzee::VidzeeClient,
    vidzee_provider::VidzeeProvider,
    vixsrc::VixsrcClient,
    vixsrc_provider::VixsrcProvider,
    xyra_subtitle::XyraSubtitleClient,
    xyra_vixsrc::XyraVixsrcClient,
    xyra_vixsrc_provider::XyraVixsrcProvider,
    xyra_aurex::XyraAurexClient,
    xyra_aurex_provider::XyraAurexProvider,
};

pub fn kisskh_enabled() -> bool {
    env::var("ENABLE_KISSKH")
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

/// VixSrc (Aura) is off by default: vixsrc.to's Cloudflare bot management
/// 403s datacenter IPs (the production VPS), so it returns nothing there and a
/// residential proxy to bypass it isn't worth the cost. It still works from a
/// clean/residential IP, so a deploy on one can re-enable it with
/// `ENABLE_VIXSRC=true`.
pub fn vixsrc_enabled() -> bool {
    env::var("ENABLE_VIXSRC")
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

/// Aura (Vixsrc) via the xyra.stream streamhub proxy. On by default since the
/// direct `vixsrc.to` path is Cloudflare-blocked on the production VPS, but
/// Xyra can proxy the same master playlist. Set `ENABLE_XYRA_VIXSRC=0` to
/// opt out and run with no Aura streams.
pub fn xyra_vixsrc_enabled() -> bool {
    env::var("ENABLE_XYRA_VIXSRC")
        .map(|value| {
            !matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "0" | "false" | "no" | "off"
            )
        })
        .unwrap_or(true)
}

/// Force IPv4 egress on the segment proxy. Off by default: a host with a
/// working IPv6 route (e.g. the production VPS reaching Cloudflare workers)
/// should use its normal path — pinning to IPv4 there routes segments over a
/// slower link and makes playback rebuffer. Only set this on a host whose IPv6
/// route is broken (reqwest hangs on the AAAA address until timeout), where
/// forcing IPv4 is the fix.
pub fn force_ipv4_egress() -> bool {
    env::var("FORCE_IPV4_EGRESS")
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

#[derive(Clone)]
pub struct AppState {
    pub client: ClientWithMiddleware,
    pub raw_client: reqwest::Client,
    pub tmdb: Arc<TmdbClient>,
    pub registry: Arc<ProviderRegistry>,
    pub cache: Arc<StreamCache>,
    pub sticky_pref: Arc<crate::infra::sticky_pref::StickyPrefCache>,
    pub provider_score: Arc<ProviderScoreCache>,
    pub filter_config: Arc<FilterConfig>,
    pub hitv: Arc<HiTvClient>,
    pub kisskh: Arc<KissKhClient>,
    pub dramacool: Arc<DramacoolClient>,
    pub kissasian: Arc<KissAsianClient>,
    pub kissasian_top: Arc<KissAsianTopClient>,
    pub moviebox: Arc<MovieBoxClient>,
    pub hianime: Arc<crate::clients::hianime::HianimeClient>,
    pub vidvault: Arc<crate::clients::vidvault::VidVaultClient>,
    pub aisports: Arc<crate::clients::aisports::AiSportsClient>,
    pub anime_agg: Arc<crate::clients::anime_agg::AnimeAggregator>,
    pub animekill: Arc<crate::clients::animekill::AnimeKillClient>,
    pub xyra_subtitle: Arc<XyraSubtitleClient>,
    pub kisskh_xyra: Arc<KissKhXyraClient>,
    pub secret: String,
    pub kkey: String,
    pub proxy_base_url: String,
    pub config: Arc<crate::config::AppConfig>,
    pub rate_limiter: RateLimitState,
    pub coalescer: Arc<FetchCoalescer>,
    pub metrics: Arc<Metrics>,
    pub start_time: std::time::Instant,
    #[allow(dead_code)]
    pub redis_pool: Option<deadpool_redis::Pool>,
}

pub async fn build_state(cfg: &crate::config::AppConfig) -> AppState {
    let tmdb = Arc::new(TmdbClient::new(cfg.tmdb_api_keys.clone()));
    let cache = Arc::new(StreamCache::new_from_config(&cfg).await);
    let circuit_breaker = Arc::new(CircuitBreaker::new());
    let metrics = Arc::new(Metrics::new(&[
        "Cinema", "Nova", "Zenith", "Orient", "Pearl", "Aura", "Hydra", "Luna", "Vault", "VidZee",
        "Nebula", "Rose", "Aurex",
    ]));
    circuit_breaker.set_metrics(metrics.clone());
    let filter_config = Arc::new(FilterConfig::default());

    let mut registry = ProviderRegistry::new(circuit_breaker.clone());
    let moviebox_client = Arc::new(MovieBoxClient::new(None));
    let kisskh_client = Arc::new(crate::providers::kisskh::KissKhClient::new());
    let sflix_client = Arc::new(crate::providers::sflix::SflixClient::new());
    let dramacool_client = Arc::new(crate::providers::dramacool::DramacoolClient::new());
    let kissasian_client = Arc::new(crate::providers::kissasian::KissAsianClient::new());
    let kissasian_top_client = Arc::new(crate::providers::kissasian_top::KissAsianTopClient::new());
    let hianime_client = Arc::new(crate::clients::hianime::HianimeClient::new());
    let animekill_client = Arc::new(crate::clients::animekill::AnimeKillClient::new());
    let xyra_subtitle_client = Arc::new(XyraSubtitleClient::new());
    let kisskh_xyra_client = Arc::new(KissKhXyraClient::new());
    let anime_agg = Arc::new(crate::clients::anime_agg::AnimeAggregator::new(
        Arc::new(crate::clients::aniwave::AniwaveClient::new(
            cfg.cf_worker_url.clone(),
            cfg.hianime_mirror_url.clone(),
        )),
        hianime_client.clone(),
        Arc::new(crate::clients::aniwatch::AniWatchClient::new()),
        Arc::new(crate::clients::anilist::AniListClient::new()),
        Arc::new(crate::clients::animexin::AnimexinClient::new()),
        animekill_client.clone(),
    ));
    let mut vixsrc_builder = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(5))
        // Fresh connection per call — vixsrc is hit at most once per request, so
        // pooling only risks reusing a stale CDN socket.
        .pool_max_idle_per_host(0)
        // Under the 8s provider deadline so a stall fails fast and frees the
        // dispatch slot instead of being cancelled mid-flight.
        .timeout(Duration::from_secs(7))
        .http1_only()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/137.0.0.0 Safari/537.36");
    if force_ipv4_egress() {
        // Only when the host's IPv6 route is broken (reqwest hangs on the AAAA
        // address). A host with working IPv6 should use its normal path.
        vixsrc_builder =
            vixsrc_builder.local_address(std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED));
    }
    let vixsrc_client = Arc::new(VixsrcClient::new(
        crate::http_client::build_client_with_retry(vixsrc_builder, "VIXSRC_PROXY_URL"),
    ));
    let xyra_vixsrc_client = Arc::new(XyraVixsrcClient::new());
    let xyra_aurex_client = Arc::new(XyraAurexClient::new());

    let vidrock_client = Arc::new(crate::clients::vidrock::VidRockClient::new());
    let vidvault_client = Arc::new(crate::clients::vidvault::VidVaultClient::new());
    let vidsrcembed_client = Arc::new(crate::clients::vidsrcembed::VidsrcEmbedClient::new());
    let vidzee_client = Arc::new(VidzeeClient::new(crate::http_client::wrap_with_retry(
        reqwest::Client::builder()
            .tcp_keepalive(Duration::from_secs(60))
            .connect_timeout(Duration::from_secs(8))
            .timeout(Duration::from_secs(20))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
            .build()
            .unwrap(),
    )));
    // let _videasy_client = Arc::new(VideasyClient::new()); // disabled — upstream 500
    let vidlink_client = Arc::new(crate::clients::vidlink::VidlinkClient::new(
        crate::http_client::wrap_with_retry(
            reqwest::Client::builder()
                .tcp_keepalive(Duration::from_secs(60))
                .connect_timeout(Duration::from_secs(8))
                .timeout(Duration::from_secs(15))
                .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/123.0.0.0 Safari/537.36")
                .build()
                .unwrap(),
        ),
    ));
    // Lordflix disabled — upstream requires Cloudflare attestation (x-attest header).
    // let lordflix_client = Arc::new(crate::clients::lordflix::LordflixClient::new(...));
    let notorrent_client = Arc::new(crate::clients::notorrent::NoTorrentClient::new(
        crate::http_client::wrap_with_retry(
            reqwest::Client::builder()
                .tcp_keepalive(Duration::from_secs(60))
                .connect_timeout(Duration::from_secs(8))
                .timeout(Duration::from_secs(20))
                .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/137.0.0.0 Safari/537.36")
                .build()
                .unwrap(),
        ),
    ));

    registry.register(Arc::new(MovieBoxProvider::new(moviebox_client.clone())));
    if kisskh_enabled() {
        registry.register(Arc::new(KissKhProvider::new(kisskh_client.clone())));
    } else {
        tracing::info!("KissKH provider disabled; set ENABLE_KISSKH=true to enable it");
    }
    registry.register(Arc::new(SflixProvider::new(sflix_client.clone())));
    // registry.register(Arc::new(StreamHubProvider::new(streamhub_client.clone()))); // disabled — upstream returns Aura duplicates
    // registry.register(Arc::new(KissKhXyraProvider::new(kisskh_xyra_client.clone()))); // disabled — xyra faulty
    // registry.register(Arc::new(DramacoolProvider::new(dramacool_client.clone()))); // disabled — xyra faulty
    registry.register(Arc::new(KissAsianProvider::new(kissasian_client.clone())));
    registry.register(Arc::new(KissAsianTopProvider::new(
        kissasian_top_client.clone(),
    )));
    if xyra_vixsrc_enabled() {
        registry.register(Arc::new(XyraVixsrcProvider::new(xyra_vixsrc_client)));
        let _ = vixsrc_client;
    } else if vixsrc_enabled() {
        registry.register(Arc::new(VixsrcProvider::new(vixsrc_client)));
    } else {
        let _ = vixsrc_client;
        tracing::info!(
            "Aura (Vixsrc) provider disabled — both Xyra and direct paths off (set ENABLE_XYRA_VIXSRC=1 to re-enable)"
        );
    }
    registry.register(Arc::new(XyraAurexProvider::new(xyra_aurex_client)));
    // registry.register(Arc::new(Mp4HydraProvider::new())); // disabled — endpoint times out
    registry.register(Arc::new(VidrockProvider::new(vidrock_client.clone())));
    // registry.register(Arc::new(VidvaultProvider::new(vidvault_client.clone()))); // disabled — hakunaymatata CDN 403s all URLs, even freshly resolved
    registry.register(Arc::new(VidzeeProvider::new(vidzee_client.clone())));
    // registry.register(Arc::new(VideasyProvider::new(videasy_client.clone()))); // disabled — upstream 500
    registry.register(Arc::new(VidsrcEmbedProvider::new(
        vidsrcembed_client.clone(),
    )));
    registry.register(Arc::new(VidlinkProvider::new(vidlink_client)));
    // registry.register(Arc::new(LordflixProvider::new(lordflix_client))); // disabled — upstream requires Cloudflare attestation
    registry.register(Arc::new(NoTorrentProvider::new(notorrent_client)));

    let redis_pool = if cfg.cache_backend == "redis" {
        let pool_cfg = deadpool_redis::Config::from_url(&cfg.redis_url);
        pool_cfg
            .create_pool(Some(deadpool_redis::Runtime::Tokio1))
            .ok()
    } else {
        None
    };

    let mut raw_builder = reqwest::Client::builder()
        .tcp_keepalive(Duration::from_secs(60))
        .tcp_nodelay(true)
        .timeout(Duration::from_secs(600))
        .connect_timeout(Duration::from_secs(10))
        .pool_max_idle_per_host(32)
        .pool_idle_timeout(Duration::from_secs(120))
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36");
    if force_ipv4_egress() {
        // Only for hosts whose IPv6 route is broken (reqwest hangs on the AAAA
        // address). On a host with working IPv6, pinning IPv4 slows segments.
        raw_builder =
            raw_builder.local_address(std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED));
    }
    // CF-blocked hosts (kisskh manifest/sub CDN, vixsrc) route through the clean-IP
    // pool; R2 segments and every other provider stay direct. Inert unless CF_PROXY_URLS
    // is set. Applied before the global STREAM_PROXY_URL so CF hosts prefer the pool.
    let raw_builder = crate::http_client::with_cf_proxy_pool(raw_builder);
    let raw_client = crate::http_client::with_optional_proxy(raw_builder, "STREAM_PROXY_URL")
        .build()
        .unwrap();

    AppState {
        client: crate::http_client::wrap_with_retry(raw_client.clone()),
        raw_client,
        tmdb: tmdb.clone(),
        registry: Arc::new(registry),
        cache: cache.clone(),
        sticky_pref: Arc::new(crate::infra::sticky_pref::StickyPrefCache::new_from_config(&cfg).await),
        provider_score: Arc::new(ProviderScoreCache::new_from_config(&cfg).await),
        filter_config: filter_config.clone(),
        hitv: Arc::new(HiTvClient::new(tmdb.clone())),
        kisskh: kisskh_client,
        dramacool: dramacool_client,
        kissasian: kissasian_client,
        kissasian_top: kissasian_top_client,
        moviebox: moviebox_client.clone(),
        hianime: hianime_client.clone(),
        vidvault: vidvault_client.clone(),
        anime_agg: anime_agg.clone(),
        animekill: animekill_client.clone(),
        xyra_subtitle: xyra_subtitle_client,
        kisskh_xyra: kisskh_xyra_client,
        aisports: Arc::new(crate::clients::aisports::AiSportsClient::new(
            crate::http_client::wrap_with_retry(
                reqwest::Client::builder()
                    .tcp_keepalive(Duration::from_secs(60))
                    .connect_timeout(Duration::from_secs(8))
                    .timeout(Duration::from_secs(15))
                    .user_agent("Mozilla/5.0 (iPhone; CPU iPhone OS 18_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.5 Mobile/15E148 Safari/604.1")
                    .build()
                    .unwrap(),
            ),
        )),
        secret: cfg.secret.clone(),
        kkey: cfg.kkey.clone(),
        proxy_base_url: cfg.proxy_base_url.clone(),
        config: Arc::new(cfg.clone()),
        rate_limiter: RateLimitState::new(300, 60),
        coalescer: Arc::new(FetchCoalescer::new()),
        metrics: metrics.clone(),
        start_time: std::time::Instant::now(),
        redis_pool,
    }
}

pub fn spawn_background_tasks(state: &AppState) {
    // Background Task: Build HiTV Library every 12 hours
    let hitv_bg = state.hitv.clone();
    tokio::spawn(async move {
        loop {
            let _ = hitv_bg.build_library().await;
            tokio::time::sleep(Duration::from_secs(12 * 3600)).await;
        }
    });

    // Background Task: keep the kisskh catalog warm so the common browsing path
    // never triggers a user-facing CF-proxy hit. Loads the persisted snapshot at
    // boot (restart-safe), then re-warms hot paths every 3h and re-snapshots.
    if kisskh_enabled() {
        let kisskh_bg = state.kisskh.clone();
        tokio::spawn(async move {
            kisskh_bg.load_catalog_snapshot().await;
            loop {
                kisskh_bg.warm_catalog().await;
                tokio::time::sleep(Duration::from_secs(3 * 3600)).await;
            }
        });
    }

    // governor handles its own key expiration — no manual eviction needed
}

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(crate::routes::system::health_check))
        .route(
            "/health/version",
            get(crate::routes::system::health_version),
        )
        .route("/metrics", get(crate::routes::system::metrics_handler))
        .route("/api/proxy", get(crate::proxy::generic_proxy_handler))
        .route("/m3u8-proxy", get(crate::proxy::m3u8_handler))
        .route("/ts-proxy", get(crate::proxy::ts_handler))
        .route("/sub-proxy", get(crate::proxy::sub_handler))
        .route("/storage/enc.key", get(vixsrc_enc_key_handler))
        .route(
            "/api/hitv/library",
            get(crate::routes::system::hitv_library_handler),
        )
        .route("/api/hitv/browse", get(crate::routes::hitv::browse_handler))
        .route("/api/hitv/search", get(crate::routes::hitv::search_handler))
        .route("/api/hitv/latest", get(crate::routes::hitv::latest_handler))
        .route(
            "/api/hitv/trending",
            get(crate::routes::hitv::trending_handler),
        )
        .route("/api/hitv/hero", get(crate::routes::hitv::hero_handler))
        .route("/api/hitv/drama", get(crate::routes::hitv::drama_handler))
        .route("/api/hitv/info/:id", get(crate::routes::hitv::info_handler))
        .route(
            "/api/hitv/play/:id/:episode",
            get(crate::routes::hitv::play_handler),
        )
        .route(
            "/api/hitv/countries",
            get(crate::routes::hitv::countries_handler),
        )
        .route(
            "/api/hitv/categories",
            get(crate::routes::hitv::categories_handler),
        )
        .route("/api/hitv/genres", get(crate::routes::hitv::genres_handler))
        .route("/api/hitv/sorts", get(crate::routes::hitv::sorts_handler))
        .route(
            "/api/hitv/:country/:sort/:genre",
            get(crate::routes::hitv::dynamic_browse_handler),
        )
        .route(
            "/api/download/:type/:tmdbId",
            get(crate::routes::download::download_handler),
        )
        .route(
            "/api/download/hls",
            get(crate::routes::download::hls_download_handler),
        )
        .route(
            "/api/streams/:type/:tmdbId",
            get(crate::routes::streams::streams_handler),
        )
        .route(
            "/api/streams/luna/:type/:tmdbId",
            get(crate::routes::streams::luna_streams_handler),
        )
        .route(
            "/api/streams/xyra/:type/:tmdbId",
            get(crate::routes::streams::xyra_streams_handler),
        )
        .route(
            "/api/streams/:type/:tmdbId/live",
            get(crate::routes::sse::sse_streams_handler),
        )
        .route(
            "/api/streams/:type/:tmdbId/report-failure",
            axum::routing::post(crate::routes::sse::report_failure_handler),
        )
        .route(
            "/api/warm/:type/:tmdbId",
            get(crate::routes::sse::warm_handler),
        )
        .route(
            "/api/providers",
            get(crate::routes::providers::providers_handler),
        )
        .route(
            "/api/admin/reload",
            axum::routing::post(crate::routes::system::admin_reload_handler),
        )
        .route(
            "/api/admin/refresh-tokens",
            axum::routing::post(crate::routes::system::admin_refresh_tokens_handler),
        )
        .route(
            "/api/play/:type/:tmdbId",
            get(crate::routes::play::play_handler),
        )
        .route(
            "/api/drama/kisskh/search",
            get(crate::routes::drama::kisskh_search_handler),
        )
        .route(
            "/api/drama/kisskh/show",
            get(crate::routes::drama::kisskh_show_handler),
        )
        .route(
            "/api/drama/kisskh/browse",
            get(crate::routes::drama::kisskh_browse_handler),
        )
        .route(
            "/api/drama/kisskh/upcoming",
            get(crate::routes::drama::kisskh_upcoming_handler),
        )
        .route(
            "/api/drama/kisskh/top-rating",
            get(crate::routes::drama::kisskh_top_rating_handler),
        )
        .route(
            "/api/drama/kisskh/most-view",
            get(crate::routes::drama::kisskh_most_view_handler),
        )
        .route(
            "/api/drama/kisskh/last-update",
            get(crate::routes::drama::kisskh_last_update_handler),
        )
        .route(
            "/api/drama/kisskh/detail/:id",
            get(crate::routes::drama::kisskh_detail_handler),
        )
        .route(
            "/api/drama/kisskh/streams/:id/:episode",
            get(crate::routes::drama::kisskh_streams_handler),
        )
        // Xyra-based drama routes disabled — xyra faulty
        // .route(
        //     "/api/drama/dramacool/search",
        //     get(crate::routes::drama::dramacool_search_handler),
        // )
        // .route(
        //     "/api/drama/dramacool/streams/:id/:episode",
        //     get(crate::routes::drama::dramacool_streams_handler),
        // )
        // .route(
        //     "/api/drama/kisskh_xyra/search",
        //     get(crate::routes::drama::kisskh_xyra_search_handler),
        // )
        // .route(
        //     "/api/drama/kisskh_xyra/streams/:id/:episode",
        //     get(crate::routes::drama::kisskh_xyra_streams_handler),
        // )
        // .route(
        //     "/api/drama/kisskh_xyra/v2/streams/:id/:episode",
        //     get(crate::routes::drama::kisskh_xyra_streams_handler),
        // )
        // .route(
        //     "/api/drama/kisskh_xyra/home",
        //     get(crate::routes::drama::kisskh_xyra_home_handler),
        // )
        .route(
            "/api/drama/kissasian/search",
            get(crate::routes::drama::kissasian_search_handler),
        )
        .route(
            "/api/drama/kissasian/streams/:id/:episode",
            get(crate::routes::drama::kissasian_streams_handler),
        )
        .route(
            "/api/drama/kissasian_top/search",
            get(crate::routes::drama::kissasian_top_search_handler),
        )
        .route(
            "/api/drama/kissasian_top/browse",
            get(crate::routes::drama::kissasian_top_browse_handler),
        )
        .route(
            "/api/drama/kissasian_top/info/:slug",
            get(crate::routes::drama::kissasian_top_info_handler),
        )
        .route(
            "/api/drama/kissasian_top/streams/:slug/:episode",
            get(crate::routes::drama::kissasian_top_streams_handler),
        )
        .route(
            "/api/drama/moviebox/browse",
            get(crate::routes::drama::moviebox_browse_handler),
        )
        .route(
            "/api/drama/moviebox/search",
            get(crate::routes::drama::moviebox_search_handler),
        )
        .route(
            "/api/drama/moviebox/streams/:title/:episode",
            get(crate::routes::drama::moviebox_streams_handler),
        )
        .route(
            "/api/drama/moviebox/streams-direct",
            get(crate::routes::drama::moviebox_streams_direct_handler),
        )
        .route(
            "/api/drama/moviebox/detail-rec",
            get(crate::routes::drama::moviebox_detail_rec_handler),
        )
        .route(
            "/api/drama/detail",
            get(crate::routes::drama::drama_detail_handler),
        )
        // Xyra subtitle routes disabled — xyra faulty
        // .route(
        //     "/api/subtitle/search",
        //     get(crate::routes::subtitle::subtitle_search_handler),
        // )
        // .route(
        //     "/api/subtitle/download",
        //     get(crate::routes::subtitle::subtitle_download_handler),
        // )
        .route(
            "/api/moviebox/home",
            get(crate::routes::moviebox::home_handler),
        )
        .route(
            "/api/moviebox/discover",
            get(crate::routes::moviebox::discover_handler),
        )
        .route(
            "/api/moviebox/section",
            get(crate::routes::moviebox::section_handler),
        )
        .route(
            "/api/moviebox/trending",
            get(crate::routes::moviebox::trending_handler),
        )
        .route(
            "/api/moviebox/popular-movie",
            get(crate::routes::moviebox::popular_movie_handler),
        )
        .route(
            "/api/moviebox/popular-series",
            get(crate::routes::moviebox::popular_series_handler),
        )
        .route(
            "/api/moviebox/hot-shorts",
            get(crate::routes::moviebox::hot_shorts_handler),
        )
        .route("/api/moviebox/sa", get(crate::routes::moviebox::sa_handler))
        .route(
            "/api/moviebox/thai",
            get(crate::routes::moviebox::thai_handler),
        )
        .route(
            "/api/moviebox/turkish",
            get(crate::routes::moviebox::turkish_handler),
        )
        .route(
            "/api/moviebox/western",
            get(crate::routes::moviebox::western_handler),
        )
        .route(
            "/api/moviebox/animation",
            get(crate::routes::moviebox::animation_handler),
        )
        .route(
            "/api/moviebox/black-shows",
            get(crate::routes::moviebox::black_shows_handler),
        )
        .route(
            "/api/moviebox/hero",
            get(crate::routes::moviebox::hero_handler),
        )
        .route(
            "/api/moviebox/bollywood",
            get(crate::routes::moviebox::bollywood_handler),
        )
        .route(
            "/api/moviebox/shorts",
            get(crate::routes::moviebox::shorts_handler),
        )
        .route(
            "/api/moviebox/filter",
            get(crate::routes::moviebox::filter_handler),
        )
        .route(
            "/api/moviebox/info",
            get(crate::routes::moviebox::info_handler),
        )
        .route(
            "/api/moviebox/detail",
            get(crate::routes::moviebox::detail_handler),
        )
        .route(
            "/api/moviebox/search",
            get(crate::routes::moviebox::search_handler),
        )
        .route(
            "/api/anime/search",
            get(crate::routes::anime::search_handler),
        )
        .route(
            "/api/anime/latest",
            get(crate::routes::anime::latest_handler),
        )
        .route(
            "/api/anime/category",
            get(crate::routes::anime::category_handler),
        )
        .route("/api/anime/home", get(crate::routes::anime::home_handler))
        .route(
            "/api/anime/detail/:slug",
            get(crate::routes::anime::detail_handler),
        )
        .route(
            "/api/anime/episodes/:id",
            get(crate::routes::anime::episodes_handler),
        )
        .route(
            "/api/anime/servers",
            get(crate::routes::anime::servers_handler),
        )
        .route(
            "/api/anime/stream",
            get(crate::routes::anime::stream_handler),
        )
        .route(
            "/api/anime/filter",
            axum::routing::post(crate::routes::anime::filter_handler),
        )
        .route(
            "/api/anime/discover",
            get(crate::routes::anime::discover_get_handler)
                .post(crate::routes::anime::discover_post_handler),
        )
        .route(
            "/api/anime/filters",
            get(crate::routes::anime::filter_options_handler),
        )
        .route(
            "/api/anime/schedule",
            get(crate::routes::anime::schedule_handler),
        )
        .route(
            "/api/aisports/leagues",
            get(crate::routes::sports::leagues_handler),
        )
        .route(
            "/api/sports/leagues",
            get(crate::routes::sports::leagues_handler),
        )
        .route(
            "/api/aisports/latest",
            get(crate::routes::sports::latest_handler),
        )
        .route(
            "/api/sports/latest",
            get(crate::routes::sports::latest_handler),
        )
        .route(
            "/api/aisports/league-tabs",
            get(crate::routes::sports::league_tabs_handler),
        )
        .route(
            "/api/sports/league-tabs",
            get(crate::routes::sports::league_tabs_handler),
        )
        .route(
            "/api/aisports/matches",
            get(crate::routes::sports::matches_handler),
        )
        .route(
            "/api/sports/matches",
            get(crate::routes::sports::matches_handler),
        )
        .route(
            "/api/aisports/matches/grouped",
            get(crate::routes::sports::grouped_matches_handler),
        )
        .route(
            "/api/sports/matches/grouped",
            get(crate::routes::sports::grouped_matches_handler),
        )
        .route(
            "/api/aisports/matches/live",
            get(crate::routes::sports::live_matches_handler),
        )
        .route(
            "/api/sports/matches/live",
            get(crate::routes::sports::live_matches_handler),
        )
        .route(
            "/api/aisports/matches/filter-tabs",
            get(crate::routes::sports::filter_tabs_handler),
        )
        .route(
            "/api/sports/matches/filter-tabs",
            get(crate::routes::sports::filter_tabs_handler),
        )
        .route(
            "/api/aisports/home",
            get(crate::routes::sports::home_handler),
        )
        .route("/api/sports/home", get(crate::routes::sports::home_handler))
        .route(
            "/api/sports/streams",
            get(crate::routes::sports::streams_handler),
        )
        .route(
            "/api/aisports/detail/:id",
            get(crate::routes::sports::detail_handler),
        )
        .route(
            "/api/sports/detail/:id",
            get(crate::routes::sports::detail_handler),
        )
        .route("/api/tmdb/home", get(crate::routes::tmdb::home_handler))
        .route(
            "/api/tmdb/movie/popular",
            get(crate::routes::tmdb::movie_popular_handler),
        )
        .route(
            "/api/tmdb/movie/top-rated",
            get(crate::routes::tmdb::movie_top_rated_handler),
        )
        .route(
            "/api/tmdb/movie/now-playing",
            get(crate::routes::tmdb::movie_now_playing_handler),
        )
        .route(
            "/api/tmdb/tv/popular",
            get(crate::routes::tmdb::tv_popular_handler),
        )
        .route(
            "/api/tmdb/tv/top-rated",
            get(crate::routes::tmdb::tv_top_rated_handler),
        )
        .route(
            "/api/tmdb/tv/on-the-air",
            get(crate::routes::tmdb::tv_on_the_air_handler),
        )
        .route(
            "/api/tmdb/trending",
            get(crate::routes::tmdb::trending_handler),
        )
        .route(
            "/api/tmdb/discover",
            get(crate::routes::tmdb::discover_handler),
        )
        .route("/api/tmdb/search", get(crate::routes::tmdb::search_handler))
        .route("/api/status", get(crate::routes::system::status_handler))
        .route(
            "/api/auth/token",
            get(crate::routes::system::auth_token_handler),
        )
        .route("/api/docs", get(scalar_docs_handler))
        .route("/api/docs/openapi.json", get(scalar_spec_handler))
        .layer(TraceLayer::new_for_http())
        .layer(CompressionLayer::new())
        .layer(middleware::from_fn(hue_anon_middleware))
        .layer(middleware::from_fn(request_id_middleware))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            rate_limit_middleware,
        ))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ))
        .layer(CorsLayer::permissive())
        .with_state(state)
}

/// Scalar API reference UI. Renders the same OpenAPI spec that
/// Swagger UI consumed, served at the same path (`/api/docs`). The
/// spec is inlined into the HTML at `to_html()` time (Scalar reads it
/// from `<script id="api-reference">…`), so the page is self-contained.
/// The raw spec JSON is also served at `/api/docs/openapi.json` for
/// tooling (Postman, code-gen, curl).
#[allow(dead_code)] // route registration reads this; clippy doesn't see that
#[utoipa::path(
    get,
    path = "/api/docs",
    responses(
        (status = 200, description = "Scalar API reference HTML", content_type = "text/html"),
    ),
)]
pub async fn scalar_docs_handler() -> impl IntoResponse {
    let spec: serde_json::Value =
        serde_json::to_value(crate::openapi::ApiDoc::openapi())
            .expect("ApiDoc is always serializable");
    let html = Scalar::new(spec).to_html();
    (
        StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "text/html; charset=utf-8")],
        html,
    )
}

/// Raw OpenAPI spec JSON for tooling. Same payload Scalar inlines into
/// the HTML; kept as a dedicated endpoint so Postman, openapi-generator,
/// etc. can pull it without parsing the HTML page.
#[allow(dead_code)]
#[utoipa::path(
    get,
    path = "/api/docs/openapi.json",
    responses(
        (status = 200, description = "OpenAPI 3 spec as JSON", content_type = "application/json"),
    ),
)]
pub async fn scalar_spec_handler() -> impl IntoResponse {
    let spec: serde_json::Value =
        serde_json::to_value(crate::openapi::ApiDoc::openapi())
            .expect("ApiDoc is always serializable");
    Json(spec)
}

async fn auth_middleware(
    State(state): State<AppState>,
    req: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    let path = req.uri().path();
    // Proxy routes are self-authenticated via encrypted h_token — skip API key check.
    // The auth-token endpoint is public so clients can mint a Bearer before making
    // any other call.
    if path == "/ts-proxy"
        || path == "/m3u8-proxy"
        || path == "/sub-proxy"
        || path == "/storage/enc.key"
        || path == "/api/auth/token"
        || path == "/api/docs"
        || path.starts_with("/api/docs/")
    {
        return Ok(next.run(req).await);
    }

    let api_key = state.config.api_key.trim();
    if api_key.is_empty() {
        // Development mode: no API key configured, allow all requests.
        return Ok(next.run(req).await);
    }

    // Priority 1: Authorization: Bearer <token> (preferred — secret never leaves headers)
    if let Some(bearer) = req
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer ").or_else(|| v.strip_prefix("bearer ")))
    {
        let client_ip = extract_client_ip(&req);
        if crate::security::verify_api_token(bearer, &state.config.secret, &client_ip).is_some() {
            return Ok(next.run(req).await);
        }
    }

    // Priority 2: X-API-Key header (server-to-server callers — Nuxt nitro, etc.)
    let header_auth = req.headers().get("x-api-key").and_then(|v| v.to_str().ok());

    // Priority 3: ?api_key=... query (legacy, log once per process)
    let query_auth = req.uri().query().and_then(|q| {
        q.split('&').find_map(|pair| {
            let mut parts = pair.splitn(2, '=');
            let key = parts.next()?;
            let value = parts.next()?;
            if key == "api_key" {
                Some(value)
            } else {
                None
            }
        })
    });

    if header_auth == Some(api_key) || query_auth == Some(api_key) {
        if query_auth == Some(api_key) {
            use std::sync::OnceLock;
            use std::sync::atomic::{AtomicBool, Ordering};
            static WARNED: OnceLock<AtomicBool> = OnceLock::new();
            let flag = WARNED.get_or_init(|| AtomicBool::new(false));
            if !flag.swap(true, Ordering::Relaxed) {
                tracing::warn!(
                    "client is still sending ?api_key=... in query string; switch to Authorization: Bearer"
                );
            }
        }
        return Ok(next.run(req).await);
    }

    Err(StatusCode::FORBIDDEN)
}

/// Best-effort client IP extraction. The Rust process sits behind the Nuxt
/// nitro proxy (and possibly a CDN), so `X-Forwarded-For` (first hop) and
/// `X-Real-IP` are the only realistic sources. Returns an empty string when
/// the caller cannot be identified — the token verifier treats that as a
/// hard failure (fails closed).
pub fn extract_client_ip(req: &Request<Body>) -> String {
    if let Some(xff) = req
        .headers()
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
    {
        if let Some(first) = xff.split(',').next() {
            let trimmed = first.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
    }
    if let Some(real) = req
        .headers()
        .get("x-real-ip")
        .and_then(|v| v.to_str().ok())
    {
        let trimmed = real.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    String::new()
}

async fn request_id_middleware(req: Request<Body>, next: Next) -> Response {
    let id = ulid::Ulid::new().to_string();
    let mut response = next.run(req).await;
    if let Ok(val) = axum::http::HeaderValue::from_str(&id) {
        response.headers_mut().insert("x-request-id", val);
    }
    response
}

const HUE_ANON_COOKIE: &str = "hue_anon";

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct HueAnonId(pub String);

async fn hue_anon_middleware(mut req: Request<Body>, next: Next) -> Response {
    let existing = req
        .headers()
        .get_all(axum::http::header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|s| s.split(';'))
        .map(str::trim)
        .find_map(|pair| {
            let (k, v) = pair.split_once('=')?;
            if k == HUE_ANON_COOKIE {
                Some(v.to_string())
            } else {
                None
            }
        });

    let is_new = existing.is_none();
    let id = existing.unwrap_or_else(|| ulid::Ulid::new().to_string());
    req.extensions_mut().insert(HueAnonId(id.clone()));

    let mut response = next.run(req).await;

    if is_new {
        if let Ok(val) = axum::http::HeaderValue::from_str(&format!(
            "{HUE_ANON_COOKIE}={id}; Path=/; Max-Age=63072000; HttpOnly; SameSite=Lax"
        )) {
            response
                .headers_mut()
                .append(axum::http::header::SET_COOKIE, val);
        }
    }
    response
}

async fn vixsrc_enc_key_handler(State(state): State<AppState>) -> Response {
    match state.client
        .get("https://vixsrc.to/storage/enc.key")
        .header("Referer", "https://vixsrc.to/")
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36")
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            match resp.bytes().await {
                Ok(bytes) => Response::builder()
                    .header("content-type", "application/octet-stream")
                    .header("cache-control", "public, max-age=300")
                    .body(Body::from(bytes))
                    .unwrap(),
                Err(e) => (StatusCode::BAD_GATEWAY, e.to_string()).into_response(),
            }
        }
        Ok(resp) => (StatusCode::from_u16(resp.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY), "").into_response(),
        Err(e) => (StatusCode::BAD_GATEWAY, e.to_string()).into_response(),
    }
}

async fn rate_limit_middleware(
    State(state): State<AppState>,
    req: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    let path = req.uri().path();
    // Proxy routes are self-authenticated via h_token and stream many segments — skip rate limiting
    if path.starts_with("/m3u8-proxy")
        || path.starts_with("/ts-proxy")
        || path.starts_with("/sub-proxy")
        || path.starts_with("/storage/")
    {
        return Ok(next.run(req).await);
    }

    // Xyra fast-path races the main aggregator in the browser. 429ing it
    // means the fast-path dies and the user only sees the slow main path's
    // results. The upstream is rate-limited by xyra themselves, so we don't
    // need to gate it on our end.
    if path.starts_with("/api/streams/xyra/") {
        return Ok(next.run(req).await);
    }

    let ip = req
        .extensions()
        .get::<axum::extract::ConnectInfo<SocketAddr>>()
        .map(|c| c.0.ip().to_string())
        .or_else(|| {
            req.headers()
                .get("x-forwarded-for")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.split(',').next().unwrap_or(s).trim().to_string())
        })
        .unwrap_or_else(|| "unknown".to_string());

    if !state.rate_limiter.check(&ip) {
        return Ok((StatusCode::TOO_MANY_REQUESTS, "Rate limit exceeded").into_response());
    }
    Ok(next.run(req).await)
}

// ── Sub-state projections ───────────────────────────────────────────────────
use axum::extract::FromRef;

/// State for /api/streams and /api/streams/.../live (SSE)
#[derive(Clone)]
pub struct StreamState {
    pub registry: Arc<ProviderRegistry>,
    pub cache: Arc<StreamCache>,
    pub sticky_pref: Arc<crate::infra::sticky_pref::StickyPrefCache>,
    pub provider_score: Arc<ProviderScoreCache>,
    pub tmdb: Arc<TmdbClient>,
    pub filter_config: Arc<FilterConfig>,
    pub coalescer: Arc<FetchCoalescer>,
    pub metrics: Arc<Metrics>,
    pub config: Arc<AppConfig>,
    pub secret: String,
    pub kkey: String,
    pub proxy_base_url: String,
    pub client: ClientWithMiddleware,
    pub hitv: Arc<HiTvClient>,
}

impl FromRef<AppState> for StreamState {
    fn from_ref(s: &AppState) -> Self {
        Self {
            registry: s.registry.clone(),
            cache: s.cache.clone(),
            sticky_pref: s.sticky_pref.clone(),
            provider_score: s.provider_score.clone(),
            tmdb: s.tmdb.clone(),
            filter_config: s.filter_config.clone(),
            coalescer: s.coalescer.clone(),
            metrics: s.metrics.clone(),
            config: s.config.clone(),
            secret: s.secret.clone(),
            kkey: s.kkey.clone(),
            proxy_base_url: s.proxy_base_url.clone(),
            client: s.client.clone(),
            hitv: s.hitv.clone(),
        }
    }
}

/// State for /api/drama/* routes
#[derive(Clone)]
pub struct DramaState {
    pub kisskh: Arc<KissKhClient>,
    pub dramacool: Arc<DramacoolClient>,
    pub kisskh_xyra: Arc<KissKhXyraClient>,
    pub kissasian: Arc<KissAsianClient>,
    pub kissasian_top: Arc<KissAsianTopClient>,
    pub moviebox: Arc<MovieBoxClient>,
    pub tmdb: Arc<TmdbClient>,
    pub secret: String,
    pub kkey: String,
    pub proxy_base_url: String,
}

impl FromRef<AppState> for DramaState {
    fn from_ref(s: &AppState) -> Self {
        Self {
            kisskh: s.kisskh.clone(),
            dramacool: s.dramacool.clone(),
            kisskh_xyra: s.kisskh_xyra.clone(),
            kissasian: s.kissasian.clone(),
            kissasian_top: s.kissasian_top.clone(),
            moviebox: s.moviebox.clone(),
            tmdb: s.tmdb.clone(),
            secret: s.secret.clone(),
            kkey: s.kkey.clone(),
            proxy_base_url: s.proxy_base_url.clone(),
        }
    }
}

/// State for /api/hitv/* routes
#[derive(Clone)]
pub struct HiTvState {
    pub hitv: Arc<HiTvClient>,
    pub tmdb: Arc<TmdbClient>,
    pub proxy_base_url: String,
}

impl FromRef<AppState> for HiTvState {
    fn from_ref(s: &AppState) -> Self {
        Self {
            hitv: s.hitv.clone(),
            tmdb: s.tmdb.clone(),
            proxy_base_url: s.proxy_base_url.clone(),
        }
    }
}

/// State for /api/anime/* routes
#[derive(Clone)]
pub struct AnimeState {
    pub hianime: Arc<crate::clients::hianime::HianimeClient>,
    pub anime_agg: Arc<crate::clients::anime_agg::AnimeAggregator>,
    pub config: Arc<AppConfig>,
    pub secret: String,
    pub proxy_base_url: String,
}

impl FromRef<AppState> for AnimeState {
    fn from_ref(s: &AppState) -> Self {
        Self {
            hianime: s.hianime.clone(),
            anime_agg: s.anime_agg.clone(),
            config: s.config.clone(),
            secret: s.secret.clone(),
            proxy_base_url: s.proxy_base_url.clone(),
        }
    }
}

/// State for /api/moviebox/* routes
#[derive(Clone)]
pub struct MovieBoxRouteState {
    pub moviebox: Arc<MovieBoxClient>,
}

impl FromRef<AppState> for MovieBoxRouteState {
    fn from_ref(s: &AppState) -> Self {
        Self {
            moviebox: s.moviebox.clone(),
        }
    }
}

/// State for /api/providers
#[derive(Clone)]
pub struct ProvidersState {
    pub registry: Arc<ProviderRegistry>,
}

impl FromRef<AppState> for ProvidersState {
    fn from_ref(s: &AppState) -> Self {
        Self {
            registry: s.registry.clone(),
        }
    }
}

/// State for /api/tmdb/* routes
#[derive(Clone)]
pub struct TmdbState {
    pub tmdb: Arc<TmdbClient>,
}

impl FromRef<AppState> for TmdbState {
    fn from_ref(s: &AppState) -> Self {
        Self {
            tmdb: s.tmdb.clone(),
        }
    }
}

/// State for system routes (health, metrics, status, admin)
#[derive(Clone)]
pub struct SystemState {
    pub metrics: Arc<Metrics>,
    pub hitv: Arc<HiTvClient>,
    pub registry: Arc<ProviderRegistry>,
    pub start_time: std::time::Instant,
    pub client: ClientWithMiddleware,
    pub secret: String,
    pub config: std::sync::Arc<crate::config::AppConfig>,
}

impl FromRef<AppState> for SystemState {
    fn from_ref(s: &AppState) -> Self {
        Self {
            metrics: s.metrics.clone(),
            hitv: s.hitv.clone(),
            registry: s.registry.clone(),
            start_time: s.start_time,
            client: s.client.clone(),
            secret: s.config.secret.clone(),
            config: s.config.clone(),
        }
    }
}
