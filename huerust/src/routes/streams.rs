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
use tracing::{info, warn};

use crate::{
    app::StreamState,
    infra::cache::CacheKey,
    filters::apply_filters,
    models::StreamResult,
};
use crate::providers::dispatch::{fetch_from_providers, ProviderTiming};
use crate::proxy::wrap::wrap_with_proxy;

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

    let mut captured_timings: Vec<ProviderTiming> = vec![];

    let coalesced = if no_cache {
        // Bypass coalescer — start all providers simultaneously, per-provider timeouts apply.
        let start = std::time::Instant::now();
        let (streams, timings) = fetch_from_providers(
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
        captured_timings = timings;
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
        provider_timings: captured_timings,
    }).unwrap()).into_response()
}


