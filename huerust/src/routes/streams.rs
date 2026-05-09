//! Hue Stream API
//! Professional Streaming Aggregator
//!
//! @author roywalk3r <roywalker440@gmail.com>
//! @repository https://github.com/roywalk3r/hue.git
//! @license MIT

use axum::{
    extract::{Query, State, Path},
    response::{IntoResponse, Response},
    http::StatusCode,
};
use serde::Serialize;
use utoipa::ToSchema;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;
use tracing::{info, warn};

use crate::{
    AppState,
    cache::CacheKey,
    filters::apply_filters,
    registry::ProviderRegistry,
    models::StreamResult,
    security::encrypt_url,
    metrics::Metrics,
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

#[derive(Serialize, Clone, ToSchema)]
pub struct ProviderTiming {
    pub provider: String,
    pub elapsed_ms: u64,
    pub streams_found: usize,
    pub cancelled: bool,
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
    State(state): State<AppState>,
    Path((m_type, tmdb_id)): Path<(String, String)>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    let season = params.get("season").and_then(|s| s.parse::<u32>().ok()).unwrap_or(1);
    let episode = params.get("episode").and_then(|e| e.parse::<u32>().ok()).unwrap_or(1);
    let no_cache = params.get("nocache").map(|v| v == "1").unwrap_or(false);

    info!("[api] Rust Streams: {} {} S{} E{}", &m_type, tmdb_id, season, episode);

    let cache_key = CacheKey::new(&m_type, &tmdb_id, season, episode);
    let enable_proxy = state.config.enable_proxy;
    let proxy_base = state.proxy_base_url.clone();
    let secret = state.secret.clone();

    // 1. Check cache
    if !no_cache {
        if let Some(cached) = state.cache.get(&cache_key).await {
            let filtered = apply_filters(cached, &state.filter_config);
            
            // Proxy wrap even on cache hits
            let final_streams = if enable_proxy {
                wrap_with_proxy(filtered, &proxy_base, &secret)
            } else {
                filtered
            };

            return (StatusCode::OK, serde_json::to_string(&StreamsResponse {
                success: true,
                tmdb_id: tmdb_id.clone(),
                media_type: m_type.clone(),
                season,
                episode,
                count: final_streams.len(),
                streams: final_streams,
                provider_timings: vec![],
            }).unwrap()).into_response();
        }
    }

    // 2. Fetch TMDB details
    let tmdb_info = match state.tmdb.get_details(&m_type, tmdb_id.parse().unwrap_or(0)).await {
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
    let slow_deadline = state.config.slow_deadline();
    let metrics = state.metrics.clone();

    let coalesced = if no_cache {
        // Bypass coalescer — start all providers simultaneously, per-provider timeouts apply.
        let start = std::time::Instant::now();
        let (streams, _) = crate::routes::streams::fetch_from_providers(
            registry.clone(),
            metrics.clone(),
            &tmdb_info_c,
            &m_type_c,
            season,
            episode,
            false, // all providers in one parallel batch
            999,
            slow_deadline,
        ).await;
        let mut seen_urls = std::collections::HashSet::new();
        let unique: Vec<_> = streams.into_iter().filter(|s| seen_urls.insert(s.url.clone())).collect();
        let filtered = apply_filters(unique, &filter_config);
        info!("[api] No-cache fresh fetch: {} streams in {:?}", filtered.len(), start.elapsed());
        Some(filtered)
    } else {
        state.coalescer.execute(&cache_key, &cache, || async move {
            let start = std::time::Instant::now();
            // All providers start simultaneously; each respects its own per-provider timeout.
            let (streams, _) = fetch_from_providers(
                registry.clone(),
                metrics.clone(),
                &tmdb_info_c,
                &m_type_c,
                season,
                episode,
                false, // all providers in one parallel batch
                999,
                slow_deadline,
            ).await;
            let mut seen_urls = std::collections::HashSet::new();
            let unique: Vec<_> = streams.into_iter().filter(|s| seen_urls.insert(s.url.clone())).collect();
            let filtered = apply_filters(unique, &filter_config);
            info!("[api] Coalesced fetch: {} streams in {:?}", filtered.len(), start.elapsed());
            filtered
        }).await
    };

    let filtered = match coalesced {
        Some(v) => v,
        None => return (StatusCode::NOT_FOUND, "No streams found").into_response(),
    };

    let final_streams = if enable_proxy {
        wrap_with_proxy(filtered, &proxy_base, &secret)
    } else {
        filtered
    };

    (StatusCode::OK, serde_json::to_string(&StreamsResponse {
        success: true,
        tmdb_id,
        media_type: m_type,
        season,
        episode,
        count: final_streams.len(),
        streams: final_streams,
        provider_timings: vec![],
    }).unwrap()).into_response()
}

async fn fetch_from_providers(
    registry: Arc<ProviderRegistry>,
    metrics: Arc<Metrics>,
    tmdb_info: &crate::tmdb::TmdbNormalizedInfo,
    media_type: &str,
    season: u32,
    episode: u32,
    fast_only: bool,
    min_streams_threshold: usize,
    aggregate_deadline: Duration,
) -> (Vec<StreamResult>, Vec<ProviderTiming>) {
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use tokio::time::Instant;

    let providers: Vec<_> = if fast_only {
        registry.list_fast().into_iter().map(|p| (p.name().to_string(), p.timeout_secs(), p.is_fast())).collect()
    } else {
        registry.list_all().into_iter().map(|p| (p.name().to_string(), p.timeout_secs(), p.is_fast())).collect()
    };

    let stream_count = Arc::new(AtomicUsize::new(0));
    let cancelled = Arc::new(AtomicBool::new(false));
    let mut handles = Vec::new();
    let start = Instant::now();

    for (name, timeout_secs, is_fast) in providers {
        let registry = registry.clone();
        let provider_name = name.clone();
        let tmdb = tmdb_info.clone();
        let mtype = media_type.to_string();
        let count = stream_count.clone();
        let cancel = cancelled.clone();

        let metrics_c = metrics.clone();
        let handle = tokio::spawn(async move {
            if cancel.load(Ordering::Relaxed) {
                return (provider_name, vec![], Duration::ZERO, true);
            }

            let provider_start = Instant::now();
            let provider = match registry.get(&provider_name) {
                Some(p) => p,
                None => {
                    warn!("[api] Provider {} not found in registry", provider_name);
                    metrics_c.record_request(&provider_name, 0, false, false);
                    return (provider_name, vec![], Duration::ZERO, false);
                }
            };

            let cb = registry.circuit_breaker().clone();
            let real_name = provider.real_name().to_string();

            if !cb.is_healthy(&real_name) {
                warn!("[api] Provider {} circuit open, skipping", provider_name);
                metrics_c.record_request(&provider_name, 0, false, false);
                return (provider_name, vec![], Duration::ZERO, false);
            }

            let provider_timeout = Duration::from_secs(timeout_secs);

            let result = match timeout(provider_timeout, provider.fetch(&tmdb, &mtype, season, episode)).await {
                Ok(Ok(streams)) => {
                    cb.record_success(&real_name);
                    let latency = provider_start.elapsed().as_millis() as u64;
                    metrics_c.record_request(&provider_name, latency, true, false);
                    let new_count = count.fetch_add(streams.len(), Ordering::Relaxed) + streams.len();
                    if new_count >= min_streams_threshold {
                        cancel.store(true, Ordering::Relaxed);
                    }
                    streams
                }
                Ok(Err(e)) => {
                    warn!("[api] Provider {} failed: {}", provider_name, e);
                    cb.record_failure(&real_name);
                    let latency = provider_start.elapsed().as_millis() as u64;
                    metrics_c.record_request(&provider_name, latency, false, false);
                    vec![]
                }
                Err(_) => {
                    warn!("[api] Provider {} timed out after {}s", provider_name, timeout_secs);
                    cb.record_failure(&real_name);
                    let latency = provider_start.elapsed().as_millis() as u64;
                    metrics_c.record_request(&provider_name, latency, false, true);
                    vec![]
                }
            };

            let elapsed = provider_start.elapsed();
            let was_cancelled = cancel.load(Ordering::Relaxed) && result.is_empty() && !is_fast;
            (provider_name, result, elapsed, was_cancelled)
        });
        handles.push(handle);
    }

    let mut all = Vec::new();
    let mut timings = Vec::new();

    match timeout(aggregate_deadline, async {
        for h in handles {
            if let Ok((name, streams, elapsed, was_cancelled)) = h.await {
                timings.push(ProviderTiming {
                    provider: name,
                    elapsed_ms: elapsed.as_millis() as u64,
                    streams_found: streams.len(),
                    cancelled: was_cancelled,
                });
                all.extend(streams);
            }
        }
    }).await {
        Ok(_) => {},
        Err(_) => {
            warn!("[api] Aggregate deadline ({:?}) reached, returning {} streams", aggregate_deadline, all.len());
        }
    }

    let total_elapsed = start.elapsed();
    info!(
        "[api] Fetched {} streams from {} providers in {:?}",
        all.len(),
        timings.len(),
        total_elapsed
    );

    (all, timings)
}

pub(crate) fn wrap_with_proxy(streams: Vec<StreamResult>, proxy_base: &str, secret: &str) -> Vec<StreamResult> {
    streams.into_iter().map(|mut s| {
        let is_hls = s.format == "hls" || s.url.contains(".m3u8") || s.provider == "Aura";

        // Cinema (MovieBox) MP4 streams come from a CDN that IP-blocks server/datacenter
        // ranges. Proxying them through our server always 403s. Return the raw URL with
        // headers so the client (browser/player) fetches directly from its own IP.
        if s.provider == "Cinema" && !is_hls {
            return s;
        }

        let use_url_mode = s.extra_metadata.as_ref()
            .map(|m| m.get("proxyMode").map(|v| v == "url").unwrap_or(false))
            .unwrap_or(false);

        if use_url_mode {
            let route = if is_hls { "m3u8-proxy" } else { "ts-proxy" };
            let mut proxy_url = format!("{}/{}?url={}", proxy_base, route, urlencoding::encode(&s.url));

            if let Some(ref h) = s.headers {
                let skip: &[&str] = &["host", "connection", "content-length", "accept-encoding"];
                let filtered: HashMap<String, String> = h
                    .iter()
                    .filter(|(k, _)| !skip.contains(&k.to_lowercase().as_str()))
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();
                if !filtered.is_empty() {
                    if let Ok(h_json) = serde_json::to_string(&filtered) {
                        proxy_url.push_str(&format!("&headers={}", urlencoding::encode(&h_json)));
                    }
                }
            }
            s.url = proxy_url;
        } else {
            let token = encrypt_url(&s.url, secret, s.headers.clone());
            s.url = if is_hls {
                format!("{}/m3u8-proxy?h_token={}", proxy_base, token)
            } else {
                format!("{}/ts-proxy?h_token={}", proxy_base, token)
            };
        }

        s.headers = None; // headers are now baked into the token or url query

        if let Some(subs) = s.subtitles.take() {
            s.subtitles = Some(subs.into_iter().map(|mut sub| {
                let token = encrypt_url(&sub.url, secret, None);
                sub.url = format!("{}/sub-proxy?h_token={}", proxy_base, token);
                sub
            }).collect());
        }
        s
    }).collect()
}
