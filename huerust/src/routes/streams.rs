//! Hue Stream API
//! Professional Streaming Aggregator
//!
//! @author roywalk3r <roywalker440@gmail.com>
//! @repository https://github.com/roywalk3r/hue.git
//! @license MIT

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use std::collections::HashMap;
use tracing::{info, warn};
use utoipa::ToSchema;

use crate::providers::dispatch::{fetch_from_providers, ProviderTiming};
use crate::proxy::dynamic_proxy_base;
use crate::proxy::wrap::wrap_with_proxy;
use crate::{
    app::StreamState, filters::apply_filters, infra::cache::CacheKey, models::StreamResult,
    security::encrypt_stream_result_urls,
};

#[derive(Serialize, ToSchema)]
pub struct StreamsResponse {
    pub success: bool,
    pub tmdb_id: String,
    pub media_type: String,
    pub season: u32,
    pub episode: u32,
    pub count: usize,
    pub streams: Vec<StreamResult>,
    pub provider_timings: Vec<ProviderTiming>,
}

fn filter_generic_movie_tv_streams(
    media_type: &str,
    streams: Vec<StreamResult>,
) -> Vec<StreamResult> {
    if media_type != "movie" && media_type != "tv" {
        return streams;
    }

    streams
        .into_iter()
        .filter(|s| {
            let provider = s.provider.to_lowercase();
            provider != "zenith" && provider != "orient"
        })
        .collect()
}

#[utoipa::path(
    get,
    path = "/api/streams/{type}/{tmdbId}",
    params(
        ("type" = String, Path, description = "Content type: movie or tv"),
        ("tmdbId" = String, Path, description = "TMDB ID"),
        ("season" = Option<u32>, Query, description = "Season number (tv only)"),
        ("episode" = Option<u32>, Query, description = "Episode number (tv only)"),
        ("nocache" = Option<String>, Query, description = "Set to 1 to bypass cache"),
    ),
    responses(
        (status = 200, description = "Stream aggregation results", body = StreamsResponse),
        (status = 404, description = "TMDB info not found"),
    ),
)]
pub async fn streams_handler(
    State(state): State<StreamState>,
    headers: HeaderMap,
    Path((m_type, tmdb_id)): Path<(String, String)>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    let season = params
        .get("season")
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(1);
    let episode = params
        .get("episode")
        .and_then(|e| e.parse::<u32>().ok())
        .unwrap_or(1);
    let no_cache = params.get("nocache").map(|v| v == "1").unwrap_or(false);

    info!(
        "[api] Rust Streams: {} {} S{} E{}",
        &m_type, tmdb_id, season, episode
    );

    let cache_key = CacheKey::new(&m_type, &tmdb_id, season, episode);
    let enable_proxy = state.config.enable_proxy;
    let proxy_base = dynamic_proxy_base(&headers, &state.proxy_base_url);
    let secret = state.secret.clone();

    // 1. Check cache
    if !no_cache {
        if let Some(cached) = state.cache.get(&cache_key).await {
            let filtered = filter_generic_movie_tv_streams(
                &m_type,
                apply_filters(cached, &state.filter_config),
            );

            // Proxy wrap even on cache hits
            let mut final_streams = if enable_proxy {
                wrap_with_proxy(filtered, &proxy_base, &secret)
            } else {
                filtered
            };
            crate::security::re_encrypt_cached_urls(&mut final_streams, &state.kkey);

            return (
                StatusCode::OK,
                Json(StreamsResponse {
                    success: true,
                    tmdb_id: tmdb_id.clone(),
                    media_type: m_type.clone(),
                    season,
                    episode,
                    count: final_streams.len(),
                    streams: final_streams,
                    provider_timings: vec![],
                }),
            )
                .into_response();
        }
    }

    // 2. Fetch TMDB details
    let tmdb_info = match state
        .tmdb
        .get_details(&m_type, tmdb_id.parse().unwrap_or(0))
        .await
    {
        Ok(info) => info,
        Err(e) => {
            warn!("[api] TMDB lookup failed for {}: {}", tmdb_id, e);
            return (StatusCode::NOT_FOUND, "TMDB info not found").into_response();
        }
    };

    // 3. Coalesce concurrent requests for the same content (skip if no_cache)
    let cache = state.cache.clone();
    let filter_config = state.filter_config.clone();
    let registry = state.registry.clone();
    let tmdb_info_c = tmdb_info.clone();
    let m_type_c = m_type.clone();
    let fast_deadline = state.config.fast_deadline();
    let slow_deadline = state.config.slow_deadline();
    let metrics = state.metrics.clone();

    let mut captured_timings: Vec<ProviderTiming> = vec![];

    // ── Background fill: after fast return, keep fetching all providers ──
    /// Drop streams whose upstream fails a quick reachability probe. Without
    /// this the player ranks dead 1080p mirrors above live 480p ones and
    /// burns 8s per corpse before reaching a playable stream.
    async fn drop_unreachable(
        client: &reqwest_middleware::ClientWithMiddleware,
        streams: Vec<StreamResult>,
        timeout_secs: u64,
        hls_only: bool,
    ) -> Vec<StreamResult> {
        // hls_only: the inline response path probes just manifests (fast
        // HEADs that catch dead worker hosts). Slow mp4 mirror probes are
        // the ones that eat the 5s budget, and the player rotates past a
        // dead mp4 in ~2s anyway — only the background fill (which feeds
        // the cache) pays for the full pass.
        let checks = futures_util::future::join_all(streams.iter().map(|s| async move {
            let is_hls = s.format == "hls" || s.url.to_lowercase().contains(".m3u8");
            if hls_only && !is_hls {
                return true;
            }
            crate::routes::sse::is_stream_reachable(client, s, timeout_secs).await
        }))
        .await;
        let before = streams.len();
        // Fail open: if every probe failed the problem is more likely the
        // probe (slow mirrors, egress path) than all upstreams at once —
        // a possibly-stale list beats a guaranteed 404.
        if !checks.iter().any(|&ok| ok) {
            warn!(
                "[api] Reachability: all {} probes failed — serving unfiltered list",
                before
            );
            return streams;
        }
        let alive: Vec<StreamResult> = streams
            .into_iter()
            .zip(checks)
            .filter(|(_, ok)| *ok)
            .map(|(s, _)| s)
            .collect();
        if alive.len() < before {
            info!(
                "[api] Reachability: dropped {} dead streams, {} remain",
                before - alive.len(),
                alive.len()
            );
        }
        alive
    }

    fn spawn_background_fill(
        state: StreamState,
        cache_key: CacheKey,
        tmdb_info: crate::clients::tmdb::TmdbNormalizedInfo,
        m_type: String,
        season: u32,
        episode: u32,
        _fast_deadline: std::time::Duration,
        slow_deadline: std::time::Duration,
    ) {
        tokio::spawn(async move {
            let start = std::time::Instant::now();
            let (streams, _) = fetch_from_providers(
                state.registry.clone(),
                state.metrics.clone(),
                &tmdb_info,
                &m_type,
                season,
                episode,
                false, // all providers
                1,
                slow_deadline,
            )
            .await;
            let mut seen = std::collections::HashSet::new();
            let unique: Vec<_> = streams
                .into_iter()
                .filter(|s| seen.insert(s.url.clone()))
                .collect();
            let filtered = filter_generic_movie_tv_streams(
                &m_type,
                apply_filters(unique, &state.filter_config),
            );
            let filtered = drop_unreachable(
                &state.client,
                filtered,
                state.config.head_check_timeout_secs,
                false,
            )
            .await;
            let count = filtered.len();
            // A thin round (one flaky provider answered) would lock a bad
            // list into the cache for the full TTL. Leave the cache empty
            // instead so the next request re-fetches until a full round lands.
            if count >= 3 {
                state.cache.set(&cache_key, filtered).await;
                info!(
                    "[api] Background fill: cached {} streams in {:?}",
                    count,
                    start.elapsed()
                );
            } else if count > 0 {
                info!(
                    "[api] Background fill: only {} streams — not caching thin round",
                    count
                );
            }
        });
    }

    let coalesced = if no_cache {
        // Bypass coalescer — start all providers simultaneously, per-provider timeouts apply.
        let start = std::time::Instant::now();
        let (mut streams, mut timings) = fetch_from_providers(
            registry.clone(),
            metrics.clone(),
            &tmdb_info_c,
            &m_type_c,
            season,
            episode,
            true,
            // Wait for ALL fast providers (Luna + Aura + Moviebox + ...) to
            // finish before aborting — the fast phase is meant to be a
            // parallel race, not a "first one wins" cancel. Once we have
            // a base of fast streams, fall back to slow providers to fill
            // gaps. Without this, the threshold of 1 was killing Aura
            // before it could return Vixsrc streams.
            4,
            fast_deadline,
        )
        .await;
        if streams.is_empty() {
            let (slow_streams, slow_timings) = fetch_from_providers(
                registry.clone(),
                metrics.clone(),
                &tmdb_info_c,
                &m_type_c,
                season,
                episode,
                false,
                1,
                slow_deadline,
            )
            .await;
            streams = slow_streams;
            timings.extend(slow_timings);
        } else {
            // Fast stream found — return immediately and backfill cache in background.
            spawn_background_fill(
                state.clone(),
                cache_key.clone(),
                tmdb_info_c.clone(),
                m_type_c.clone(),
                season,
                episode,
                fast_deadline,
                slow_deadline,
            );
        }
        captured_timings = timings;
        let mut seen_urls = std::collections::HashSet::new();
        let unique: Vec<_> = streams
            .into_iter()
            .filter(|s| seen_urls.insert(s.url.clone()))
            .collect();
        let filtered =
            filter_generic_movie_tv_streams(&m_type_c, apply_filters(unique, &filter_config));
        info!(
            "[api] No-cache fresh fetch: {} streams in {:?}",
            filtered.len(),
            start.elapsed()
        );
        Some(filtered)
    } else {
        let state_for_bg = state.clone();
        let cache_key_for_bg = cache_key.clone();
        state
            .coalescer
            .execute(&cache_key, &cache, || async move {
                let start = std::time::Instant::now();
                // Fast providers first so a working source can reach the player quickly.
                let (mut streams, _) = fetch_from_providers(
                    registry.clone(),
                    metrics.clone(),
                    &tmdb_info_c,
                    &m_type_c,
                    season,
                    episode,
                    true,
                    // See comment in the no_cache branch above.
                    4,
                    fast_deadline,
                )
                .await;
                if streams.is_empty() {
                    let (slow_streams, _) = fetch_from_providers(
                        registry.clone(),
                        metrics.clone(),
                        &tmdb_info_c,
                        &m_type_c,
                        season,
                        episode,
                        false,
                        1,
                        slow_deadline,
                    )
                    .await;
                    streams = slow_streams;
                } else {
                    // Fast stream found — backfill cache in background.
                    spawn_background_fill(
                        state_for_bg,
                        cache_key_for_bg,
                        tmdb_info_c.clone(),
                        m_type_c.clone(),
                        season,
                        episode,
                        fast_deadline,
                        slow_deadline,
                    );
                }
                let mut seen_urls = std::collections::HashSet::new();
                let unique: Vec<_> = streams
                    .into_iter()
                    .filter(|s| seen_urls.insert(s.url.clone()))
                    .collect();
                let filtered = filter_generic_movie_tv_streams(
                    &m_type_c,
                    apply_filters(unique, &filter_config),
                );
                info!(
                    "[api] Coalesced fetch: {} streams in {:?}",
                    filtered.len(),
                    start.elapsed()
                );
                filtered
            })
            .await
    };

    let filtered = match coalesced {
        Some(v) => v,
        None => return (StatusCode::NOT_FOUND, "No streams found").into_response(),
    };

    let filtered = drop_unreachable(
        &state.client,
        filtered,
        state.config.head_check_timeout_secs,
        true,
    )
    .await;
    if filtered.is_empty() {
        return (StatusCode::NOT_FOUND, "No streams found").into_response();
    }

    let mut final_streams = if enable_proxy {
        wrap_with_proxy(filtered, &proxy_base, &secret)
    } else {
        filtered
    };
    encrypt_stream_result_urls(&mut final_streams, &state.kkey);

    (
        StatusCode::OK,
        Json(StreamsResponse {
            success: true,
            tmdb_id,
            media_type: m_type,
            season,
            episode,
            count: final_streams.len(),
            streams: final_streams,
            provider_timings: captured_timings,
        }),
    )
        .into_response()
}
