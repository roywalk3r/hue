//! Hue Stream API
//! Professional Streaming Aggregator
//!
//! @author roywalk3r <roywalker440@gmail.com>
//! @repository https://github.com/roywalk3r/hue.git
//! @license MIT

use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use serde::Serialize;
use std::collections::HashMap;
use tracing::{info, warn};
use utoipa::ToSchema;
use url::Url;

const DISABLED_PROVIDERS: &[&str] = &["4khdhub"];
const REDIRECTOR_HOSTS: &[&str] = &["hubcloud.cx", "hubcdn", "hubdrive", "hubic", "vixsrc.to"];

use crate::providers::dispatch::{
    ProviderTiming, fetch_from_providers, fetch_from_providers_matching, single_provider_registry,
};
use crate::providers::vdrk_subtitles::fetch_vdrk_subtitles;
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

fn is_web_playable_stream(stream: &StreamResult) -> bool {
    let format = stream.format.to_lowercase();
    let url = stream.url.to_lowercase();

    format != "dash" && !url.contains(".mpd") && !url.contains(".mkv")
}

fn should_fetch_slow_providers(web_client: bool, streams: &[StreamResult]) -> bool {
    streams.is_empty() || (web_client && !streams.iter().any(is_web_playable_stream))
}

fn filter_for_client(web_client: bool, streams: Vec<StreamResult>) -> Vec<StreamResult> {
    if !web_client {
        return streams;
    }

    streams.into_iter().filter(is_web_playable_stream).collect()
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
    let web_client = params
        .get("client")
        .is_some_and(|value| value.eq_ignore_ascii_case("web"));

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
            let filtered = filter_for_client(
                web_client,
                filter_generic_movie_tv_streams(
                    &m_type,
                    apply_filters(cached, &state.filter_config),
                ),
            );

            if filtered.is_empty() && web_client {
                info!("[api] Cached streams are not web-playable; fetching providers");
            } else {
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
                3,
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
        let (mut streams, mut timings) = if web_client {
            fetch_from_providers_matching(
                registry.clone(),
                metrics.clone(),
                &tmdb_info_c,
                &m_type_c,
                season,
                episode,
                false,
                1,
                slow_deadline,
                is_web_playable_stream,
            )
            .await
        } else {
            fetch_from_providers(
                registry.clone(),
                metrics.clone(),
                &tmdb_info_c,
                &m_type_c,
                season,
                episode,
                true,
                4,
                fast_deadline,
            )
            .await
        };
        if !web_client && should_fetch_slow_providers(false, &streams) {
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
            streams.extend(slow_streams);
            timings.extend(slow_timings);
        } else if !web_client {
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
                if web_client {
                    let (streams, _) = fetch_from_providers_matching(
                        registry.clone(),
                        metrics.clone(),
                        &tmdb_info_c,
                        &m_type_c,
                        season,
                        episode,
                        false,
                        1,
                        slow_deadline,
                        is_web_playable_stream,
                    )
                    .await;
                    let mut seen_urls = std::collections::HashSet::new();
                    let unique: Vec<_> = streams
                        .into_iter()
                        .filter(|stream| seen_urls.insert(stream.url.clone()))
                        .collect();
                    let filtered = filter_generic_movie_tv_streams(
                        &m_type_c,
                        apply_filters(unique, &filter_config),
                    );
                    info!(
                        "[api] Coalesced web fetch: {} streams in {:?}",
                        filtered.len(),
                        start.elapsed()
                    );
                    return filtered;
                }

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
                if should_fetch_slow_providers(false, &streams) {
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
                    streams.extend(slow_streams);
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
    let filtered = filter_for_client(web_client, filtered);

    let filtered = drop_unreachable(
        &state.client,
        filtered,
        state.config.head_check_timeout_secs,
        true,
    )
    .await;
    let filtered: Vec<StreamResult> = filtered
        .into_iter()
        .filter(|s| {
            let p = s.provider.to_lowercase();
            !DISABLED_PROVIDERS.iter().any(|d| p == *d)
        })
        .filter(|s| {
            let host = Url::parse(&s.url)
                .ok()
                .and_then(|u| u.host_str().map(|h| h.to_lowercase()))
                .unwrap_or_default();
            !REDIRECTOR_HOSTS.iter().any(|h| host.contains(h))
        })
        .collect();
    if filtered.is_empty() {
        return (StatusCode::NOT_FOUND, "No streams found").into_response();
    }

    let needs_subs = filtered.iter().any(|s| s.subtitles.is_none());
    let vdrk_subs = if needs_subs {
        let n = tmdb_id.parse::<u64>().unwrap_or(0);
        if n > 0 {
            fetch_vdrk_subtitles(&state.client, n, &m_type, season, episode).await
        } else {
            None
        }
    } else {
        None
    };
    let filtered: Vec<StreamResult> = filtered
        .into_iter()
        .map(|mut s| {
            if s.subtitles.is_none() {
                s.subtitles = vdrk_subs.clone();
            }
            s
        })
        .collect();

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

#[utoipa::path(
    get,
    path = "/api/streams/luna/{type}/{tmdbId}",
    params(
        ("type" = String, Path, description = "Content type: movie or tv"),
        ("tmdbId" = String, Path, description = "TMDB ID"),
        ("season" = Option<u32>, Query, description = "Season number (tv only)"),
        ("episode" = Option<u32>, Query, description = "Episode number (tv only)"),
        ("nocache" = Option<String>, Query, description = "Set to 1 to bypass cache"),
    ),
    responses(
        (status = 200, description = "Luna-only stream aggregation results", body = StreamsResponse),
        (status = 404, description = "TMDB info not found"),
        (status = 501, description = "Luna provider not registered"),
    ),
)]
pub async fn luna_streams_handler(
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
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(1);
    let no_cache = params.get("nocache").map(|v| v == "1").unwrap_or(false);

    info!(
        "[api] Rust Luna-only Streams: {} {} S{} E{}",
        &m_type, tmdb_id, season, episode
    );

    let cache_key = CacheKey::new(&format!("luna:{}", m_type), &tmdb_id, season, episode);
    let enable_proxy = state.config.enable_proxy;
    let proxy_base = dynamic_proxy_base(&headers, &state.proxy_base_url);
    let secret = state.secret.clone();

    if !no_cache {
        if let Some(cached) = state.cache.get(&cache_key).await {
            let filtered = filter_generic_movie_tv_streams(
                &m_type,
                apply_filters(cached, &state.filter_config),
            );
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

    let registry = match single_provider_registry(&state.registry, "vidrock") {
        Some(r) => r,
        None => {
            return (StatusCode::NOT_IMPLEMENTED, "Luna provider not registered").into_response();
        }
    };

    let (streams, timings) = fetch_from_providers(
        registry,
        state.metrics.clone(),
        &tmdb_info,
        &m_type,
        season,
        episode,
        false,
        1,
        state.config.slow_deadline(),
    )
    .await;

    let filtered =
        filter_generic_movie_tv_streams(&m_type, apply_filters(streams, &state.filter_config));
    if filtered.is_empty() {
        return (StatusCode::NOT_FOUND, "No streams found").into_response();
    }

    // Keep cache entries raw. Proxy wrapping and kkey encryption are
    // response-specific and must be regenerated for each request.
    state.cache.set(&cache_key, filtered.clone()).await;

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
            provider_timings: timings,
        }),
    )
        .into_response()
}

#[utoipa::path(
    get,
    path = "/api/streams/xyra/{type}/{tmdbId}",
    params(
        ("type" = String, Path, description = "Content type: movie or tv"),
        ("tmdbId" = String, Path, description = "TMDB ID"),
        ("season" = Option<u32>, Query, description = "Season number (tv only)"),
        ("episode" = Option<u32>, Query, description = "Episode number (tv only)"),
    ),
    responses(
        (status = 200, description = "Xyra fast-path stream results", body = StreamsResponse),
        (status = 502, description = "Xyra upstream error"),
        (status = 504, description = "Xyra upstream timeout"),
    ),
)]
pub async fn xyra_streams_handler(
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
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(1);
    let xyra_type = if m_type == "movie" { "movie" } else { "series" };

    let enable_proxy = state.config.enable_proxy;
    let proxy_base = dynamic_proxy_base(&headers, &state.proxy_base_url);
    let secret = state.secret.clone();

    let mut url = format!(
        "https://api.xyra.stream/v1/streamhub/streams?api_key=key1&tmdb_id={}&type={}",
        tmdb_id, xyra_type
    );
    if xyra_type == "series" {
        url.push_str(&format!("&season={}&episode={}", season, episode));
    }

    info!(
        "[api] Xyra fast-path: {} {} S{} E{}",
        m_type, tmdb_id, season, episode
    );

    let resp = match tokio::time::timeout(
        std::time::Duration::from_secs(12),
        state.client.get(&url).send(),
    )
    .await
    {
        Ok(Ok(r)) if r.status().is_success() => r,
        Ok(Ok(r)) => {
            warn!("[api] Xyra returned {}", r.status());
            return (StatusCode::BAD_GATEWAY, "xyra upstream error").into_response();
        }
        Ok(Err(e)) => {
            warn!("[api] Xyra request failed: {}", e);
            return (StatusCode::BAD_GATEWAY, format!("xyra error: {e}")).into_response();
        }
        Err(_) => {
            warn!("[api] Xyra request timed out after 8s");
            return (StatusCode::GATEWAY_TIMEOUT, "xyra timeout").into_response();
        }
    };

    #[derive(serde::Deserialize)]
    struct XyraStream {
        url: String,
        #[serde(default)]
        title: Option<String>,
        #[serde(default)]
        quality: Option<String>,
        #[serde(default)]
        provider: Option<String>,
        #[serde(default)]
        headers: Option<HashMap<String, String>>,
    }
    #[derive(serde::Deserialize)]
    struct XyraResp {
        #[serde(default)]
        success: bool,
        #[serde(default)]
        streams: Vec<XyraStream>,
        #[serde(default)]
        provider_timings: HashMap<String, u64>,
    }

    let body: XyraResp = match resp.json().await {
        Ok(b) => b,
        Err(e) => {
            warn!("[api] Xyra json decode failed: {}", e);
            return (StatusCode::BAD_GATEWAY, "xyra bad response").into_response();
        }
    };
    if !body.success {
        return (
            StatusCode::OK,
            Json(StreamsResponse {
                success: true,
                tmdb_id,
                media_type: m_type,
                season,
                episode,
                count: 0,
                streams: vec![],
                provider_timings: vec![],
            }),
        )
            .into_response();
    }

    let mut timings: Vec<ProviderTiming> = body
        .provider_timings
        .into_iter()
        .map(|(provider, elapsed_ms)| ProviderTiming {
            provider,
            elapsed_ms,
            streams_found: 0,
            cancelled: false,
        })
        .collect();
    let mut seen_timings: HashMap<String, usize> = timings
        .iter()
        .enumerate()
        .map(|(i, t)| (t.provider.clone(), i))
        .collect();

    let provider_counts: HashMap<String, usize> =
        body.streams
            .iter()
            .filter_map(|s| s.provider.clone())
            .fold(HashMap::new(), |mut acc, p| {
                *acc.entry(p).or_insert(0) += 1;
                acc
            });
    for (p, c) in provider_counts {
        if let Some(i) = seen_timings.get(&p) {
            timings[*i].streams_found = c;
        } else {
            timings.push(ProviderTiming {
                provider: p,
                elapsed_ms: 0,
                streams_found: c,
                cancelled: false,
            });
        }
    }

    let streams: Vec<StreamResult> = body
        .streams
        .into_iter()
        .filter(|s| {
            s.provider
                .as_deref()
                .map(|p| !DISABLED_PROVIDERS.contains(&p.to_lowercase().as_str()))
                .unwrap_or(true)
        })
        .filter(|s| {
            let host = Url::parse(&s.url)
                .ok()
                .and_then(|u| u.host_str().map(|h| h.to_lowercase()))
                .unwrap_or_default();
            !REDIRECTOR_HOSTS.iter().any(|h| host.contains(h))
        })
        .map(|s| {
            let lower = s.url.to_lowercase();
            let format = if lower.contains(".m3u8") || lower.contains("/playlist/") {
                "hls".to_string()
            } else if lower.contains(".mp4") || lower.contains(".mkv") {
                "mp4".to_string()
            } else {
                "hls".to_string()
            };
            StreamResult {
                url: s.url,
                title: s.title.unwrap_or_default(),
                quality: s.quality.unwrap_or_else(|| "Auto".to_string()),
                provider: s.provider.unwrap_or_else(|| "Xyra".to_string()),
                format,
                subtitles: None,
                headers: s.headers,
                extra_metadata: None,
            }
        })
        .collect();

    let tmdb_id_num = tmdb_id.parse::<u64>().unwrap_or(0);
    let subs = if tmdb_id_num > 0 {
        fetch_vdrk_subtitles(&state.client, tmdb_id_num, &m_type, season, episode).await
    } else {
        None
    };
    let streams: Vec<StreamResult> = streams
        .into_iter()
        .map(|mut s| {
            if s.subtitles.is_none() {
                s.subtitles = subs.clone();
            }
            s
        })
        .collect();

    let mut final_streams = if enable_proxy {
        wrap_with_proxy(streams, &proxy_base, &secret)
    } else {
        streams
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
            provider_timings: timings,
        }),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::{is_web_playable_stream, should_fetch_slow_providers};
    use crate::models::StreamResult;

    fn stream(provider: &str, format: &str, url: &str) -> StreamResult {
        StreamResult {
            url: url.to_string(),
            title: String::new(),
            quality: String::new(),
            provider: provider.to_string(),
            format: format.to_string(),
            subtitles: None,
            headers: None,
            extra_metadata: None,
        }
    }

    #[test]
    fn web_playability_excludes_known_browser_incompatible_sources() {
        // NoTorrent mp4 from a non-mkv URL is web-playable — its slow mirrors
        // and EAC3 audio are caught at the HLS layer, not by a provider-name
        // filter. Provider name is no longer part of the check.
        assert!(is_web_playable_stream(&stream(
            "NoTorrent",
            "mp4",
            "https://example.com/video"
        )));
        assert!(!is_web_playable_stream(&stream(
            "Nova",
            "dash",
            "https://example.com/manifest.mpd"
        )));
        assert!(!is_web_playable_stream(&stream(
            "NoTorrent",
            "mp4",
            "https://example.com/video.mkv"
        )));
        assert!(is_web_playable_stream(&stream(
            "VidZee",
            "hls",
            "https://example.com/master.m3u8"
        )));
    }

    #[test]
    fn web_client_fetches_slow_providers_when_fast_results_are_unplayable() {
        let mkv = vec![stream("NoTorrent", "mp4", "https://example.com/video.mkv")];
        let dash = vec![stream("Nova", "dash", "https://example.com/manifest.mpd")];
        let hls = vec![stream("VidZee", "hls", "https://example.com/master.m3u8")];

        assert!(should_fetch_slow_providers(true, &mkv));
        assert!(should_fetch_slow_providers(true, &dash));
        assert!(!should_fetch_slow_providers(false, &mkv));
        assert!(!should_fetch_slow_providers(true, &hls));
    }
}
