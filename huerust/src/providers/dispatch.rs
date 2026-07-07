use serde::Serialize;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;
use tracing::{info, warn};
use utoipa::ToSchema;

use crate::clients::tmdb::TmdbNormalizedInfo;
use crate::infra::metrics::Metrics;
use crate::models::StreamResult;
use crate::providers::registry::ProviderRegistry;

/// Build a fresh registry containing exactly the named provider, sharing
/// the circuit breaker from the main registry. Used by single-provider
/// routes (e.g. /api/streams/luna/...) that should not fan out to siblings.
pub fn single_provider_registry(
    main: &Arc<ProviderRegistry>,
    real_name: &str,
) -> Option<Arc<ProviderRegistry>> {
    let provider = main.list_all_arc().into_iter().find(|p| {
        p.real_name().eq_ignore_ascii_case(real_name) || p.name().eq_ignore_ascii_case(real_name)
    })?;
    let cb = main.circuit_breaker().clone();
    let mut r = ProviderRegistry::new(cb);
    r.register(provider);
    Some(Arc::new(r))
}

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
    fetch_from_providers_matching(
        registry,
        metrics,
        tmdb_info,
        media_type,
        season,
        episode,
        fast_only,
        min_streams_threshold,
        aggregate_deadline,
        None,
        |_| true,
    )
    .await
}

pub async fn fetch_from_providers_matching(
    registry: Arc<ProviderRegistry>,
    metrics: Arc<Metrics>,
    tmdb_info: &TmdbNormalizedInfo,
    media_type: &str,
    season: u32,
    episode: u32,
    fast_only: bool,
    min_streams_threshold: usize,
    aggregate_deadline: Duration,
    min_distinct_providers: Option<usize>,
    threshold_matcher: fn(&StreamResult) -> bool,
) -> (Vec<StreamResult>, Vec<ProviderTiming>) {
    use tokio::time::Instant;

    let providers: Vec<_> = if fast_only {
        registry
            .list_all_arc()
            .into_iter()
            .filter(|p| p.is_fast() && registry.circuit_breaker().is_healthy(p.real_name()))
            .collect()
    } else {
        registry
            .list_all_arc()
            .into_iter()
            .filter(|p| registry.circuit_breaker().is_healthy(p.real_name()))
            .collect()
    };

    let mut tasks = tokio::task::JoinSet::new();
    let start = Instant::now();

    for provider in providers {
        let provider_name = provider.name().to_string();
        let timeout_secs = provider.timeout_secs();
        let tmdb = tmdb_info.clone();
        let mtype = media_type.to_string();
        let metrics_c = metrics.clone();
        let cb = registry.circuit_breaker().clone();

        tasks.spawn(async move {
            let provider_start = Instant::now();
            let real_name = provider.real_name().to_string();

            if !cb.is_healthy(&real_name) {
                warn!("[api] Provider {} circuit open, skipping", provider_name);
                metrics_c.record_request(&provider_name, 0, false, false);
                return (provider_name, vec![], Duration::ZERO, false);
            }

            let provider_timeout = Duration::from_secs(timeout_secs);

            let result = match timeout(
                provider_timeout,
                provider.fetch(&tmdb, &mtype, season, episode),
            )
            .await
            {
                Ok(Ok(streams)) => {
                    cb.record_success(&real_name);
                    let latency = provider_start.elapsed().as_millis() as u64;
                    metrics_c.record_request(&provider_name, latency, true, false);
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
                    warn!(
                        "[api] Provider {} timed out after {}s",
                        provider_name, timeout_secs
                    );
                    cb.record_failure(&real_name);
                    let latency = provider_start.elapsed().as_millis() as u64;
                    metrics_c.record_request(&provider_name, latency, false, true);
                    vec![]
                }
            };

            let elapsed = provider_start.elapsed();
            (provider_name, result, elapsed, false)
        });
    }

    let mut all = Vec::new();
    let mut timings = Vec::new();

    // For the fast phase (headstart) we want to give every fast provider
    // the full `aggregate_deadline` to complete. Aborting on threshold
    // was killing in-flight Vixsrc requests because Luna landed first.
    // For the slow phase we keep the abort-on-threshold behavior so we
    // return early once we have enough streams to satisfy the user.
    let abort_on_threshold = !fast_only;

    match timeout(aggregate_deadline, async {
        while let Some(result) = tasks.join_next().await {
            if let Ok((name, streams, elapsed, was_cancelled)) = result {
                let stream_len = streams.len();
                timings.push(ProviderTiming {
                    provider: name,
                    elapsed_ms: elapsed.as_millis() as u64,
                    streams_found: stream_len,
                    cancelled: was_cancelled,
                });
                all.extend(streams);
                if abort_on_threshold {
                    let matched = all
                        .iter()
                        .filter(|stream| threshold_matcher(stream))
                        .count();
                    let matched_providers = all
                        .iter()
                        .filter(|stream| threshold_matcher(stream))
                        .map(|stream| stream.provider.as_str())
                        .collect::<std::collections::HashSet<_>>()
                        .len();
                    let stream_ok = matched >= min_streams_threshold;
                    let provider_ok = min_distinct_providers
                        .map(|n| matched_providers >= n)
                        .unwrap_or(true);
                    if stream_ok && provider_ok {
                        tasks.abort_all();
                        break;
                    }
                }
            }
        }
    })
    .await
    {
        Ok(_) => {}
        Err(_) => {
            warn!(
                "[api] Aggregate deadline ({:?}) reached, returning {} streams",
                aggregate_deadline,
                all.len()
            );
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
