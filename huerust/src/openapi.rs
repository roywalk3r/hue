//! Hue Stream API
//! Professional Streaming Aggregator
//!
//! @author roywalk3r <roywalker440@gmail.com>
//! @repository https://github.com/roywalk3r/hue.git
//! @license MIT

use crate::models::{StreamResult, Subtitle};
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Huerust Engine API",
        version = "0.1.0",
        description = "High-performance Rust streaming aggregator engine. Exposes stream aggregation, HiTV discovery, drama search, and proxy endpoints."
    ),
    paths(
        // system / health
        crate::routes::system::health_check,
        crate::routes::system::health_version,
        crate::routes::system::metrics_handler,
        crate::routes::system::hitv_library_handler,
        crate::routes::system::admin_reload_handler,
        crate::routes::system::admin_refresh_tokens_handler,
        crate::routes::system::status_handler,
        crate::routes::system::auth_token_handler,
        // providers
        crate::routes::providers::providers_handler,
        // streams
        crate::routes::streams::streams_handler,
        crate::routes::streams::luna_streams_handler,
        crate::routes::streams::xyra_streams_handler,
        // sse
        crate::routes::sse::sse_streams_handler,
        crate::routes::sse::report_failure_handler,
        crate::routes::sse::warm_handler,
        // play
        crate::routes::play::play_handler,
        // download
        crate::routes::download::download_handler,
        crate::routes::download::hls_download_handler,
        // anime
        crate::routes::anime::search_handler,
        crate::routes::anime::latest_handler,
        crate::routes::anime::home_handler,
        crate::routes::anime::detail_handler,
        crate::routes::anime::episodes_handler,
        crate::routes::anime::servers_handler,
        crate::routes::anime::stream_handler,
        crate::routes::anime::discover_get_handler,
        crate::routes::anime::discover_post_handler,
        crate::routes::anime::filter_options_handler,
        crate::routes::anime::schedule_handler,
        crate::routes::anime::category_handler,
        crate::routes::anime::filter_handler,
        // hitv
        crate::routes::hitv::browse_handler,
        crate::routes::hitv::search_handler,
        crate::routes::hitv::latest_handler,
        crate::routes::hitv::trending_handler,
        crate::routes::hitv::hero_handler,
        crate::routes::hitv::drama_handler,
        crate::routes::hitv::info_handler,
        crate::routes::hitv::play_handler,
        crate::routes::hitv::countries_handler,
        crate::routes::hitv::categories_handler,
        crate::routes::hitv::genres_handler,
        crate::routes::hitv::sorts_handler,
        crate::routes::hitv::dynamic_browse_handler,
        // drama
        crate::routes::drama::kisskh_search_handler,
        crate::routes::drama::kisskh_streams_handler,
        crate::routes::drama::kisskh_most_view_handler,
        crate::routes::drama::kisskh_last_update_handler,
        crate::routes::drama::kisskh_detail_handler,
        crate::routes::drama::kisskh_show_handler,
        crate::routes::drama::kisskh_browse_handler,
        crate::routes::drama::kisskh_upcoming_handler,
        crate::routes::drama::kisskh_top_rating_handler,
        crate::routes::drama::dramacool_search_handler,
        crate::routes::drama::dramacool_streams_handler,
        crate::routes::drama::kisskh_xyra_search_handler,
        crate::routes::drama::kisskh_xyra_streams_handler,
        crate::routes::drama::kisskh_xyra_home_handler,
        crate::routes::drama::kissasian_search_handler,
        crate::routes::drama::kissasian_top_search_handler,
        crate::routes::drama::kissasian_top_browse_handler,
        crate::routes::drama::kissasian_top_info_handler,
        crate::routes::drama::kissasian_top_streams_handler,
        crate::routes::drama::kissasian_streams_handler,
        crate::routes::drama::moviebox_browse_handler,
        crate::routes::drama::moviebox_search_handler,
        crate::routes::drama::moviebox_streams_handler,
        crate::routes::drama::moviebox_streams_direct_handler,
        crate::routes::drama::drama_detail_handler,
        crate::routes::drama::moviebox_detail_rec_handler,
        // moviebox
        crate::routes::moviebox::home_handler,
        crate::routes::moviebox::discover_handler,
        crate::routes::moviebox::section_handler,
        crate::routes::moviebox::trending_handler,
        crate::routes::moviebox::popular_movie_handler,
        crate::routes::moviebox::popular_series_handler,
        crate::routes::moviebox::hot_shorts_handler,
        crate::routes::moviebox::sa_handler,
        crate::routes::moviebox::thai_handler,
        crate::routes::moviebox::turkish_handler,
        crate::routes::moviebox::western_handler,
        crate::routes::moviebox::animation_handler,
        crate::routes::moviebox::black_shows_handler,
        crate::routes::moviebox::hero_handler,
        crate::routes::moviebox::bollywood_handler,
        crate::routes::moviebox::shorts_handler,
        crate::routes::moviebox::filter_handler,
        crate::routes::moviebox::info_handler,
        crate::routes::moviebox::detail_handler,
        crate::routes::moviebox::search_handler,
        // tmdb
        crate::routes::tmdb::home_handler,
        crate::routes::tmdb::movie_popular_handler,
        crate::routes::tmdb::movie_top_rated_handler,
        crate::routes::tmdb::movie_now_playing_handler,
        crate::routes::tmdb::tv_popular_handler,
        crate::routes::tmdb::tv_top_rated_handler,
        crate::routes::tmdb::tv_on_the_air_handler,
        crate::routes::tmdb::trending_handler,
        crate::routes::tmdb::discover_handler,
        crate::routes::tmdb::search_handler,
        // subtitle
        crate::routes::subtitle::subtitle_search_handler,
        crate::routes::subtitle::subtitle_download_handler,
        // sports (all 10 — including the 9 that were previously unregistered)
        crate::routes::sports::leagues_handler,
        crate::routes::sports::league_tabs_handler,
        crate::routes::sports::matches_handler,
        crate::routes::sports::grouped_matches_handler,
        crate::routes::sports::live_matches_handler,
        crate::routes::sports::filter_tabs_handler,
        crate::routes::sports::home_handler,
        crate::routes::sports::latest_handler,
        crate::routes::sports::streams_handler,
        crate::routes::sports::detail_handler,
    ),
    components(schemas(
        // core models
        StreamResult,
        Subtitle,
        // providers / dispatch
        crate::clients::aisports::SportsStream,
        crate::providers::dispatch::ProviderTiming,
        // system
        crate::routes::system::AuthTokenResponse,
        // streams
        crate::routes::streams::StreamsResponse,
        // play
        crate::routes::play::PlayResponse,
        // providers
        crate::routes::providers::ProviderInfo,
        // anime
        crate::models::anime::AnimeCard,
        crate::models::anime::AnimeDetail,
        crate::models::anime::EpisodeSummary,
        crate::models::anime::ServerEntry,
        crate::models::anime::ServerList,
        crate::models::anime::StreamPayload,
        crate::models::anime::SubtitleTrack,
        crate::models::anime::SkipRange,
        crate::models::anime::AnimeHomeFeed,
        crate::models::anime::AnimeSearchResponse,
        crate::models::anime::FilterChoice,
        crate::models::anime::FilterOptions,
        crate::models::anime::FilterParams,
        crate::models::anime::ScheduleEntry,
        crate::models::anime::EpisodeListResponse,
        crate::routes::anime::LegacyFilterBody,
        // hitv
        crate::clients::hitv::HiTvItem,
        crate::routes::hitv::HitvListResponse,
        crate::routes::hitv::HitvDetailResponse,
        crate::routes::hitv::StaticListResponse,
        // tmdb
        crate::clients::tmdb::TmdbNormalizedInfo,
        crate::routes::tmdb::TmdbListResponse,
        crate::routes::tmdb::TmdbHomeResponse,
        // sse
        crate::routes::sse::ReportFailureBody,
        // sports
        crate::routes::sports::SportsStreamQuery,
        crate::routes::sports::LatestMatchesQuery,
        crate::routes::sports::LeagueMatchesQuery,
        crate::routes::sports::GroupedMatchesQuery,
        crate::routes::sports::MatchFilterTabsQuery,
        crate::routes::sports::MatchDetailPath,
        crate::routes::sports::SportsStreamsResponse,
        crate::routes::sports::SportsDataResponse,
        crate::routes::sports::SportsDetailResponse,
    ))
)]
pub struct ApiDoc;
