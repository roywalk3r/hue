//! Hue Stream API
//! Professional Streaming Aggregator
//!
//! @author roywalk3r <roywalker440@gmail.com>
//! @repository https://github.com/roywalk3r/hue.git
//! @license MIT

use std::convert::Infallible;
use std::time::Duration;
use axum::{
    extract::{Path, Query, State},
    response::sse::{Event, KeepAlive, Sse},
    http::HeaderMap,
};
use serde_json::json;
use std::collections::HashMap;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tokio::time::timeout;
use tracing::warn;

use crate::{
    app::StreamState,
    infra::{
        cache::CacheKey,
        sticky_pref::{StickyPrefEntry, StickyPrefKey},
    },
    filters::apply_filters,
    proxy::wrap::wrap_with_proxy,
    proxy::dynamic_proxy_base,
    security::encrypt_stream_result_urls,
};

fn is_generic_movie_tv_provider_allowed(media_type: &str, real_name: &str) -> bool {
    if media_type != "movie" && media_type != "tv" {
        return true;
    }

    real_name != "kisskh" && real_name != "dramacool"
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
    let season = params.get("season").and_then(|s| s.parse::<u32>().ok()).unwrap_or(1);
    let episode = params.get("episode").and_then(|e| e.parse::<u32>().ok()).unwrap_or(1);
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
            if k == "hue_anon" { Some(v.to_string()) } else { None }
        })
        .unwrap_or_else(|| "anon".to_string());

    tokio::spawn(async move {
        let cache_key = CacheKey::new(&m_type, &tmdb_id, season, episode);
        let sticky_key = StickyPrefKey::new(&user_id, &m_type, &tmdb_id);

        // Serve from cache immediately if available (only if nocache is not set)
        if !no_cache {
            if let Some(cached) = state.cache.get(&cache_key).await {
                let filtered = apply_filters(cached, &state.filter_config)
                    .into_iter()
                    .filter(|s| {
                        let provider = s.provider.to_lowercase();
                        provider != "zenith" && provider != "orient"
                    })
                    .collect();
                let mut streams = if state.config.enable_proxy {
                    wrap_with_proxy(filtered, &proxy_base, &state.secret)
                } else {
                    filtered
                };
                encrypt_stream_result_urls(&mut streams, &state.kkey);
                let count = streams.len();
                let _ = tx.send(Ok(Event::default().event("cached").data(
                    json!({"streams": streams, "count": count, "cached": true}).to_string()
                ))).await;
                let _ = tx.send(Ok(Event::default().event("done").data(
                    json!({"total": count, "cached": true}).to_string()
                ))).await;
                return;
            }
        }

        // Sticky fast-path: if this user has a recent successful provider
        // for this title, try just that one with a short timeout before
        // fanning out to every provider.
        let cb = state.registry.circuit_breaker();
        let mut sticky_hit: Option<StickyPrefEntry> = None;
        if let Some(entry) = state.sticky_pref.get(&sticky_key).await {
            if entry.consecutive_failures == 0 && cb.is_healthy(&entry.provider_real_name) {
                sticky_hit = Some(entry);
            } else {
                state.sticky_pref.record_failure(&sticky_key).await;
            }
        }

        // Fetch TMDB info (needed for both fast path and full fan-out)
        let tmdb_info = match state.tmdb.get_details(&m_type, tmdb_id.parse().unwrap_or(0)).await {
            Ok(info) => info,
            Err(e) => {
                let _ = tx.send(Ok(Event::default().event("error").data(e.to_string()))).await;
                return;
            }
        };

        // Try the sticky provider alone first
        if let Some(entry) = sticky_hit {
            if let Some(provider) = state
                .registry
                .list_all_arc()
                .into_iter()
                .find(|p| p.real_name() == entry.provider_real_name)
            {
                let t0 = std::time::Instant::now();
                let result = timeout(
                    Duration::from_secs(3),
                    provider.fetch(&tmdb_info, &m_type, season, episode),
                ).await;

                match result {
                    Ok(Ok(streams)) if !streams.is_empty() => {
                        cb.record_success(&entry.provider_real_name);
                        let elapsed_ms = t0.elapsed().as_millis() as u64;
                        let mut display = if state.config.enable_proxy {
                            wrap_with_proxy(streams.clone(), &proxy_base, &state.secret)
                        } else {
                            streams.clone()
                        };
                        encrypt_stream_result_urls(&mut display, &state.kkey);
                        let _ = tx.send(Ok(Event::default().event("provider").data(
                            json!({
                                "provider": provider.name(),
                                "streams": display,
                                "count": display.len(),
                                "elapsed_ms": elapsed_ms,
                                "sticky": true
                            }).to_string()
                        ))).await;
                        state.sticky_pref.record_success(&sticky_key, &entry.provider_real_name).await;

                        // Also populate the regular stream cache so future
                        // requests can short-circuit entirely.
                        let mut seen = std::collections::HashSet::new();
                        let deduped: Vec<_> = streams.into_iter().filter(|s| seen.insert(s.url.clone())).collect();
                        state.cache.set(&cache_key, deduped).await;

                        let _ = tx.send(Ok(Event::default().event("done").data(
                            json!({"total": 1, "cached": false, "sticky": true}).to_string()
                        ))).await;
                        return;
                    }
                    _ => {
                        warn!(
                            "[SSE] Sticky provider {} failed fast-path for {} — falling back to fan-out",
                            entry.provider_real_name, tmdb_id
                        );
                        state.sticky_pref.record_failure(&sticky_key).await;
                    }
                }
            } else {
                state.sticky_pref.record_failure(&sticky_key).await;
            }
        }

        // Fire each provider independently, emitting results as they arrive
        let registry = state.registry.clone();
        let providers: Vec<_> = registry
            .list_all_arc()
            .into_iter()
            .filter(|provider| is_generic_movie_tv_provider_allowed(&m_type, provider.real_name()))
            .collect();
        let cb = registry.circuit_breaker().clone();
        let mut raw_all: Vec<crate::models::StreamResult> = Vec::new();

        let mut provider_tasks = tokio::task::JoinSet::new();
        for provider in providers {
            let cb = cb.clone();
            let tmdb = tmdb_info.clone();
            let mtype = m_type.clone();
            let tx = tx.clone();
            let secret = state.secret.clone();
            let kkey = state.kkey.clone();
            let proxy_base = proxy_base.clone();
            let enable_proxy = state.config.enable_proxy;

            provider_tasks.spawn(async move {
                let name = provider.name().to_string();
                let real_name = provider.real_name().to_string();
                let timeout_secs = provider.timeout_secs();
                if !cb.is_healthy(&real_name) {
                    return vec![];
                }

                let t0 = std::time::Instant::now();
                let result = timeout(
                    Duration::from_secs(timeout_secs),
                    provider.fetch(&tmdb, &mtype, season, episode),
                ).await;

                let elapsed_ms = t0.elapsed().as_millis() as u64;

                match result {
                    Ok(Ok(streams)) if !streams.is_empty() => {
                        cb.record_success(&real_name);
                        let mut display = if enable_proxy {
                            wrap_with_proxy(streams.clone(), &proxy_base, &secret)
                        } else {
                            streams.clone()
                        };
                        encrypt_stream_result_urls(&mut display, &kkey);
                        let _ = tx.send(Ok(Event::default().event("provider").data(
                            json!({
                                "provider": name,
                                "streams": display,
                                "count": display.len(),
                                "elapsed_ms": elapsed_ms
                            }).to_string()
                        ))).await;
                        streams
                    }
                    Ok(Err(e)) => {
                        warn!("[SSE] Provider {} failed: {}", name, e);
                        cb.record_failure(&real_name);
                        vec![]
                    }
                    Err(_) => {
                        warn!("[SSE] Provider {} timed out after {}s", name, timeout_secs);
                        cb.record_failure(&real_name);
                        vec![]
                    }
                    _ => vec![],
                }
            });
        }

        let aggregate_deadline = state.config.slow_deadline();
        match timeout(aggregate_deadline, async {
            while let Some(Ok(streams)) = provider_tasks.join_next().await {
                raw_all.extend(streams);
            }
        }).await {
            Ok(_) => {},
            Err(_) => {
                warn!("[SSE] Aggregate deadline ({:?}) reached, {} streams collected", aggregate_deadline, raw_all.len());
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

        let _ = tx.send(Ok(Event::default().event("done").data(
            json!({"total": raw_all.len(), "cached": false}).to_string()
        ))).await;
    });

    Sse::new(ReceiverStream::new(rx)).keep_alive(KeepAlive::default())
}
