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
use std::collections::HashMap;
use std::time::Duration;
use tokio::time::timeout;
use tracing::{info, warn};
use utoipa::ToSchema;
use crate::{
    AppState,
    cache::CacheKey,
    filters::{apply_filters, sort_by_quality},
};

#[derive(serde::Serialize, ToSchema)]
pub struct PlayResponse {
    pub success: bool,
    pub url: String,
    pub title: String,
    pub quality: String,
    pub provider: String,
    pub format: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subtitles: Option<Vec<crate::models::Subtitle>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<HashMap<String, String>>,
}

#[utoipa::path(
    get,
    path = "/api/play/{type}/{tmdbId}",
    params(
        ("type" = String, Path, description = "Content type: movie or tv"),
        ("tmdbId" = String, Path, description = "TMDB ID"),
        ("season" = Option<u32>, Query, description = "Season number (tv only)"),
        ("episode" = Option<u32>, Query, description = "Episode number (tv only)"),
        ("redirect" = Option<String>, Query, description = "Set to 1 for 302 redirect"),
    ),
    responses(
        (status = 200, description = "Best working stream", body = PlayResponse),
        (status = 302, description = "Redirect to stream URL"),
        (status = 404, description = "No streams found"),
    ),
)]
pub async fn play_handler(
    State(state): State<AppState>,
    Path((m_type, tmdb_id)): Path<(String, String)>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    let season = params.get("season").and_then(|s| s.parse::<u32>().ok()).unwrap_or(1);
    let episode = params.get("episode").and_then(|e| e.parse::<u32>().ok()).unwrap_or(1);

    info!("[play] Smart Play: {} {} S{} E{}", m_type, tmdb_id, season, episode);

    // 1. Check if tmdb_id is a HiTV internal ID (15+ digits)
    let mut resolved_tmdb_id = tmdb_id.clone();
    if tmdb_id.len() >= 15 && tmdb_id.chars().all(|c| c.is_ascii_digit()) {
        info!("[play] Detected HiTV internal ID: {}", tmdb_id);
        if let Ok(Some(detail)) = state.hitv.get_detail(&tmdb_id).await {
            let clean_title = sanitize_title(&detail.title);
            info!("[play] Resolved HiTV title: {}", clean_title);
            if let Ok(tmdb_results) = state.tmdb.search(&clean_title, &m_type, None).await {
                if let Some(first) = tmdb_results.first() {
                    resolved_tmdb_id = first.id.to_string();
                    info!("[play] Mapped to TMDB ID: {}", resolved_tmdb_id);
                }
            }
        }
    }

    // 2. Check cache
    let cache_key = CacheKey::new(&m_type, &resolved_tmdb_id, season, episode);
    if let Some(cached) = state.cache.get(&cache_key).await {
        if let Some(best) = pick_best_stream(cached, &state).await {
            return (StatusCode::OK, serde_json::to_string(&best).unwrap()).into_response();
        }
    }

    // 3. Fetch TMDB details
    let tmdb_info = match state.tmdb.get_details(&m_type, resolved_tmdb_id.parse().unwrap_or(0)).await {
        Ok(info) => info,
        Err(e) => {
            warn!("[play] TMDB lookup failed: {}", e);
            return (StatusCode::NOT_FOUND, "TMDB info not found").into_response();
        }
    };

    // 4. Fetch from all fast providers + drama specialists
    let registry = state.registry.clone();
    let mut all_streams = Vec::new();

    // Fast providers
    let fast = registry.list_fast();
    for provider in fast {
        let tmdb = tmdb_info.clone();
        let mtype = m_type.clone();
        let cb = registry.circuit_breaker().clone();
        let name = provider.name().to_string();
        let real_name = provider.real_name().to_string();
        let timeout_secs = provider.timeout_secs();

        match timeout(Duration::from_secs(timeout_secs), provider.fetch(&tmdb, &mtype, season, episode)).await {
            Ok(Ok(streams)) => {
                cb.record_success(&real_name);
                all_streams.extend(streams);
            }
            Ok(Err(e)) => {
                warn!("[play] Provider {} failed: {}", name, e);
                cb.record_failure(&real_name);
            }
            Err(_) => {
                warn!("[play] Provider {} timed out", name);
                cb.record_failure(&real_name);
            }
        }
    }

    // If no fast results, try slow providers too
    if all_streams.is_empty() {
        let slow = registry.list_slow();
        for provider in slow {
            let tmdb = tmdb_info.clone();
            let mtype = m_type.clone();
            let cb = registry.circuit_breaker().clone();
            let name = provider.name().to_string();
            let real_name = provider.real_name().to_string();

            match timeout(Duration::from_secs(15), provider.fetch(&tmdb, &mtype, season, episode)).await {
                Ok(Ok(streams)) => {
                    cb.record_success(&real_name);
                    all_streams.extend(streams);
                }
                Ok(Err(e)) => {
                    warn!("[play] Provider {} failed: {}", name, e);
                    cb.record_failure(&real_name);
                }
                Err(_) => {
                    warn!("[play] Provider {} timed out", name);
                    cb.record_failure(&real_name);
                }
            }
        }
    }

    // 5. Filter and sort
    let mut filtered = apply_filters(all_streams, &state.filter_config);
    sort_by_quality(&mut filtered);

    // Cache results
    state.cache.set(&cache_key, filtered.clone()).await;

    // 6. Pick best stream
    if let Some(best) = pick_best_stream(filtered, &state).await {
        (StatusCode::OK, serde_json::to_string(&best).unwrap()).into_response()
    } else {
        (StatusCode::NOT_FOUND, "No playable streams found").into_response()
    }
}

async fn pick_best_stream(streams: Vec<crate::models::StreamResult>, state: &AppState) -> Option<PlayResponse> {
    for stream in streams.into_iter().take(5) {
        // Pre-flight validation with HEAD request
        let req = state.client.head(&stream.url);
        let req = if let Some(ref headers) = stream.headers {
            let mut r = req;
            for (k, v) in headers {
                r = r.header(k, v);
            }
            r
        } else {
            req
        };

        match timeout(Duration::from_secs(state.config.head_check_timeout_secs), req.send()).await {
            Ok(Ok(resp)) => {
                if resp.status().is_success() || resp.status().as_u16() == 206 {
                    info!("[play] Validated stream from {}: {}", stream.provider, stream.url);
                    return Some(PlayResponse {
                        success: true,
                        url: stream.url,
                        title: stream.title,
                        quality: stream.quality,
                        provider: stream.provider,
                        format: stream.format,
                        subtitles: stream.subtitles,
                        headers: stream.headers,
                    });
                }
            }
            Ok(Err(e)) => {
                warn!("[play] HEAD check failed for {}: {}", stream.provider, e);
            }
            Err(_) => {
                warn!("[play] HEAD check timed out for {}", stream.provider);
            }
        }
    }
    None
}

fn sanitize_title(title: &str) -> String {
    title
        .replace(" (TV Series)", "")
        .replace(" (Movie)", "")
        .replace(" [Korean Drama]", "")
        .replace(" [Chinese Drama]", "")
        .trim()
        .to_string()
}
