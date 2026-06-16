use axum::body::Body;
use axum::{
    extract::State,
    http::{Request, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::get,
    Router,
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
use utoipa_swagger_ui::SwaggerUi;

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
    kisskh::KissKhClient,
    kisskh_provider::KissKhProvider,
    lordflix_provider::LordflixProvider,
    moviebox_provider::MovieBoxProvider,
    notorrent_provider::NoTorrentProvider,
    // mp4hydra_provider::Mp4HydraProvider, // disabled — endpoint times out
    sflix_provider::SflixProvider,
    vidlink_provider::VidlinkProvider,
    // videasy::VideasyClient,       // disabled — upstream 500
    // videasy_provider::VideasyProvider,
    vidrock_provider::VidrockProvider,
    vidsrcembed_provider::VidsrcEmbedProvider,
    vidvault_provider::VidvaultProvider,
    vidzee::VidzeeClient,
    vidzee_provider::VidzeeProvider,
    vixsrc::VixsrcClient,
    vixsrc_provider::VixsrcProvider,
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
    pub moviebox: Arc<MovieBoxClient>,
    pub hianime: Arc<crate::clients::hianime::HianimeClient>,
    pub vidvault: Arc<crate::clients::vidvault::VidVaultClient>,
    pub aisports: Arc<crate::clients::aisports::AiSportsClient>,
    pub anime_agg: Arc<crate::clients::anime_agg::AnimeAggregator>,
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
        "Nebula",
    ]));
    circuit_breaker.set_metrics(metrics.clone());
    let filter_config = Arc::new(FilterConfig::default());

    let mut registry = ProviderRegistry::new(circuit_breaker.clone());
    let moviebox_client = Arc::new(MovieBoxClient::new(None));
    let kisskh_client = Arc::new(crate::providers::kisskh::KissKhClient::new());
    let sflix_client = Arc::new(crate::providers::sflix::SflixClient::new());
    let dramacool_client = Arc::new(crate::providers::dramacool::DramacoolClient::new());
    let kissasian_client = Arc::new(crate::providers::kissasian::KissAsianClient::new());
    let hianime_client = Arc::new(crate::clients::hianime::HianimeClient::new());
    let anime_agg = Arc::new(crate::clients::anime_agg::AnimeAggregator::new(
        Arc::new(crate::clients::aniwave::AniwaveClient::new(cfg.cf_worker_url.clone())),
        hianime_client.clone(),
        Arc::new(crate::clients::aniwatch::AniWatchClient::new()),
        Arc::new(crate::clients::anilist::AniListClient::new()),
        Arc::new(crate::clients::animexin::AnimexinClient::new()),
    ));
    let vixsrc_client = Arc::new(VixsrcClient::new(
        crate::http_client::build_client_with_retry(
            reqwest::Client::builder()
                .tcp_keepalive(Duration::from_secs(60))
                .connect_timeout(Duration::from_secs(8))
                .pool_max_idle_per_host(2)
                .pool_idle_timeout(Duration::from_secs(90))
                .timeout(Duration::from_secs(15))
                // vixsrc.to uses Cloudflare bot management. HTTP/2 + rustls is
                // fingerprinted and 403'd; HTTP/1.1 is accepted but the embed
                // page body is sometimes sent raw-gzipped without the proper
                // Content-Encoding header, so we decompress manually in the client.
                .http1_only()
                .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/137.0.0.0 Safari/537.36"),
            "VIXSRC_PROXY_URL",
        ),
    ));

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
    let lordflix_client = Arc::new(crate::clients::lordflix::LordflixClient::new(
        crate::http_client::wrap_with_retry(
            reqwest::Client::builder()
                .tcp_keepalive(Duration::from_secs(60))
                .connect_timeout(Duration::from_secs(8))
                .timeout(Duration::from_secs(15))
                .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/137.0.0.0 Safari/537.36")
                .build()
                .unwrap(),
        ),
    ));
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
    registry.register(Arc::new(DramacoolProvider::new(dramacool_client.clone())));
    registry.register(Arc::new(KissAsianProvider::new(kissasian_client.clone())));
    registry.register(Arc::new(VixsrcProvider::new(vixsrc_client)));
    // registry.register(Arc::new(Mp4HydraProvider::new())); // disabled — endpoint times out
    registry.register(Arc::new(VidrockProvider::new(vidrock_client.clone())));
    registry.register(Arc::new(VidvaultProvider::new(vidvault_client.clone())));
    registry.register(Arc::new(VidzeeProvider::new(vidzee_client.clone())));
    // registry.register(Arc::new(VideasyProvider::new(videasy_client.clone()))); // disabled — upstream 500
    registry.register(Arc::new(VidsrcEmbedProvider::new(
        vidsrcembed_client.clone(),
    )));
    registry.register(Arc::new(VidlinkProvider::new(vidlink_client)));
    registry.register(Arc::new(LordflixProvider::new(lordflix_client)));
    registry.register(Arc::new(NoTorrentProvider::new(notorrent_client)));

    let redis_pool = if cfg.cache_backend == "redis" {
        let pool_cfg = deadpool_redis::Config::from_url(&cfg.redis_url);
        pool_cfg
            .create_pool(Some(deadpool_redis::Runtime::Tokio1))
            .ok()
    } else {
        None
    };

    let raw_client = crate::http_client::with_optional_proxy(
        reqwest::Client::builder()
            .tcp_keepalive(Duration::from_secs(60))
            .tcp_nodelay(true)
            .timeout(Duration::from_secs(600))
            .connect_timeout(Duration::from_secs(10))
            .pool_max_idle_per_host(32)
            .pool_idle_timeout(Duration::from_secs(120))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36"),
        "STREAM_PROXY_URL",
    )
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
        moviebox: moviebox_client.clone(),
        hianime: hianime_client.clone(),
        vidvault: vidvault_client.clone(),
        anime_agg: anime_agg.clone(),
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
        rate_limiter: RateLimitState::new(120, 60),
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

    // governor handles its own key expiration — no manual eviction needed
}

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(crate::routes::system::health_check))
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
            "/api/streams/:type/:tmdbId",
            get(crate::routes::streams::streams_handler),
        )
        .route(
            "/api/streams/:type/:tmdbId/live",
            get(crate::routes::sse::sse_streams_handler),
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
            "/api/drama/kisskh/streams/:id/:episode",
            get(crate::routes::drama::kisskh_streams_handler),
        )
        .route(
            "/api/drama/dramacool/search",
            get(crate::routes::drama::dramacool_search_handler),
        )
        .route(
            "/api/drama/dramacool/streams/:id/:episode",
            get(crate::routes::drama::dramacool_streams_handler),
        )
        .route(
            "/api/drama/kissasian/search",
            get(crate::routes::drama::kissasian_search_handler),
        )
        .route(
            "/api/drama/kissasian/streams/:id/:episode",
            get(crate::routes::drama::kissasian_streams_handler),
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
        .merge(
            SwaggerUi::new("/api/docs")
                .url("/api/docs/openapi.json", crate::openapi::ApiDoc::openapi()),
        )
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

async fn auth_middleware(
    State(_state): State<AppState>,
    req: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    let path = req.uri().path();
    // Proxy routes are self-authenticated via encrypted h_token — skip API key check
    if path == "/ts-proxy"
        || path == "/m3u8-proxy"
        || path == "/sub-proxy"
        || path == "/storage/enc.key"
    {
        return Ok(next.run(req).await);
    }
    if path.starts_with("/api/admin/") {
        let api_key = env::var("API_KEY")
            .ok()
            .filter(|key| !key.trim().is_empty())
            .ok_or(StatusCode::FORBIDDEN)?;
        let auth = req.headers().get("x-api-key").and_then(|v| v.to_str().ok());
        if auth != Some(api_key.as_str()) {
            return Err(StatusCode::FORBIDDEN);
        }
        return Ok(next.run(req).await);
    }

    let is_local = req
        .extensions()
        .get::<SocketAddr>()
        .map_or(false, |a| a.ip().is_loopback());
    if is_local {
        return Ok(next.run(req).await);
    }

    if let Ok(api_key) = env::var("API_KEY") {
        if api_key.trim().is_empty() {
            return Ok(next.run(req).await);
        }
        let auth = req.headers().get("x-api-key").and_then(|v| v.to_str().ok());
        if auth != Some(&api_key) {
            return Err(StatusCode::FORBIDDEN);
        }
    }
    Ok(next.run(req).await)
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
    pub kissasian: Arc<KissAsianClient>,
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
            kissasian: s.kissasian.clone(),
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
}

impl FromRef<AppState> for SystemState {
    fn from_ref(s: &AppState) -> Self {
        Self {
            metrics: s.metrics.clone(),
            hitv: s.hitv.clone(),
            registry: s.registry.clone(),
            start_time: s.start_time,
            client: s.client.clone(),
        }
    }
}
