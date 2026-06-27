//! Hue Stream API
//! Professional Streaming Aggregator
//!
//! @author roywalk3r <roywalker440@gmail.com>
//! @repository https://github.com/roywalk3r/hue.git
//! @license MIT

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::sse::{Event, KeepAlive, Sse},
};
use serde_json::json;
use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::timeout;
use tokio_stream::wrappers::ReceiverStream;
use tracing::{info, warn};

use futures_util::future::join_all;

use crate::{
    app::StreamState,
    filters::apply_filters,
    infra::{
        cache::CacheKey,
        sticky_pref::{StickyPrefEntry, StickyPrefKey},
    },
    models::StreamResult,
    providers::StreamProvider,
    proxy::dynamic_proxy_base,
    proxy::wrap::wrap_with_proxy,
    security::encrypt_stream_result_urls,
};

fn is_generic_movie_tv_provider_allowed(media_type: &str, real_name: &str) -> bool {
    if media_type != "movie" && media_type != "tv" {
        return true;
    }

    real_name != "kisskh"
        && real_name != "dramacool"
        && real_name != "kissasian"
        && real_name != "vidvault"
}

/// HEAD-check a stream manifest/URL. Returns true for HTTP 2xx/206 responses
/// within the configured timeout. HLS manifests are checked with a plain GET/HEAD;
/// MP4/direct files use a Range request to avoid downloading the whole file.
async fn is_stream_reachable(client: &reqwest_middleware::ClientWithMiddleware, stream: &StreamResult, timeout_secs: u64) -> bool {
    let is_hls = stream.format == "hls" || stream.url.to_lowercase().contains(".m3u8");
    let mut req = if is_hls {
        client.head(&stream.url)
    } else {
        client.get(&stream.url).header("Range", "bytes=0-1")
    };
    if let Some(ref headers) = stream.headers {
        for (k, v) in headers {
            req = req.header(k, v);
        }
    }
    match timeout(Duration::from_secs(timeout_secs), req.send()).await {
        Ok(Ok(resp)) => {
            let status = resp.status();
            status.is_success() || status.as_u16() == 206
        }
        Ok(Err(e)) => {
            warn!("[SSE] HEAD check failed for {}: {}", stream.provider, e);
            false
        }
        Err(_) => {
            warn!("[SSE] HEAD check timed out for {}", stream.provider);
            false
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/streams/{type}/{tmdbId}/live",
    params(
        ("type" = String, Path, description = "Content type: movie or tv"),
        ("tmdbId" = String, Path, description = "TMDB ID"),
        ("season" = Option<u32>, Query, description = "Season number (tv only)"),
        ("episode" = Option<u32>, Query, description = "Episode number (tv only)"),
        ("nocache" = Option<String>, Query, description = "Set to 1 to bypass cache"),
    ),
    responses(
        (status = 200, description = "SSE stream with provider results as they arrive"),
        (status = 404, description = "TMDB info not found"),
    ),
)]
pub async fn sse_streams_handler(
    State(state): State<StreamState>,
    headers: HeaderMap,
    Path((m_type, tmdb_id)): Path<(String, String)>,
    Query(params): Query<HashMap<String, String>>,
) -> Sse<ReceiverStream<Result<Event, Infallible>>> {
    let season = params
        .get("season")
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(1);
    let episode = params
        .get("episode")
        .and_then(|e| e.parse::<u32>().ok())
        .unwrap_or(1);
    let no_cache = params.get("nocache").map(|v| v == "1").unwrap_or(false);

    let proxy_base = dynamic_proxy_base(&headers, &state.proxy_base_url);
    let (tx, rx) = mpsc::channel::<Result<Event, Infallible>>(64);

    let user_id = headers
        .get_all(axum::http::header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|s| s.split(';'))
        .map(str::trim)
        .find_map(|pair| {
            let (k, v) = pair.split_once('=')?;
            if k == "hue_anon" {
                Some(v.to_string())
            } else {
                None
            }
        })
        .unwrap_or_else(|| "anon".to_string());

    info!(
        "[SSE] request user={} type={} tmdb={} S{}E{} nocache={}",
        user_id, m_type, tmdb_id, season, episode, no_cache
    );

    tokio::spawn(async move {
        let cache_key = CacheKey::new(&m_type, &tmdb_id, season, episode);
        let sticky_key = StickyPrefKey::new(&user_id, &m_type, &tmdb_id);
        let cb = state.registry.circuit_breaker();

        // Fetch TMDB info once — needed for cache refresh, sticky path, and fan-out
        let tmdb_info = match state
            .tmdb
            .get_details(&m_type, tmdb_id.parse().unwrap_or(0))
            .await
        {
            Ok(info) => info,
            Err(e) => {
                let _ = tx
                    .send(Ok(Event::default().event("error").data(e.to_string())))
                    .await;
                return;
            }
        };

        // Serve from cache immediately if available (only if nocache is not set)
        if !no_cache {
            if let Some(cached) = state.cache.get(&cache_key).await {
                let filtered = apply_filters(cached, &state.filter_config)
                    .into_iter()
                    .filter(|s| {
                        let provider = s.provider.to_lowercase();
                        provider != "zenith" && provider != "orient"
                    })
                    .collect::<Vec<_>>();

                let mut streams = if state.config.enable_proxy {
                    wrap_with_proxy(filtered, &proxy_base, &state.secret)
                } else {
                    filtered
                };
                encrypt_stream_result_urls(&mut streams, &state.kkey);
                let count = streams.len();
                info!(
                    "[SSE] cache hit for {}/{}: {} streams",
                    m_type, tmdb_id, count
                );
                let _ = tx
                    .send(Ok(Event::default().event("cached").data(
                        json!({"streams": streams, "count": count, "cached": true}).to_string(),
                    )))
                    .await;
                let _ = tx
                    .send(Ok(Event::default()
                        .event("done")
                        .data(json!({"total": count, "cached": true}).to_string())))
                    .await;
                return;
            }
        }

        // Sticky fast-path: if this user has a recent successful provider
        // for this title, try just that one with a short timeout before
        // fanning out to every provider.
        let mut sticky_hit: Option<StickyPrefEntry> = None;
        if let Some(entry) = state.sticky_pref.get(&sticky_key).await {
            if entry.consecutive_failures == 0 && cb.is_healthy(&entry.provider_real_name) {
                sticky_hit = Some(entry);
            } else {
                state.sticky_pref.record_failure(&sticky_key).await;
            }
        }

        // Global score fallback: if no sticky preference exists, try the
        // globally most successful provider for this title.
        if sticky_hit.is_none() {
            if let Some(top_provider) = state
                .provider_score
                .get_top_provider(&m_type, &tmdb_id)
                .await
            {
                if cb.is_healthy(&top_provider) {
                    sticky_hit = Some(crate::infra::sticky_pref::StickyPrefEntry::fresh(
                        &top_provider,
                    ));
                }
            }
        }

        // Shared state for the 10s partial event and for counting sticky results.
        let stream_count = Arc::new(AtomicUsize::new(0));
        let done_sent = Arc::new(AtomicBool::new(false));

        // Sticky fast-path only for non-priority providers. Priority sticky
        // providers (Luna/Aura) join the normal priority fan-out below so
        // they race with each other and the user gets the fastest source.
        //
        // The sticky provider is fired concurrently with the priority fan-out
        // so it can still deliver an early result, but it never short-circuits
        // the rest of the providers. Returning only the sticky source hides
        // faster/better alternatives and leaves the user stuck if its streams
        // fail to play.
        let mut sticky_task: Option<tokio::task::JoinHandle<Vec<StreamResult>>> = None;
        if let Some(entry) = sticky_hit.clone() {
            let sticky_is_priority = state
                .registry
                .list_all_arc()
                .iter()
                .find(|p| p.real_name() == entry.provider_real_name)
                .map(|p| matches!(p.name(), "Luna" | "Aura"))
                .unwrap_or(false);

            if !sticky_is_priority {
                if let Some(provider) = state
                    .registry
                    .list_all_arc()
                    .into_iter()
                    .find(|p| p.real_name() == entry.provider_real_name)
                {
                    let cb = cb.clone();
                    let tmdb = tmdb_info.clone();
                    let mtype = m_type.clone();
                    let tx = tx.clone();
                    let secret = state.secret.clone();
                    let kkey = state.kkey.clone();
                    let proxy_base = proxy_base.clone();
                    let enable_proxy = state.config.enable_proxy;
                    let stream_count = stream_count.clone();
                    let config = state.config.clone();
                    let client = state.client.clone();
                    let head_timeout = state.config.head_check_timeout_secs;
                    let tmdb_id = tmdb_id.clone();
                    sticky_task = Some(tokio::spawn(async move {
                        let name = provider.name().to_string();
                        let real_name = provider.real_name().to_string();
                        let timeout_secs = provider.timeout_secs().min(3);
                        let (provider_season, provider_episode) =
                            config.map_season_episode(&real_name, &tmdb_id, season, episode);
                        let t0 = std::time::Instant::now();
                        info!(
                            "[SSE] sticky fast-path {} (timeout {}s)",
                            name, timeout_secs
                        );
                        let mut result = timeout(
                            Duration::from_secs(timeout_secs),
                            provider.fetch(&tmdb, &mtype, provider_season, provider_episode),
                        )
                        .await;

                        // Auto-detect season split: if season 1 returns empty and
                        // episode > 12, try season 2 with episode shifted by 12.
                        if matches!(result, Ok(Ok(ref s)) if s.is_empty())
                            && season == 1
                            && episode > 12
                        {
                            let fallback = timeout(
                                Duration::from_secs(3),
                                provider.fetch(&tmdb, &mtype, 2, episode - 12),
                            )
                            .await;
                            if matches!(fallback, Ok(Ok(ref s)) if !s.is_empty()) {
                                result = fallback;
                            }
                        }

                        let elapsed_ms = t0.elapsed().as_millis() as u64;
                        match result {
                            Ok(Ok(streams)) if !streams.is_empty() => {
                                let checks = join_all(
                                    streams.iter().map(|s| is_stream_reachable(&client, s, head_timeout)),
                                )
                                .await;
                                let streams: Vec<StreamResult> = streams
                                    .into_iter()
                                    .zip(checks)
                                    .filter(|(_, ok)| *ok)
                                    .map(|(s, _)| s)
                                    .collect();
                                if streams.is_empty() {
                                    info!("[SSE] sticky fast-path {} returned only unreachable streams", name);
                                    cb.record_failure(&real_name);
                                    return vec![];
                                }
                                cb.record_success(&real_name);
                                stream_count.fetch_add(streams.len(), Ordering::Relaxed);
                                info!(
                                    "[SSE] sticky fast-path {} returned {} reachable streams in {}ms",
                                    name,
                                    streams.len(),
                                    elapsed_ms
                                );
                                let mut display = if enable_proxy {
                                    wrap_with_proxy(streams.clone(), &proxy_base, &secret)
                                } else {
                                    streams.clone()
                                };
                                encrypt_stream_result_urls(&mut display, &kkey);
                                let _ = tx
                                    .send(Ok(Event::default().event("provider").data(
                                        json!({
                                            "provider": name,
                                            "streams": display,
                                            "count": display.len(),
                                            "elapsed_ms": elapsed_ms,
                                            "sticky": true
                                        })
                                        .to_string(),
                                    )))
                                    .await;
                                streams
                            }
                            Ok(Err(e)) => {
                                warn!("[SSE] Sticky provider {} failed: {}", name, e);
                                cb.record_failure(&real_name);
                                vec![]
                            }
                            Err(_) => {
                                warn!(
                                    "[SSE] Sticky provider {} timed out after {}s",
                                    name, timeout_secs
                                );
                                cb.record_failure(&real_name);
                                vec![]
                            }
                            _ => {
                                info!("[SSE] Sticky provider {} outcome=empty", name);
                                vec![]
                            }
                        }
                    }));
                } else {
                    state.sticky_pref.record_failure(&sticky_key).await;
                }
            }
        }

        // Fire each provider independently, emitting results as they arrive.
        // Luna (VidRock) and Aura (VidXsrc) run first with a short timeout
        // so the player can start quickly before fanning out to slower providers.
        let registry = state.registry.clone();
        let all_providers: Vec<_> = registry
            .list_all_arc()
            .into_iter()
            .filter(|provider| is_generic_movie_tv_provider_allowed(&m_type, provider.real_name()))
            .collect();
        let cb = registry.circuit_breaker().clone();
        let mut raw_all: Vec<StreamResult> = Vec::new();

        let priority_names = ["Luna", "Aura"];
        let (priority, rest): (Vec<_>, Vec<_>) = all_providers
            .into_iter()
            .partition(|p| priority_names.contains(&p.name()));

        info!(
            "[SSE] priority phase providers: {:?} | health: {:?}",
            priority.iter().map(|p| p.name()).collect::<Vec<_>>(),
            priority
                .iter()
                .map(|p| (p.name(), cb.is_healthy(p.real_name())))
                .collect::<Vec<_>>()
        );

        let mut provider_tasks = tokio::task::JoinSet::new();

        // 10s partial emission timer — tells the frontend "you have enough to
        // start playing, more may arrive" without waiting for stragglers.
        let tx_partial = tx.clone();
        let stream_count_partial = stream_count.clone();
        let done_sent_partial = done_sent.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(10)).await;
            let count = stream_count_partial.load(Ordering::Relaxed);
            if count > 0 && !done_sent_partial.load(Ordering::Relaxed) {
                let _ = tx_partial
                    .send(Ok(Event::default().event("partial").data(
                        json!({"total_so_far": count, "partial": true}).to_string(),
                    )))
                    .await;
            }
        });

        // Phase 1: spawn priority providers first (Luna, Aura, and any
        // globally top-scoring provider for this title).
        for provider in priority.clone() {
            let cb = cb.clone();
            let tmdb = tmdb_info.clone();
            let mtype = m_type.clone();
            let tx = tx.clone();
            let secret = state.secret.clone();
            let kkey = state.kkey.clone();
            let proxy_base = proxy_base.clone();
            let enable_proxy = state.config.enable_proxy;
            let stream_count = stream_count.clone();
            let config = state.config.clone();
            let client = state.client.clone();
            let head_timeout = state.config.head_check_timeout_secs;
            let tmdb_id = tmdb_id.clone();
            provider_tasks.spawn(async move {
                let name = provider.name().to_string();
                let real_name = provider.real_name().to_string();
                let timeout_secs = provider.timeout_secs();
                if !cb.is_healthy(&real_name) {
                    info!("[SSE] provider {} skipped — circuit open", name);
                    let _ = tx
                        .send(Ok(Event::default().event("provider_skip").data(
                            json!({"provider": name, "reason": "circuit_open"}).to_string(),
                        )))
                        .await;
                    return vec![];
                }
                let (provider_season, provider_episode) =
                    config.map_season_episode(&real_name, &tmdb_id, season, episode);
                let t0 = std::time::Instant::now();
                info!(
                    "[SSE] spawning provider {} (timeout {}s)",
                    name, timeout_secs
                );
                let mut result = timeout(
                    Duration::from_secs(timeout_secs),
                    provider.fetch(&tmdb, &mtype, provider_season, provider_episode),
                )
                .await;

                // Retry once on transient error or timeout (not on empty success)
                if matches!(result, Ok(Err(_)) | Err(_)) {
                    info!("[SSE] provider {} retrying after initial failure", name);
                    let retry = timeout(
                        Duration::from_secs(3),
                        provider.fetch(&tmdb, &mtype, provider_season, provider_episode),
                    )
                    .await;
                    result = retry;
                }

                // Auto-detect season split: if season 1 returns empty and
                // episode > 12, try season 2 with episode shifted by 12.
                if matches!(result, Ok(Ok(ref s)) if s.is_empty()) && season == 1 && episode > 12 {
                    let fallback = timeout(
                        Duration::from_secs(3),
                        provider.fetch(&tmdb, &mtype, 2, episode - 12),
                    )
                    .await;
                    if matches!(fallback, Ok(Ok(ref s)) if !s.is_empty()) {
                        result = fallback;
                    }
                }

                let elapsed_ms = t0.elapsed().as_millis() as u64;
                match result {
                    Ok(Ok(streams)) if !streams.is_empty() => {
                        let checks = join_all(
                            streams.iter().map(|s| is_stream_reachable(&client, s, head_timeout)),
                        )
                        .await;
                        let streams: Vec<StreamResult> = streams
                            .into_iter()
                            .zip(checks)
                            .filter(|(_, ok)| *ok)
                            .map(|(s, _)| s)
                            .collect();
                        if streams.is_empty() {
                            info!("[SSE] provider {} returned only unreachable streams", name);
                            cb.record_failure(&real_name);
                            return vec![];
                        }
                        cb.record_success(&real_name);
                        stream_count.fetch_add(streams.len(), Ordering::Relaxed);
                        info!(
                            "[SSE] provider {} returned {} reachable streams in {}ms",
                            name,
                            streams.len(),
                            elapsed_ms
                        );
                        let mut display = if enable_proxy {
                            wrap_with_proxy(streams.clone(), &proxy_base, &secret)
                        } else {
                            streams.clone()
                        };
                        encrypt_stream_result_urls(&mut display, &kkey);
                        let _ = tx
                            .send(Ok(Event::default().event("provider").data(
                                json!({
                                    "provider": name,
                                    "streams": display,
                                    "count": display.len(),
                                    "elapsed_ms": elapsed_ms
                                })
                                .to_string(),
                            )))
                            .await;
                        streams
                    }
                    Ok(Err(e)) => {
                        warn!("[SSE] Provider {} failed: {}", name, e);
                        cb.record_failure(&real_name);
                        info!(
                            "[SSE] provider {} outcome=failure elapsed_ms={}",
                            name, elapsed_ms
                        );
                        vec![]
                    }
                    Err(_) => {
                        warn!("[SSE] Provider {} timed out after {}s", name, timeout_secs);
                        cb.record_failure(&real_name);
                        info!(
                            "[SSE] provider {} outcome=timeout elapsed_ms={}",
                            name, elapsed_ms
                        );
                        vec![]
                    }
                    _ => {
                        info!(
                            "[SSE] provider {} outcome=empty elapsed_ms={}",
                            name, elapsed_ms
                        );
                        vec![]
                    }
                }
            });
        }

        let aggregate_deadline = state.config.slow_deadline();
        let fast_cutoff = std::time::Instant::now() + Duration::from_secs(5);
        let mut priority_streams_found = false;

        // Collect from priority phase first
        if !priority.is_empty() {
            match timeout(Duration::from_secs(5), async {
                while let Some(Ok(streams)) = provider_tasks.join_next().await {
                    if !streams.is_empty() {
                        raw_all.extend(streams);
                        priority_streams_found = true;
                    }
                }
            })
            .await
            {
                Ok(_) => {}
                Err(_) => {
                    warn!(
                        "[SSE] Priority phase deadline reached, {} streams collected",
                        raw_all.len()
                    );
                }
            }
        }

        // Previously emitted `done` here as soon as the priority phase had ANY
        // stream so the player could start sooner. The cost was that titles
        // where the priority provider's stream was dead (e.g. Luna 404s but
        // Lordflix has the file) bailed before Phase 2 providers had a chance
        // to report, so the client never saw the working stream.
        //
        // The client already picks a best stream as `provider` events arrive
        // (see useHueStreamSSE: resolves after 2 providers or a stable URL),
        // so we don't need to short-circuit `done` here — letting Phase 2 run
        // and emitting the final `done` at the natural end means the client
        // can fail over to Phase 2 streams when the priority stream dies.
        let _ = priority_streams_found;

        // Phase 2: spawn remaining providers (all for TV, rest for movies)
        for provider in rest {
            let cb = cb.clone();
            let tmdb = tmdb_info.clone();
            let mtype = m_type.clone();
            let tx = tx.clone();
            let secret = state.secret.clone();
            let kkey = state.kkey.clone();
            let proxy_base = proxy_base.clone();
            let enable_proxy = state.config.enable_proxy;
            let stream_count = stream_count.clone();
            let config = state.config.clone();
            let client = state.client.clone();
            let head_timeout = state.config.head_check_timeout_secs;
            let tmdb_id = tmdb_id.clone();
            provider_tasks.spawn(async move {
                let name = provider.name().to_string();
                let real_name = provider.real_name().to_string();
                let timeout_secs = provider.timeout_secs();
                if !cb.is_healthy(&real_name) {
                    return vec![];
                }
                let (provider_season, provider_episode) =
                    config.map_season_episode(&real_name, &tmdb_id, season, episode);
                let t0 = std::time::Instant::now();
                let mut result = timeout(
                    Duration::from_secs(timeout_secs),
                    provider.fetch(&tmdb, &mtype, provider_season, provider_episode),
                )
                .await;

                // Auto-detect season split: if season 1 returns empty and
                // episode > 12, try season 2 with episode shifted by 12.
                if matches!(result, Ok(Ok(ref s)) if s.is_empty()) && season == 1 && episode > 12 {
                    let fallback = timeout(
                        Duration::from_secs(3),
                        provider.fetch(&tmdb, &mtype, 2, episode - 12),
                    )
                    .await;
                    if matches!(fallback, Ok(Ok(ref s)) if !s.is_empty()) {
                        result = fallback;
                    }
                }

                let elapsed_ms = t0.elapsed().as_millis() as u64;
                match result {
                    Ok(Ok(streams)) if !streams.is_empty() => {
                        let checks = join_all(
                            streams.iter().map(|s| is_stream_reachable(&client, s, head_timeout)),
                        )
                        .await;
                        let streams: Vec<StreamResult> = streams
                            .into_iter()
                            .zip(checks)
                            .filter(|(_, ok)| *ok)
                            .map(|(s, _)| s)
                            .collect();
                        if streams.is_empty() {
                            info!("[SSE] provider {} returned only unreachable streams", name);
                            cb.record_failure(&real_name);
                            return vec![];
                        }
                        cb.record_success(&real_name);
                        stream_count.fetch_add(streams.len(), Ordering::Relaxed);
                        info!(
                            "[SSE] provider {} returned {} reachable streams in {}ms",
                            name,
                            streams.len(),
                            elapsed_ms
                        );
                        let mut display = if enable_proxy {
                            wrap_with_proxy(streams.clone(), &proxy_base, &secret)
                        } else {
                            streams.clone()
                        };
                        encrypt_stream_result_urls(&mut display, &kkey);
                        let _ = tx
                            .send(Ok(Event::default().event("provider").data(
                                json!({
                                    "provider": name,
                                    "streams": display,
                                    "count": display.len(),
                                    "elapsed_ms": elapsed_ms
                                })
                                .to_string(),
                            )))
                            .await;
                        streams
                    }
                    Ok(Err(e)) => {
                        warn!("[SSE] Provider {} failed: {}", name, e);
                        cb.record_failure(&real_name);
                        info!(
                            "[SSE] provider {} outcome=failure elapsed_ms={}",
                            name, elapsed_ms
                        );
                        vec![]
                    }
                    Err(_) => {
                        warn!("[SSE] Provider {} timed out after {}s", name, timeout_secs);
                        cb.record_failure(&real_name);
                        info!(
                            "[SSE] provider {} outcome=timeout elapsed_ms={}",
                            name, elapsed_ms
                        );
                        vec![]
                    }
                    _ => {
                        info!(
                            "[SSE] provider {} outcome=empty elapsed_ms={}",
                            name, elapsed_ms
                        );
                        vec![]
                    }
                }
            });
        }

        match timeout(aggregate_deadline, async {
            while let Some(Ok(streams)) = provider_tasks.join_next().await {
                let had_any = !raw_all.is_empty();
                raw_all.extend(streams);

                // If this is the first successful provider and we're still
                // within the fast window, emit "done" early so the frontend
                // Same rationale as the priority-phase early-done above —
                // when the first stream from any Phase 2 provider arrives, we
                // used to fire `done` to let the player start. That cost is
                // the same: when that first stream fails, the client thinks
                // SSE is finished and can't fail over to later providers. Keep
                // the connection open so subsequent `provider` events still
                // count toward the failover queue.
                let _ = had_any;
                let _ = fast_cutoff;
            }
        })
        .await
        {
            Ok(_) => {}
            Err(_) => {
                warn!(
                    "[SSE] Aggregate deadline ({:?}) reached, {} streams collected",
                    aggregate_deadline,
                    raw_all.len()
                );
            }
        }

        // Merge sticky fast-path results (if any) so they participate in the
        // final dedup/cache/sticky decision rather than being a dead-end early
        // event.
        if let Some(task) = sticky_task {
            match timeout(Duration::from_secs(1), task).await {
                Ok(Ok(streams)) if !streams.is_empty() => {
                    raw_all.extend(streams);
                }
                Ok(Err(e)) => {
                    warn!("[SSE] sticky task join failed: {}", e);
                }
                Err(_) => {
                    warn!("[SSE] sticky task still running after aggregate deadline");
                }
                _ => {}
            }
        }

        // If aggregate returned nothing, serve stale fallback so the user
        // never sees "no streams" from a temporary outage.
        if raw_all.is_empty() {
            if let Some(stale) = state.cache.get_stale(&cache_key).await {
                let filtered = apply_filters(stale, &state.filter_config)
                    .into_iter()
                    .filter(|s| {
                        let provider = s.provider.to_lowercase();
                        provider != "zenith" && provider != "orient"
                    })
                    .collect::<Vec<_>>();
                let mut streams = if state.config.enable_proxy {
                    wrap_with_proxy(filtered, &proxy_base, &state.secret)
                } else {
                    filtered
                };
                encrypt_stream_result_urls(&mut streams, &state.kkey);
                let count = streams.len();
                let _ = tx
                    .send(Ok(Event::default().event("stale").data(
                        json!({"streams": streams, "count": count, "stale": true}).to_string(),
                    )))
                    .await;
                if !done_sent.load(Ordering::Relaxed) {
                    done_sent.store(true, Ordering::Relaxed);
                    let _ = tx
                        .send(Ok(Event::default().event("done").data(
                            json!({"total": count, "cached": false, "stale": true}).to_string(),
                        )))
                        .await;
                }
                return;
            }
        }

        // Deduplicate and cache raw results
        let mut seen = std::collections::HashSet::new();
        raw_all.retain(|s| seen.insert(s.url.clone()));
        let cache_key2 = CacheKey::new(&m_type, &tmdb_id, season, episode);
        state.cache.set(&cache_key2, raw_all.clone()).await;

        // Record the sticky provider preference for the next watch.
        // Use the first result stream's provider — it's the fastest one
        // that returned successfully.
        if let Some(best) = crate::providers::best_provider_real_name(&raw_all) {
            state.sticky_pref.record_success(&sticky_key, best).await;
        }

        // Record global provider popularity so future users benefit from
        // the collective success of this title.
        if let Some(best) = crate::providers::best_provider_real_name(&raw_all) {
            state
                .provider_score
                .record_success(&m_type, &tmdb_id, best)
                .await;
        }

        if !done_sent.load(Ordering::Relaxed) {
            let _ = tx
                .send(Ok(Event::default().event("done").data(
                    json!({"total": raw_all.len(), "cached": false}).to_string(),
                )))
                .await;
        }
    });

    Sse::new(ReceiverStream::new(rx)).keep_alive(KeepAlive::default())
}

/// Fire-and-forget cache warmer. Called during browse / search so that by the
/// time the user clicks Play the cache is already hot. Returns 202 immediately
/// and spawns the same aggregation logic in the background.
pub async fn warm_handler(
    State(state): State<StreamState>,
    Path((m_type, tmdb_id)): Path<(String, String)>,
    Query(params): Query<HashMap<String, String>>,
) -> StatusCode {
    let season = params
        .get("season")
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(1);
    let episode = params
        .get("episode")
        .and_then(|e| e.parse::<u32>().ok())
        .unwrap_or(1);
    let cache_key = CacheKey::new(&m_type, &tmdb_id, season, episode);

    // Skip if cache is already fresh
    if state.cache.get(&cache_key).await.is_some() {
        return StatusCode::OK;
    }

    tokio::spawn(async move {
        let tmdb_info = match state
            .tmdb
            .get_details(&m_type, tmdb_id.parse().unwrap_or(0))
            .await
        {
            Ok(info) => info,
            Err(_) => return,
        };

        let all_providers: Vec<_> = state
            .registry
            .list_all_arc()
            .into_iter()
            .filter(|provider| is_generic_movie_tv_provider_allowed(&m_type, provider.real_name()))
            .collect();

        let cb = state.registry.circuit_breaker().clone();
        let mut raw_all: Vec<StreamResult> = Vec::new();
        let client = state.client.clone();
        let head_timeout = state.config.head_check_timeout_secs;

        let mut tasks = tokio::task::JoinSet::new();
        for provider in all_providers {
            let cb = cb.clone();
            let tmdb = tmdb_info.clone();
            let mtype = m_type.clone();
            let config = state.config.clone();
            let client = client.clone();
            let tmdb_id = tmdb_id.clone();
            tasks.spawn(async move {
                let real_name = provider.real_name().to_string();
                let timeout_secs = provider.timeout_secs();
                if !cb.is_healthy(&real_name) {
                    return vec![];
                }
                let (provider_season, provider_episode) =
                    config.map_season_episode(&real_name, &tmdb_id, season, episode);
                let mut result = timeout(
                    Duration::from_secs(timeout_secs),
                    provider.fetch(&tmdb, &mtype, provider_season, provider_episode),
                )
                .await;

                // Auto-detect season split: if season 1 returns empty and
                // episode > 12, try season 2 with episode shifted by 12.
                if matches!(result, Ok(Ok(ref s)) if s.is_empty()) && season == 1 && episode > 12 {
                    let fallback = timeout(
                        Duration::from_secs(3),
                        provider.fetch(&tmdb, &mtype, 2, episode - 12),
                    )
                    .await;
                    if matches!(fallback, Ok(Ok(ref s)) if !s.is_empty()) {
                        result = fallback;
                    }
                }

                match result {
                    Ok(Ok(streams)) if !streams.is_empty() => {
                        let checks = join_all(
                            streams.iter().map(|s| is_stream_reachable(&client, s, head_timeout)),
                        )
                        .await;
                        let streams: Vec<StreamResult> = streams
                            .into_iter()
                            .zip(checks)
                            .filter(|(_, ok)| *ok)
                            .map(|(s, _)| s)
                            .collect();
                        if streams.is_empty() {
                            info!("[Warm] provider {} returned only unreachable streams", real_name);
                            cb.record_failure(&real_name);
                        } else {
                            cb.record_success(&real_name);
                        }
                        streams
                    }
                    Ok(Err(_)) | Err(_) => {
                        cb.record_failure(&real_name);
                        vec![]
                    }
                    _ => vec![],
                }
            });
        }

        let deadline = state.config.slow_deadline();
        match timeout(deadline, async {
            while let Some(Ok(streams)) = tasks.join_next().await {
                raw_all.extend(streams);
            }
        })
        .await
        {
            Ok(_) => {}
            Err(_) => warn!(
                "[Warm] Deadline reached for {} {} s{}e{}",
                m_type, tmdb_id, season, episode
            ),
        }

        if !raw_all.is_empty() {
            let mut seen = std::collections::HashSet::new();
            raw_all.retain(|s| seen.insert(s.url.clone()));
            state.cache.set(&cache_key, raw_all.clone()).await;
            if let Some(best) = crate::providers::best_provider_real_name(&raw_all) {
                state
                    .provider_score
                    .record_success(&m_type, &tmdb_id, best)
                    .await;
            }
        }
    });

    StatusCode::ACCEPTED
}
