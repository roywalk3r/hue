use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;
use tracing::{info, warn};
use serde::Serialize;
use utoipa::ToSchema;

use crate::clients::tmdb::TmdbNormalizedInfo;
use crate::models::StreamResult;
use crate::infra::metrics::Metrics;
use crate::providers::registry::ProviderRegistry;

#[derive(Serialize, Clone, ToSchema)]
pub struct ProviderTiming {
    pub provider: String,
    pub elapsed_ms: u64,
    pub streams_found: usize,
    pub cancelled: bool,
}

pub async fn fetch_from_providers(
    registry: Arc<ProviderRegistry>,
    metrics: Arc<Metrics>,
    tmdb_info: &TmdbNormalizedInfo,
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
