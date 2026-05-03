//! Hue Stream API
//! Professional Streaming Aggregator
//!
//! @author roywalk3r <roywalker440@gmail.com>
//! @repository https://github.com/roywalk3r/hue.git
//! @license MIT

// The doc stub functions below are referenced only by the utoipa #[derive(OpenApi)] macro
// at compile time for OpenAPI spec generation. They are never invoked at runtime.
#![allow(dead_code)]

use utoipa::OpenApi;
use crate::models::{StreamResult, Subtitle};

// ── System endpoints ────────────────────────────────────────────────────

#[utoipa::path(
    get,
    path = "/health",
    responses((status = 200, description = "Health check — returns 'Huerust is Alive'")),
)]
pub(crate) async fn health_check_doc() {}

#[utoipa::path(
    get,
    path = "/metrics",
    responses((status = 200, description = "Prometheus metrics dump")),
)]
pub(crate) async fn metrics_doc() {}

#[utoipa::path(
    get,
    path = "/m3u8-proxy",
    params(
        ("h_token" = Option<String>, Query, description = "Encrypted token with target URL and headers"),
        ("url" = Option<String>, Query, description = "Raw target URL (alternative to h_token)"),
        ("mode" = Option<String>, Query, description = "Proxy mode: 'url' for raw url+headers"),
        ("download" = Option<String>, Query, description = "Set to force download"),
        ("filename" = Option<String>, Query, description = "Download filename hint"),
        ("adsurgery" = Option<u32>, Query, description = "Enable ad surgery (1 = on)"),
    ),
    responses(
        (status = 200, description = "Proxied M3U8/HLS playlist stream"),
        (status = 403, description = "Invalid or missing token/url"),
    ),
)]
pub(crate) async fn proxy_m3u8_doc() {}

#[utoipa::path(
    get,
    path = "/ts-proxy",
    params(
        ("h_token" = Option<String>, Query, description = "Encrypted token with target URL and headers"),
        ("url" = Option<String>, Query, description = "Raw target URL (alternative to h_token)"),
        ("range" = Option<String>, Query, description = "Byte range request"),
        ("force_200" = Option<u32>, Query, description = "Force 200 status instead of 206"),
        ("no_synth" = Option<u32>, Query, description = "Disable synthetic segments"),
        ("sanitize" = Option<u32>, Query, description = "Sanitize segment (1 = on)"),
    ),
    responses(
        (status = 200, description = "Proxied TS segment stream"),
        (status = 403, description = "Invalid or missing token/url"),
    ),
)]
pub(crate) async fn proxy_ts_doc() {}

#[utoipa::path(
    get,
    path = "/sub-proxy",
    params(
        ("h_token" = Option<String>, Query, description = "Encrypted token with subtitle URL and headers"),
        ("url" = Option<String>, Query, description = "Raw subtitle URL (alternative to h_token)"),
    ),
    responses(
        (status = 200, description = "Proxied subtitle file (VTT/SRT/ASS)"),
        (status = 403, description = "Invalid or missing token/url"),
    ),
)]
pub(crate) async fn proxy_sub_doc() {}

// ── Admin endpoints ─────────────────────────────────────────────────────

#[utoipa::path(
    post,
    path = "/api/admin/reload",
    responses((status = 200, description = "Config cache invalidated — next request reloads from disk")),
)]
pub(crate) async fn admin_reload_doc() {}

#[utoipa::path(
    post,
    path = "/api/admin/refresh-tokens",
    responses(
        (status = 200, description = "Token refresh triggered via Node.js microservice"),
        (status = 502, description = "Node.js microservice unreachable"),
    ),
)]
pub(crate) async fn admin_refresh_tokens_doc() {}

// ── HiTV endpoints ─────────────────────────────────────────────────────

#[utoipa::path(
    get,
    path = "/api/hitv/library",
    responses((status = 200, description = "Complete HiTV library (all cached items)")),
)]
pub(crate) async fn hitv_library_doc() {}

#[utoipa::path(
    get,
    path = "/api/hitv/browse",
    params(
        ("country" = Option<String>, Query, description = "Filter by country (All, Korea, China, Thailand, Japan)"),
        ("category" = Option<String>, Query, description = "Content category (Drama, Movie, Variety)"),
        ("genre" = Option<String>, Query, description = "Genre filter (All, Romance, Comedy, Action, Thriller, Horror, Fantasy, Historical, Mystery, Family)"),
        ("sort" = Option<String>, Query, description = "Sort order (Hottest, Newest, TopRated)"),
        ("year" = Option<String>, Query, description = "Filter by release year"),
        ("page" = Option<u32>, Query, description = "Page number (default 1)"),
        ("page_size" = Option<u32>, Query, description = "Results per page (default 60)"),
    ),
    responses(
        (status = 200, description = "Browse results with items array and total count"),
        (status = 502, description = "Upstream fetch failed"),
    ),
)]
pub(crate) async fn hitv_browse_doc() {}

#[utoipa::path(
    get,
    path = "/api/hitv/search",
    params(
        ("q" = String, Query, description = "Search query"),
        ("page" = Option<u32>, Query, description = "Page number (default 1)"),
        ("page_size" = Option<u32>, Query, description = "Results per page (default 60)"),
    ),
    responses(
        (status = 200, description = "Search results matching query"),
        (status = 400, description = "Missing 'q' parameter"),
        (status = 502, description = "Upstream fetch failed"),
    ),
)]
pub(crate) async fn hitv_search_doc() {}

#[utoipa::path(
    get,
    path = "/api/hitv/latest",
    params(
        ("country" = Option<String>, Query, description = "Filter by country"),
        ("category" = Option<String>, Query, description = "Content category"),
        ("genre" = Option<String>, Query, description = "Genre filter"),
        ("page" = Option<u32>, Query, description = "Page number (default 1)"),
        ("page_size" = Option<u32>, Query, description = "Results per page (default 60)"),
    ),
    responses(
        (status = 200, description = "Latest HiTV content (sorted by Newest)"),
        (status = 502, description = "Upstream fetch failed"),
    ),
)]
pub(crate) async fn hitv_latest_doc() {}

#[utoipa::path(
    get,
    path = "/api/hitv/trending",
    params(
        ("country" = Option<String>, Query, description = "Filter by country"),
        ("category" = Option<String>, Query, description = "Content category"),
        ("genre" = Option<String>, Query, description = "Genre filter"),
        ("page" = Option<u32>, Query, description = "Page number (default 1)"),
        ("page_size" = Option<u32>, Query, description = "Results per page (default 60)"),
    ),
    responses(
        (status = 200, description = "Trending HiTV content (sorted by Hottest)"),
        (status = 502, description = "Upstream fetch failed"),
    ),
)]
pub(crate) async fn hitv_trending_doc() {}

#[utoipa::path(
    get,
    path = "/api/hitv/hero",
    responses((status = 200, description = "Hero carousel items (top 10 from library)")),
)]
pub(crate) async fn hitv_hero_doc() {}

#[utoipa::path(
    get,
    path = "/api/hitv/drama",
    params(
        ("country" = Option<String>, Query, description = "Filter by country"),
        ("genre" = Option<String>, Query, description = "Genre filter"),
        ("sort" = Option<String>, Query, description = "Sort order"),
        ("page" = Option<u32>, Query, description = "Page number (default 1)"),
        ("page_size" = Option<u32>, Query, description = "Results per page (default 60)"),
    ),
    responses(
        (status = 200, description = "Drama category browse results"),
        (status = 502, description = "Upstream fetch failed"),
    ),
)]
pub(crate) async fn hitv_drama_doc() {}

#[utoipa::path(
    get,
    path = "/api/hitv/info/{id}",
    params(
        ("id" = String, Path, description = "HiTV internal item ID"),
    ),
    responses(
        (status = 200, description = "Detailed HiTV item with metadata"),
        (status = 404, description = "Item not found"),
        (status = 502, description = "Upstream fetch failed"),
    ),
)]
pub(crate) async fn hitv_info_doc() {}

#[utoipa::path(
    get,
    path = "/api/hitv/play/{id}/{episode}",
    params(
        ("id" = String, Path, description = "HiTV internal item ID"),
        ("episode" = u32, Path, description = "Episode number"),
        ("season" = Option<u32>, Query, description = "Season number (default 1)"),
    ),
    responses(
        (status = 307, description = "Redirect to resolved /api/play/tv/{tmdbId} stream"),
        (status = 404, description = "HiTV item not found or TMDB ID unresolvable"),
    ),
)]
pub(crate) async fn hitv_play_doc() {}

#[utoipa::path(
    get,
    path = "/api/hitv/countries",
    responses((status = 200, description = "List of available countries: Korea, China, Thailand, Japan")),
)]
pub(crate) async fn hitv_countries_doc() {}

#[utoipa::path(
    get,
    path = "/api/hitv/categories",
    responses((status = 200, description = "List of available categories: Drama, Movie, Variety")),
)]
pub(crate) async fn hitv_categories_doc() {}

#[utoipa::path(
    get,
    path = "/api/hitv/genres",
    responses((status = 200, description = "List of available genres")),
)]
pub(crate) async fn hitv_genres_doc() {}

#[utoipa::path(
    get,
    path = "/api/hitv/sorts",
    responses((status = 200, description = "List of available sort orders: Hottest, Newest, TopRated")),
)]
pub(crate) async fn hitv_sorts_doc() {}

#[utoipa::path(
    get,
    path = "/api/hitv/{country}/{sort}/{genre}",
    params(
        ("country" = String, Path, description = "Country filter"),
        ("sort" = String, Path, description = "Sort order (Hottest, Newest, TopRated)"),
        ("genre" = String, Path, description = "Genre filter"),
        ("category" = Option<String>, Query, description = "Content category (default Drama)"),
        ("page" = Option<u32>, Query, description = "Page number (default 1)"),
        ("page_size" = Option<u32>, Query, description = "Results per page (default 60)"),
    ),
    responses(
        (status = 200, description = "Dynamic browse results by country/sort/genre path"),
        (status = 502, description = "Upstream fetch failed"),
    ),
)]
pub(crate) async fn hitv_dynamic_browse_doc() {}

// ── Drama endpoints ────────────────────────────────────────────────────

#[utoipa::path(
    get,
    path = "/api/drama/kisskh/search",
    params(
        ("q" = String, Query, description = "Search query"),
    ),
    responses(
        (status = 200, description = "KissKH search results"),
        (status = 500, description = "Internal search error"),
    ),
)]
pub(crate) async fn drama_kisskh_search_doc() {}

#[utoipa::path(
    get,
    path = "/api/drama/kisskh/streams/{id}/{episode}",
    params(
        ("id" = String, Path, description = "KissKH drama ID"),
        ("episode" = u32, Path, description = "Episode number"),
    ),
    responses(
        (status = 200, description = "Stream URLs for requested episode", body = Vec<StreamResult>),
    ),
)]
pub(crate) async fn drama_kisskh_streams_doc() {}

#[utoipa::path(
    get,
    path = "/api/drama/dramacool/search",
    params(
        ("q" = String, Query, description = "Search query"),
    ),
    responses(
        (status = 200, description = "Dramacool search results"),
        (status = 500, description = "Internal search error"),
    ),
)]
pub(crate) async fn drama_dramacool_search_doc() {}

#[utoipa::path(
    get,
    path = "/api/drama/dramacool/streams/{id}/{episode}",
    params(
        ("id" = String, Path, description = "Dramacool drama ID"),
        ("episode" = u32, Path, description = "Episode number"),
    ),
    responses(
        (status = 200, description = "Stream URLs for requested episode", body = Vec<StreamResult>),
    ),
)]
pub(crate) async fn drama_dramacool_streams_doc() {}

// ── MovieBox endpoints (handled by Node.js microservice) ───────────────

#[utoipa::path(
    get,
    path = "/api/moviebox/home",
    responses((status = 200, description = "MovieBox home carousel and sections")),
)]
pub(crate) async fn moviebox_home_doc() {}

#[utoipa::path(
    get,
    path = "/api/moviebox/discover",
    responses((status = 200, description = "MovieBox discover content")),
)]
pub(crate) async fn moviebox_discover_doc() {}

#[utoipa::path(
    get,
    path = "/api/moviebox/hero",
    responses((status = 200, description = "MovieBox hero banner items")),
)]
pub(crate) async fn moviebox_hero_doc() {}

#[utoipa::path(
    get,
    path = "/api/moviebox/info",
    responses((status = 200, description = "MovieBox content details")),
)]
pub(crate) async fn moviebox_info_doc() {}

#[utoipa::path(
    get,
    path = "/api/moviebox/search",
    responses((status = 200, description = "MovieBox search results")),
)]
pub(crate) async fn moviebox_search_doc() {}

#[utoipa::path(
    get,
    path = "/api/moviebox/section",
    responses((status = 200, description = "MovieBox section content")),
)]
pub(crate) async fn moviebox_section_doc() {}

#[utoipa::path(
    get,
    path = "/api/moviebox/trending",
    responses((status = 200, description = "MovieBox trending content")),
)]
pub(crate) async fn moviebox_trending_doc() {}

#[utoipa::path(
    get,
    path = "/api/moviebox/popular-movie",
    responses((status = 200, description = "MovieBox popular movies")),
)]
pub(crate) async fn moviebox_popular_movie_doc() {}

#[utoipa::path(
    get,
    path = "/api/moviebox/popular-series",
    responses((status = 200, description = "MovieBox popular series")),
)]
pub(crate) async fn moviebox_popular_series_doc() {}

// ── Anime endpoints (handled by Node.js microservice) ──────────────────

#[utoipa::path(
    get,
    path = "/api/anime/search",
    responses((status = 200, description = "Anime search results")),
)]
pub(crate) async fn anime_search_doc() {}

#[utoipa::path(
    get,
    path = "/api/anime/home",
    responses((status = 200, description = "Anime home screen content")),
)]
pub(crate) async fn anime_home_doc() {}

#[utoipa::path(
    get,
    path = "/api/anime/latest",
    responses((status = 200, description = "Latest anime episodes")),
)]
pub(crate) async fn anime_latest_doc() {}

#[utoipa::path(
    get,
    path = "/api/anime/info/{slug}",
    params(
        ("slug" = String, Path, description = "Anime slug identifier"),
    ),
    responses((status = 200, description = "Anime detail information")),
)]
pub(crate) async fn anime_info_doc() {}

#[utoipa::path(
    get,
    path = "/api/anime/episodes/{id}",
    params(
        ("id" = String, Path, description = "Anime ID"),
    ),
    responses((status = 200, description = "Episode list for anime")),
)]
pub(crate) async fn anime_episodes_doc() {}

#[utoipa::path(
    get,
    path = "/api/anime/genres",
    responses((status = 200, description = "Available anime genres")),
)]
pub(crate) async fn anime_genres_doc() {}

#[utoipa::path(
    get,
    path = "/api/anime/genre/{genre_id}",
    params(
        ("genre_id" = String, Path, description = "Genre identifier"),
    ),
    responses((status = 200, description = "Anime filtered by genre")),
)]
pub(crate) async fn anime_genre_doc() {}

#[utoipa::path(
    get,
    path = "/api/anime/seasonal",
    responses((status = 200, description = "Seasonal anime lineup")),
)]
pub(crate) async fn anime_seasonal_doc() {}

#[utoipa::path(
    get,
    path = "/api/anime/top",
    responses((status = 200, description = "Top-ranked anime")),
)]
pub(crate) async fn anime_top_doc() {}

#[utoipa::path(
    get,
    path = "/api/anime/jikan/{mal_id}",
    params(
        ("mal_id" = String, Path, description = "MyAnimeList ID"),
    ),
    responses((status = 200, description = "Anime details from Jikan (MyAnimeList) API")),
)]
pub(crate) async fn anime_jikan_doc() {}

#[utoipa::path(
    get,
    path = "/api/anime/jikan/{mal_id}/characters",
    params(
        ("mal_id" = String, Path, description = "MyAnimeList ID"),
    ),
    responses((status = 200, description = "Character list for anime from Jikan API")),
)]
pub(crate) async fn anime_jikan_characters_doc() {}

#[utoipa::path(
    get,
    path = "/api/anime/jikan/{mal_id}/staff",
    params(
        ("mal_id" = String, Path, description = "MyAnimeList ID"),
    ),
    responses((status = 200, description = "Staff list for anime from Jikan API")),
)]
pub(crate) async fn anime_jikan_staff_doc() {}

// ── Main API document ──────────────────────────────────────────────────

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Huerust Engine API",
        version = "0.1.0",
        description = "High-performance Rust streaming aggregator engine. Exposes stream aggregation, HiTV discovery, drama search, proxy, and system endpoints."
    ),
    paths(
        // System
        crate::openapi::health_check_doc,
        crate::openapi::metrics_doc,
        crate::openapi::proxy_m3u8_doc,
        crate::openapi::proxy_ts_doc,
        crate::openapi::proxy_sub_doc,
        // Streams (have existing annotations in their own files)
        crate::routes::streams::streams_handler,
        crate::routes::sse::sse_streams_handler,
        // Providers
        crate::routes::providers::providers_handler,
        // Play
        crate::routes::play::play_handler,
        // Admin
        crate::openapi::admin_reload_doc,
        crate::openapi::admin_refresh_tokens_doc,
        // HiTV
        crate::openapi::hitv_library_doc,
        crate::openapi::hitv_browse_doc,
        crate::openapi::hitv_search_doc,
        crate::openapi::hitv_latest_doc,
        crate::openapi::hitv_trending_doc,
        crate::openapi::hitv_hero_doc,
        crate::openapi::hitv_drama_doc,
        crate::openapi::hitv_info_doc,
        crate::openapi::hitv_play_doc,
        crate::openapi::hitv_countries_doc,
        crate::openapi::hitv_categories_doc,
        crate::openapi::hitv_genres_doc,
        crate::openapi::hitv_sorts_doc,
        crate::openapi::hitv_dynamic_browse_doc,
        // Drama
        crate::openapi::drama_kisskh_search_doc,
        crate::openapi::drama_kisskh_streams_doc,
        crate::openapi::drama_dramacool_search_doc,
        crate::openapi::drama_dramacool_streams_doc,
        // MovieBox (handled by Node.js microservice)
        crate::openapi::moviebox_home_doc,
        crate::openapi::moviebox_discover_doc,
        crate::openapi::moviebox_hero_doc,
        crate::openapi::moviebox_info_doc,
        crate::openapi::moviebox_search_doc,
        crate::openapi::moviebox_section_doc,
        crate::openapi::moviebox_trending_doc,
        crate::openapi::moviebox_popular_movie_doc,
        crate::openapi::moviebox_popular_series_doc,
        // Anime (handled by Node.js microservice)
        crate::openapi::anime_search_doc,
        crate::openapi::anime_home_doc,
        crate::openapi::anime_latest_doc,
        crate::openapi::anime_info_doc,
        crate::openapi::anime_episodes_doc,
        crate::openapi::anime_genres_doc,
        crate::openapi::anime_genre_doc,
        crate::openapi::anime_seasonal_doc,
        crate::openapi::anime_top_doc,
        crate::openapi::anime_jikan_doc,
        crate::openapi::anime_jikan_characters_doc,
        crate::openapi::anime_jikan_staff_doc,
    ),
    components(schemas(StreamResult, Subtitle))
)]
pub struct ApiDoc;
