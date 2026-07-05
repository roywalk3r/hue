//! Hue Stream API
//! Professional Streaming Aggregator
//!
//! @author roywalk3r <roywalker440@gmail.com>
//! @repository https://github.com/roywalk3r/hue.git
//! @license MIT

use crate::{
    app::StreamState,
    filters::{apply_filters, sort_by_quality},
    infra::cache::CacheKey,
    providers::dispatch::fetch_from_providers,
    proxy::dynamic_proxy_base,
    proxy::wrap::wrap_with_proxy,
    security::kkey_encrypt,
};
use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use std::collections::HashMap;
use std::time::Duration;
use tokio::time::timeout;
use tracing::{info, warn};
use utoipa::ToSchema;

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

    info!(
        "[play] Smart Play: {} {} S{} E{}",
        m_type, tmdb_id, season, episode
    );
    let proxy_base = dynamic_proxy_base(&headers, &state.proxy_base_url);

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
            let best = proxify_play_response(best, &state, &proxy_base);
            return (StatusCode::OK, Json(best)).into_response();
        }
    }

    // 3. Fetch TMDB details
    let tmdb_info = match state
        .tmdb
        .get_details(&m_type, resolved_tmdb_id.parse().unwrap_or(0))
        .await
    {
        Ok(info) => info,
        Err(e) => {
            warn!("[play] TMDB lookup failed: {}", e);
            return (StatusCode::NOT_FOUND, "TMDB info not found").into_response();
        }
    };

    // 4. Fetch fast providers first; only fall back to the full set if none work.
    let (mut all_streams, _) = fetch_from_providers(
        state.registry.clone(),
        state.metrics.clone(),
        &tmdb_info,
        &m_type,
        season,
        episode,
        true,
        1,
        state.config.fast_deadline(),
    )
    .await;
    if all_streams.is_empty() {
        let (fallback_streams, _) = fetch_from_providers(
            state.registry.clone(),
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
        all_streams = fallback_streams;
    }

    // 5. Filter and sort
    let mut filtered = apply_filters(all_streams, &state.filter_config);
    sort_by_quality(&mut filtered);

    // Cache results
    state.cache.set(&cache_key, filtered.clone()).await;

    // 6. Pick best stream
    if let Some(best) = pick_best_stream(filtered, &state).await {
        let best = proxify_play_response(best, &state, &proxy_base);
        (StatusCode::OK, Json(best)).into_response()
    } else {
        (StatusCode::NOT_FOUND, "No playable streams found").into_response()
    }
}

fn proxify_play_response(
    mut best: PlayResponse,
    state: &StreamState,
    proxy_base: &str,
) -> PlayResponse {
    if !state.config.enable_proxy {
        // Still encrypt even when proxy is disabled
        if !best.url.is_empty() {
            best.url = kkey_encrypt(&best.url, &state.kkey);
        }
        if let Some(ref mut subs) = best.subtitles {
            for sub in subs.iter_mut() {
                if !sub.url.is_empty() {
                    sub.url = kkey_encrypt(&sub.url, &state.kkey);
                }
            }
        }
        return best;
    }

    let stream = crate::models::StreamResult {
        url: best.url,
        title: best.title,
        quality: best.quality,
        provider: best.provider,
        format: best.format,
        subtitles: best.subtitles,
        headers: best.headers,
        extra_metadata: None,
    };

    let Some(mut stream) = wrap_with_proxy(vec![stream], proxy_base, &state.secret)
        .into_iter()
        .next()
    else {
        return PlayResponse {
            success: false,
            url: String::new(),
            title: String::new(),
            quality: String::new(),
            provider: String::new(),
            format: String::new(),
            subtitles: None,
            headers: None,
        };
    };

    // Kkey encrypt the proxied URL and subtitles
    if !stream.url.is_empty() {
        stream.url = kkey_encrypt(&stream.url, &state.kkey);
    }
    if let Some(ref mut subs) = stream.subtitles {
        for sub in subs.iter_mut() {
            if !sub.url.is_empty() {
                sub.url = kkey_encrypt(&sub.url, &state.kkey);
            }
        }
    }

    PlayResponse {
        success: true,
        url: stream.url,
        title: stream.title,
        quality: stream.quality,
        provider: stream.provider,
        format: stream.format,
        subtitles: stream.subtitles,
        headers: stream.headers,
    }
}

async fn pick_best_stream(
    streams: Vec<crate::models::StreamResult>,
    state: &StreamState,
) -> Option<PlayResponse> {
    let client = state.client.clone();
    let timeout_secs = state.config.head_check_timeout_secs;

    // Validate candidates concurrently; return the first playable response.
    let mut handles = tokio::task::JoinSet::new();
    for stream in streams.into_iter().take(5) {
        let client = client.clone();
        handles.spawn(async move {
            let is_hls = stream.format == "hls" || stream.url.to_lowercase().contains(".m3u8");
            let mut req = if is_hls {
                client.get(&stream.url)
            } else {
                client.get(&stream.url).header("Range", "bytes=0-1")
            };
            let req = if let Some(ref headers) = stream.headers {
                for (k, v) in headers {
                    req = req.header(k, v);
                }
                req
            } else {
                req
            };
            match timeout(Duration::from_secs(timeout_secs), req.send()).await {
                Ok(Ok(resp)) if resp.status().is_success() || resp.status().as_u16() == 206 => {
                    info!(
                        "[play] Validated stream from {}: {}",
                        stream.provider, stream.url
                    );
                    Some(PlayResponse {
                        success: true,
                        url: stream.url,
                        title: stream.title,
                        quality: stream.quality,
                        provider: stream.provider,
                        format: stream.format,
                        subtitles: stream.subtitles,
                        headers: stream.headers,
                    })
                }
                Ok(Ok(_)) => None,
                Ok(Err(e)) => {
                    warn!("[play] Stream validation failed: {}", e);
                    None
                }
                Err(_) => {
                    warn!("[play] Stream validation timed out");
                    None
                }
            }
        });
    }

    while let Some(Ok(result)) = handles.join_next().await {
        if result.is_some() {
            handles.abort_all();
            return result;
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
