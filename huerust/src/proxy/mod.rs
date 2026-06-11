//! Hue Stream API
//! Professional Streaming Aggregator
//!
//! @author roywalk3r <roywalker440@gmail.com>
//! @repository https://github.com/roywalk3r/hue.git
//! @license MIT

use axum::{
    body::Body,
    extract::{Query, State},
    http::{header::CONTENT_TYPE, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use std::collections::HashMap;
use std::net::IpAddr;
use url::Url;

use crate::{app::AppState, security};
use std::env;

pub mod adsurgery;
pub mod sanitize;
pub mod wrap;

pub(crate) fn is_whitelisted_raw_url(url: &str) -> bool {
    let lower = url.to_lowercase();
    lower.contains("vixsrc.to")
        || lower.contains("hakunaymatata.com")
        || lower.contains("aisports.mobi")
        || lower.contains("poocloud.in")
        || lower.contains("mewstream.buzz")
        || lower.contains("animeplay.cfd")
        || lower.contains("pixeldrain.")
        || lower.contains("video-downloads.googleusercontent.com")
        || lower.contains("cache.vdrk.site")
        || lower.contains("sub.vdrk.site")
}

const PROGRESSIVE_STARTUP_RANGE_END: u64 = (512 * 1024) - 1;
const PROGRESSIVE_OPEN_RANGE_CHUNK_END: u64 = (1 * 1024 * 1024) - 1;

// ── Query Types ──────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct ProxyQuery {
    pub h_token: Option<String>,
    #[allow(dead_code)]
    pub url: Option<String>,
    #[allow(dead_code)]
    pub headers: Option<String>,
    #[allow(dead_code)]
    pub mode: Option<String>,
    pub download: Option<String>,
    pub filename: Option<String>,
    pub adsurgery: Option<u32>,
}

#[derive(Deserialize)]
pub struct TsProxyQuery {
    pub h_token: Option<String>,
    #[allow(dead_code)]
    pub url: Option<String>,
    #[allow(dead_code)]
    pub headers: Option<String>,
    #[allow(dead_code)]
    pub mode: Option<String>,
    pub range: Option<String>,
    #[allow(dead_code)]
    #[serde(alias = "clampOpen")]
    pub clamp_open: Option<u32>,
    #[allow(dead_code)]
    #[serde(alias = "progressiveOpen")]
    pub progressive_open: Option<u32>,
    #[serde(alias = "noSynth")]
    pub no_synth: Option<u32>,
    #[serde(alias = "force200")]
    pub force_200: Option<u32>,
    #[allow(dead_code)]
    #[serde(alias = "tailPrefetch")]
    pub tail_prefetch: Option<u32>,
    pub sanitize: Option<u32>,
    pub download: Option<String>,
    pub filename: Option<String>,
}

// ── Shared helpers ───────────────────────────────────────────────────────────

/// Derive the public proxy base URL.
///
/// If the configured `fallback` is a valid URL with a real public host
/// (not localhost/127.0.0.1), use it directly. This prevents proxy URLs from
/// pointing back to the frontend domain (e.g. Vercel) when requests arrive
/// through a reverse-proxy that preserves the browser's Host header.
///
/// Only fall back to header detection when the configured URL is localhost,
/// which is the expected setup behind nginx or in local development.
pub fn dynamic_proxy_base(headers: &HeaderMap, fallback: &str) -> String {
    let fallback_url = fallback.parse::<Url>().ok();
    let default_proto = fallback_url.as_ref().map(|u| u.scheme()).unwrap_or("http");
    let default_host = fallback_url
        .as_ref()
        .and_then(|u| u.host_str())
        .unwrap_or("localhost");

    // Prefer the explicitly configured fallback when it points to a real host.
    // Use the full origin so configured ports are preserved.
    if let Some(ref url) = fallback_url {
        if let Some(host) = url.host_str() {
            let lower = host.to_lowercase();
            if lower != "localhost" && lower != "127.0.0.1" && lower != "0.0.0.0" && lower != "::1"
            {
                return url
                    .origin()
                    .unicode_serialization()
                    .trim_end_matches('/')
                    .to_string();
            }
        }
    }

    let host = headers
        .get("x-forwarded-host")
        .or_else(|| headers.get("host"))
        .and_then(|v| v.to_str().ok())
        .unwrap_or(default_host);

    let proto = headers
        .get("x-forwarded-proto")
        .or_else(|| headers.get("x-forwarded-protocol"))
        .and_then(|v| v.to_str().ok())
        .unwrap_or(default_proto);

    format!("{}://{}", proto, host)
}

/// Parse JSON headers string from query param.
fn parse_headers_json(raw: &str) -> Option<HashMap<String, String>> {
    serde_json::from_str::<HashMap<String, String>>(raw).ok()
}

/// Extract target URL + headers from `url`+`headers` mode or signed `h_token` mode.
fn resolve_proxy_target(
    params: &ProxyQuery,
    secret: &str,
) -> Option<(String, Option<HashMap<String, String>>)> {
    if let Some(token) = &params.h_token {
        if let Some(data) = security::decrypt_url(token, secret) {
            return Some((data.url, data.headers));
        }
    }
    if let Some(url) = &params.url {
        if !url.trim().is_empty() && is_whitelisted_raw_url(url) {
            let headers = params.headers.as_deref().and_then(parse_headers_json);
            return Some((url.clone(), headers));
        }
    }
    None
}

fn resolve_ts_target(
    params: &TsProxyQuery,
    secret: &str,
) -> Option<(String, Option<HashMap<String, String>>)> {
    if let Some(token) = &params.h_token {
        if let Some(data) = security::decrypt_url(token, secret) {
            return Some((data.url, data.headers));
        }
    }
    if let Some(url) = &params.url {
        if !url.trim().is_empty() && is_whitelisted_raw_url(url) {
            let headers = params.headers.as_deref().and_then(parse_headers_json);
            return Some((url.clone(), headers));
        }
    }
    None
}

/// Inject CDN-specific headers based on target host
fn inject_cdn_headers(
    mut req: reqwest::RequestBuilder,
    target_url: &str,
    token_headers: &Option<HashMap<String, String>>,
) -> reqwest::RequestBuilder {
    let host = match Url::parse(target_url) {
        Ok(u) => u.host_str().unwrap_or("").to_string(),
        Err(_) => return req,
    };

    tracing::info!(
        "[inject_cdn_headers] Host: {}, has_referer: {}",
        host,
        has_header(token_headers, "referer")
    );

    // Only inject host-based Referer/Origin if the token doesn't already have them
    let has_referer = has_header(token_headers, "referer");

    if !has_referer {
        let referer = if host.contains("vixsrc.to") {
            Some("https://vixsrc.to/")
        } else if host.contains("hakunaymatata.com")
            || host.contains("aisports.mobi")
            || host.contains("poocloud.in")
        {
            Some("https://moviebox.ph/")
        } else if host.contains("mewstream.buzz") || host.contains("animeplay.cfd") {
            Some("https://hianimes.se/")
        } else {
            None
        };

        if let Some(r) = referer {
            req = req.header("Referer", r);
            if let Ok(origin_url) = Url::parse(r) {
                let mut origin = origin_url.origin().unicode_serialization();
                if origin.ends_with('/') {
                    origin.pop();
                }
                tracing::info!("[inject_cdn_headers] Added derived Origin: {}", origin);
                req = req.header("Origin", origin);
            }
        }
    } else {
        // We have a Referer from token, but maybe no Origin. Match its logic.
        if let Some(h) = token_headers {
            tracing::info!(
                "[inject_cdn_headers] Has Referer, checking for Origin. Keys: {:?}",
                h.keys().collect::<Vec<_>>()
            );

            let mut referer_val = h.get("Referer").or_else(|| h.get("referer")).cloned();

            if !has_header(token_headers, "origin") {
                if let Some(r) = referer_val {
                    if let Ok(origin_url) = Url::parse(&r) {
                        let mut origin = origin_url.origin().unicode_serialization();
                        if origin.ends_with('/') {
                            origin.pop();
                        }
                        tracing::info!(
                            "[inject_cdn_headers] Derived Origin from Referer: {} -> {}",
                            r,
                            origin
                        );
                        req = req.header("Origin", origin);
                    } else {
                        tracing::warn!(
                            "[inject_cdn_headers] Failed to parse Origin from Referer: {}",
                            r
                        );
                    }
                }
            } else {
                tracing::info!("[inject_cdn_headers] Origin already present");
            }
        }
    }

    let has_ua = has_header(token_headers, "user-agent");
    if !has_ua {
        req = req.header(
            "User-Agent",
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
        );
    }

    req = req.header("Accept", "*/*");
    req
}

fn has_header(headers: &Option<HashMap<String, String>>, name: &str) -> bool {
    headers
        .as_ref()
        .map(|h| h.keys().any(|k| k.eq_ignore_ascii_case(name)))
        .unwrap_or(false)
}

/// Route the upstream fetch through the Cloudflare Worker for hosts that
/// geo-block datacenter IPs (e.g. MovieBox). The worker forwards the
/// request with the same headers, returning the upstream response.
fn worker_routed_url(
    target_url: &str,
    worker_url: &Option<String>,
    worker_hosts: &[String],
) -> String {
    if worker_url.is_none() || worker_hosts.is_empty() {
        return target_url.to_string();
    }
    let host = match Url::parse(target_url) {
        Ok(u) => u.host_str().unwrap_or("").to_lowercase(),
        Err(_) => return target_url.to_string(),
    };
    if worker_hosts
        .iter()
        .any(|h| host == *h || host.ends_with(&format!(".{}", h)))
    {
        if let Some(worker) = worker_url {
            return format!("{}?target={}", worker, urlencoding::encode(target_url));
        }
    }
    target_url.to_string()
}

fn is_vixsrc_target(target_url: &str, headers: &Option<HashMap<String, String>>) -> bool {
    if target_url.contains("vixsrc.to") {
        return true;
    }
    if let Some(h) = headers {
        if let Some(r) = h.get("Referer").or_else(|| h.get("referer")) {
            if r.contains("vixsrc.to") {
                return true;
            }
        }
    }
    false
}

fn is_progressive_video_target(target_url: &str) -> bool {
    let lower = target_url.to_lowercase();
    // Only apply progressive range clamping to HLS segment formats.
    // Full MP4/MKV/WebM files must not be truncated — mobile browsers need the
    // complete ftyp + moov atoms to demux (ffmpegDemux error otherwise).
    if lower.contains(".mp4") || lower.contains(".mkv") || lower.contains(".webm") {
        return false;
    }
    if lower.contains(".ts") || lower.contains(".m4s") {
        return true;
    }

    // Host-based heuristic: these CDNs also serve TS segments from
    // extensionless URLs, so keep progressive clamping for them.
    if let Ok(url) = Url::parse(target_url) {
        let host = url.host_str().unwrap_or("").to_lowercase();
        if host.contains("hakunaymatata.com")
            || host.contains("aisports.mobi")
            || host.contains("poocloud.in")
            || host.contains("pixeldrain.")
            || host == "video-downloads.googleusercontent.com"
        {
            return true;
        }
    }

    false
}

fn clamp_progressive_open_range(range: &str) -> Option<String> {
    let trimmed = range.trim();
    let spec = trimmed.strip_prefix("bytes=")?;
    let (start_raw, end_raw) = spec.split_once('-')?;
    if !end_raw.trim().is_empty() {
        return Some(trimmed.to_string());
    }

    let start = start_raw.trim().parse::<u64>().ok()?;
    let end = start.saturating_add(PROGRESSIVE_OPEN_RANGE_CHUNK_END);
    Some(format!("bytes={}-{}", start, end))
}

fn looks_like_hls_playlist_url(target_url: &str) -> bool {
    let lower = target_url.to_lowercase();
    if lower.contains(".m3u8") {
        return true;
    }

    if let Ok(url) = Url::parse(target_url) {
        let host = url.host_str().unwrap_or("").to_lowercase();
        let path = url.path().to_lowercase();

        // VixSrc serves HLS playlists from extensionless /playlist/... URLs.
        // Treating those as TS makes HLS.js receive an un-rewritten child
        // playlist, which then leaks raw vix-content.net segment URLs.
        if host.contains("vixsrc.to") && path.starts_with("/playlist/") {
            return true;
        }
    }

    false
}

// ── M3U8 Proxy ───────────────────────────────────────────────────────────────

/// Send an M3U8 request and return (status, body, content_type).
async fn send_m3u8_request(
    _client: &reqwest::Client,
    req: reqwest::RequestBuilder,
    target_url: &str,
    _is_vix: bool,
) -> Result<(reqwest::StatusCode, String, String), String> {
    match req.send().await {
        Ok(resp) => {
            let status = resp.status();
            let ct = resp
                .headers()
                .get("content-type")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("")
                .to_string();
            match resp.text().await {
                Ok(body) => Ok((status, body, ct)),
                Err(e) => {
                    tracing::warn!("[m3u8-proxy] Failed to read body for {}: {}", target_url, e);
                    Err(format!("Upstream body unreadable: {}", e))
                }
            }
        }
        Err(e) => {
            tracing::warn!("[m3u8-proxy] Request failed for {}: {}", target_url, e);
            Err(format!("Upstream request failed: {}", e))
        }
    }
}

pub async fn m3u8_handler(
    State(state): State<AppState>,
    Query(params): Query<ProxyQuery>,
    request_headers: HeaderMap,
) -> Response {
    // Handle CORS preflight so cross-origin <video> manifest requests don't get blocked.
    if request_headers
        .get("access-control-request-method")
        .is_some()
        || request_headers
            .get("access-control-request-headers")
            .is_some()
    {
        let mut builder = Response::builder().status(StatusCode::NO_CONTENT);
        builder = builder.header("Access-Control-Allow-Origin", "*");
        builder = builder.header("Access-Control-Allow-Methods", "GET, HEAD, OPTIONS");
        builder = builder.header("Access-Control-Allow-Headers", "*");
        builder = builder.header("Access-Control-Max-Age", "86400");
        return builder.body(Body::empty()).unwrap();
    }

    let (target_url, headers) = match resolve_proxy_target(&params, &state.secret) {
        Some(t) => t,
        None => return (StatusCode::FORBIDDEN, "Invalid or missing signed token").into_response(),
    };

    if is_internal_host(&target_url) {
        return (StatusCode::FORBIDDEN, "Blocked internal URL").into_response();
    }

    let is_vix = is_vixsrc_target(&target_url, &headers);
    let mut req = state.raw_client.get(&worker_routed_url(
        &target_url,
        &state.config.cf_worker_url,
        &state.config.worker_upstream_hosts,
    ));
    req = inject_cdn_headers(req, &target_url, &headers);

    // Inject headers from token (overrides defaults)
    let mut referer_value = None;
    let mut has_origin = false;
    if let Some(ref h) = headers {
        for (k, v) in h.iter() {
            if k.eq_ignore_ascii_case("origin") {
                has_origin = true;
            }
            if k.eq_ignore_ascii_case("referer") {
                referer_value = Some(v.clone());
            }
            req = req.header(k, v);
        }
    }

    // Vixsrc Special: If it's Vixsrc, we MUST ensure the Referer is correct
    if is_vix && referer_value.is_none() {
        req = req.header("Referer", "https://vixsrc.to/");
        referer_value = Some("https://vixsrc.to/".to_string());
    }

    if !has_origin {
        if let Some(ref ref_url) = referer_value {
            if let Ok(origin_url) = Url::parse(ref_url) {
                let mut origin = origin_url.origin().unicode_serialization();
                if origin.ends_with('/') {
                    origin.pop();
                }
                req = req.header("Origin", origin);
            }
        } else {
            // Fallback: derive Origin from the target host itself if Referer is missing
            if let Ok(target_u) = Url::parse(&target_url) {
                let mut origin = target_u.origin().unicode_serialization();
                if origin.ends_with('/') {
                    origin.pop();
                }
                req = req.header("Origin", origin);
            }
        }
    }

    // Debug: log full request shape for Vixsrc before sending
    if is_vix {
        let mut header_log = Vec::new();
        // We can't introspect reqwest::RequestBuilder directly, but we know what we injected
        if let Some(ref h) = headers {
            for (k, v) in h.iter() {
                header_log.push(format!("{}: {}", k, v));
            }
        }
        if referer_value.is_some() {
            header_log.push(format!(
                "referer(from-inject): {}",
                referer_value.as_ref().unwrap()
            ));
        }
        tracing::info!(
            "[m3u8-proxy] Vixsrc request → {} | headers: {:?}",
            target_url,
            header_log
        );
    }

    let (status, body, ct) =
        match send_m3u8_request(&state.raw_client, req, &target_url, is_vix).await {
            Ok(result) => result,
            Err(e) => return (StatusCode::BAD_GATEWAY, e).into_response(),
        };

    let body_trim = body.trim_start_matches('\u{feff}').trim_start();
    let is_hls = ct.contains("mpegurl")
        || ct.contains("x-mpegurl")
        || ct.contains("vnd.apple.mpegurl")
        || body_trim.starts_with("#EXTM3U")
        || (ct.is_empty() && body_trim.starts_with("#EXTM3U"));

    // m3u8-proxy was requested but upstream didn't return a valid M3U8 —
    // don't forward HTML error pages as 200; that causes HLS.js manifestParsingError
    let looks_like_html = body_trim.starts_with("<!DOCTYPE")
        || body_trim.starts_with("<html")
        || body_trim.starts_with("<HTML")
        || body_trim.starts_with("<?xml");
    let looks_like_json = body_trim.starts_with("{") || body_trim.starts_with("[");
    if !is_hls && (looks_like_html || looks_like_json) {
        // Log full body for Vixsrc so we can see the exact error
        if is_vix {
            let now = chrono::Utc::now().timestamp();
            let expires_val = Url::parse(&target_url).ok().and_then(|u| {
                u.query_pairs()
                    .find(|(k, _)| k == "expires")
                    .map(|(_, v)| v.to_string())
            });
            let token_val = Url::parse(&target_url).ok().and_then(|u| {
                u.query_pairs()
                    .find(|(k, _)| k == "token")
                    .map(|(_, v)| v.to_string())
            });
            tracing::error!(
                "[m3u8-proxy] VIXSRC FAILURE — status={}, now={}, expires={:?}, has_token={} body={}",
                status.as_u16(),
                now,
                expires_val.as_deref().unwrap_or("NONE"),
                token_val.is_some(),
                body  // full body, likely small HTML error page
            );
        }

        tracing::warn!(
            "[m3u8-proxy] Upstream returned non-HLS (status={}, ct={}, body_preview={:?}) for {}",
            status.as_u16(),
            ct,
            body.chars().take(80).collect::<String>(),
            target_url
        );

        // Retry once for Vixsrc with minimal headers (strip everything except Referer + UA + Origin)
        if is_vix && (status.as_u16() == 403 || status.as_u16() == 401) {
            tracing::info!(
                "[m3u8-proxy] Vixsrc got {} — retrying with minimal header set",
                status.as_u16()
            );
            let mut retry = state.raw_client.get(&worker_routed_url(
                &target_url,
                &state.config.cf_worker_url,
                &state.config.worker_upstream_hosts,
            ));
            retry = retry.header("Referer", "https://vixsrc.to/");
            retry = retry.header("Origin", "https://vixsrc.to");
            retry = retry.header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36");
            retry = retry.header("Accept", "*/*");

            if let Ok((status2, body2, ct2)) =
                send_m3u8_request(&state.raw_client, retry, &target_url, false).await
            {
                let body_trim2 = body2.trim_start_matches('\u{feff}').trim_start();
                let is_hls2 = ct2.contains("mpegurl")
                    || ct2.contains("x-mpegurl")
                    || ct2.contains("vnd.apple.mpegurl")
                    || body_trim2.starts_with("#EXTM3U")
                    || (ct2.is_empty() && body_trim2.starts_with("#EXTM3U"));
                if is_hls2 {
                    tracing::info!("[m3u8-proxy] Vixsrc retry succeeded with minimal headers");
                    let proxy_base = dynamic_proxy_base(&request_headers, &state.proxy_base_url);
                    let final_body =
                        rewrite_playlist(&body2, &target_url, &proxy_base, &state.secret, None);
                    let mut builder = Response::builder()
                        .status(StatusCode::from_u16(status2.as_u16()).unwrap_or(StatusCode::OK));
                    builder = builder.header("Access-Control-Allow-Origin", "*");
                    builder = builder.header(CONTENT_TYPE, infer_content_type(&ct2, &target_url));
                    return builder.body(Body::from(final_body)).unwrap();
                } else {
                    tracing::warn!(
                        "[m3u8-proxy] Vixsrc retry also failed (status={}, ct={}, preview={:?})",
                        status2.as_u16(),
                        ct2,
                        body2.chars().take(80).collect::<String>(),
                    );
                }
            }
        }

        return (
            StatusCode::BAD_GATEWAY,
            format!(
                "Upstream returned non-HLS content (status={}, ct={})",
                status.as_u16(),
                ct
            ),
        )
            .into_response();
    }

    // Ad-surgery if requested or globally enabled
    let adsurgery_enabled = params.adsurgery.unwrap_or(0) == 1
        || env::var("SANITIZE_STREAMS").map_or(false, |v| v == "true" || v == "1");

    let mut body = body;
    if is_hls && adsurgery_enabled {
        let (cleaned, removed) = adsurgery::sanitize_manifest(&body);
        if removed > 0 {
            tracing::debug!(
                "[m3u8-proxy] Ad-surgery removed {} segments from {}",
                removed,
                target_url
            );
        }
        body = cleaned;
    }

    let proxy_base = dynamic_proxy_base(&request_headers, &state.proxy_base_url);
    let final_body = if is_hls {
        rewrite_playlist(&body, &target_url, &proxy_base, &state.secret, headers)
    } else {
        body
    };

    let mut builder =
        Response::builder().status(StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::OK));
    builder = builder.header("Access-Control-Allow-Origin", "*");

    // Cache m3u8 playlists briefly so HLS.js doesn't re-fetch identical manifests
    // on every level switch or recovery attempt.
    builder = builder.header("Cache-Control", "public, max-age=30");

    // Download mode: force attachment disposition
    if params.download.is_some() {
        let filename = params.filename.as_deref().unwrap_or("playlist.m3u8");
        builder = builder.header(
            "Content-Disposition",
            format!(r#"attachment; filename="{}""#, filename),
        );
    }

    builder = builder.header(CONTENT_TYPE, infer_content_type(&ct, &target_url));
    builder.body(Body::from(final_body)).unwrap()
}

// ── TS / Segment Proxy ───────────────────────────────────────────────────────

/// Block localhost/internal IPs for security
fn is_internal_host(target_url: &str) -> bool {
    if let Ok(u) = Url::parse(target_url) {
        if let Some(host) = u.host_str() {
            let h = host.to_lowercase();
            if h == "localhost" {
                return true;
            }

            if let Ok(ip) = h.parse::<IpAddr>() {
                return match ip {
                    IpAddr::V4(v4) => {
                        v4.is_loopback()
                            || v4.is_private()
                            || v4.is_link_local()
                            || v4.is_unspecified()
                    }
                    IpAddr::V6(v6) => {
                        v6.is_loopback()
                            || v6.is_unspecified()
                            || v6.is_unique_local()
                            || v6.segments()[0] & 0xffc0 == 0xfe80
                    }
                };
            }
        }
    }
    false
}

pub async fn ts_handler(
    State(state): State<AppState>,
    Query(params): Query<TsProxyQuery>,
    request_headers: HeaderMap,
) -> Response {
    // Handle CORS preflight so cross-origin <video> range requests don't get blocked.
    if request_headers
        .get("access-control-request-method")
        .is_some()
        || request_headers
            .get("access-control-request-headers")
            .is_some()
    {
        let mut builder = Response::builder().status(StatusCode::NO_CONTENT);
        builder = builder.header("Access-Control-Allow-Origin", "*");
        builder = builder.header("Access-Control-Allow-Methods", "GET, HEAD, OPTIONS");
        builder = builder.header("Access-Control-Allow-Headers", "*");
        builder = builder.header("Access-Control-Max-Age", "86400");
        return builder.body(Body::empty()).unwrap();
    }

    let (target_url, headers) = match resolve_ts_target(&params, &state.secret) {
        Some(t) => t,
        None => {
            return (StatusCode::FORBIDDEN, "Invalid or missing signed token").into_response();
        }
    };

    if is_internal_host(&target_url) {
        return (StatusCode::FORBIDDEN, "Blocked internal URL").into_response();
    }

    let force_200 = params.force_200.unwrap_or(0) == 1;
    let no_synth = params.no_synth.unwrap_or(0) == 1;
    let sanitize = params.sanitize.unwrap_or(0) == 1;
    let is_download = params.download.is_some();

    let is_vix = is_vixsrc_target(&target_url, &headers);
    let incoming_range = params.range.as_deref().or_else(|| {
        request_headers
            .get("range")
            .and_then(|value| value.to_str().ok())
    });
    let progressive_video = is_progressive_video_target(&target_url);
    let effective_range = if progressive_video {
        incoming_range.and_then(clamp_progressive_open_range)
    } else {
        incoming_range.map(str::to_string)
    };
    let synthesized_startup_range =
        incoming_range.is_none() && !force_200 && !no_synth && progressive_video;

    tracing::info!("[ts-proxy] Target: {}", target_url);
    tracing::info!("[ts-proxy] Token headers: {:?}", headers);

    let mut req = state.raw_client.get(&worker_routed_url(
        &target_url,
        if is_download { &None } else { &state.config.cf_worker_url },
        &state.config.worker_upstream_hosts,
    ));
    req = inject_cdn_headers(req, &target_url, &headers);
    req = req.header("Accept-Encoding", "identity");

    let mut has_origin = false;
    let mut referer_value = None;
    if let Some(ref h) = headers {
        for (k, v) in h {
            req = req.header(k, v);
            if k.eq_ignore_ascii_case("origin") {
                has_origin = true;
            }
            if k.eq_ignore_ascii_case("referer") {
                referer_value = Some(v.clone());
            }
        }
    }

    if is_vix && referer_value.is_none() {
        req = req.header("Referer", "https://vixsrc.to/");
        req = req.header("Origin", "https://vixsrc.to");
        referer_value = Some("https://vixsrc.to/".to_string());
        has_origin = true;
    }

    if !has_origin {
        if let Some(ref_url) = referer_value {
            if let Ok(origin_url) = Url::parse(&ref_url) {
                let mut origin = origin_url.origin().unicode_serialization();
                if origin.ends_with('/') {
                    origin.pop();
                }
                req = req.header("Origin", origin);
            }
        } else {
            if let Ok(target_u) = Url::parse(&target_url) {
                let mut origin = target_u.origin().unicode_serialization();
                if origin.ends_with('/') {
                    origin.pop();
                }
                req = req.header("Origin", origin);
            }
        }
    }

    if let Some(range) = effective_range.as_deref() {
        req = req.header("Range", range);
    } else if synthesized_startup_range {
        req = req.header(
            "Range",
            format!("bytes=0-{}", PROGRESSIVE_STARTUP_RANGE_END),
        );
    }

    match req.send().await {
        Ok(resp) => {
            let status = resp.status();

            // Handle redirects manually — block localhost destinations
            if status.is_redirection() {
                if let Some(loc) = resp.headers().get("location") {
                    if let Ok(loc_str) = loc.to_str() {
                        if is_internal_host(loc_str) {
                            tracing::error!(
                                "[ts-proxy] Blocked redirect to localhost: {} from {}",
                                loc_str,
                                target_url
                            );
                            return (
                                StatusCode::FORBIDDEN,
                                "Blocked redirect to internal network",
                            )
                                .into_response();
                        }
                        // Follow redirect (one level)
                        let follow = state.raw_client.get(loc_str);
                        if let Ok(resp2) = follow.send().await {
                            let status2 = resp2.status();
                            let mut builder = Response::builder().status(
                                StatusCode::from_u16(status2.as_u16()).unwrap_or(StatusCode::OK),
                            );
                            if let Some(ct) = resp2.headers().get("content-type") {
                                if let Ok(ct_str) = ct.to_str() {
                                    builder = builder.header(
                                        "content-type",
                                        infer_content_type(ct_str, &target_url),
                                    );
                                }
                            }
                            builder = builder.header("Access-Control-Allow-Origin", "*");
                            builder =
                                builder.header("Cache-Control", "public, max-age=86400, immutable");
                            if params.download.is_some() {
                                let filename = params.filename.as_deref().unwrap_or("video.mp4");
                                builder = builder.header(
                                    "Content-Disposition",
                                    format!(r#"attachment; filename="{}""#, filename),
                                );
                            }
                            return builder
                                .body(Body::from_stream(resp2.bytes_stream()))
                                .unwrap();
                        }
                    }
                }
            }

            // If upstream returned non-success, try direct with headers
            if !status.is_success() && status != reqwest::StatusCode::PARTIAL_CONTENT {
                tracing::warn!(
                    "[ts-proxy] Upstream returned {} for {}, trying direct",
                    status.as_u16(),
                    target_url
                );
                let mut direct = state.raw_client.get(&target_url);
                direct = inject_cdn_headers(direct, &target_url, &headers);
                if let Some(range) = effective_range.as_deref() {
                    direct = direct.header("Range", range);
                }
                if let Ok(direct_resp) = direct.send().await {
                    let direct_status = direct_resp.status();
                    if direct_status.is_success()
                        || direct_status == reqwest::StatusCode::PARTIAL_CONTENT
                    {
                        let mut builder = Response::builder().status(StatusCode::OK);
                        if let Some(ct) = direct_resp.headers().get("content-type") {
                            if let Ok(ct_str) = ct.to_str() {
                                builder = builder.header(
                                    "content-type",
                                    infer_content_type(ct_str, &target_url),
                                );
                            }
                        }
                        builder = builder.header("Access-Control-Allow-Origin", "*");
                        builder =
                            builder.header("Cache-Control", "public, max-age=86400, immutable");
                        if let Some(cl) = direct_resp.headers().get("content-length") {
                            if let Ok(v) = cl.to_str() {
                                builder = builder.header("content-length", v);
                            }
                        }
                        if let Some(cr) = direct_resp.headers().get("content-range") {
                            if let Ok(v) = cr.to_str() {
                                builder = builder.header("content-range", v);
                            }
                        }
                        if params.download.is_some() {
                            let filename = params.filename.as_deref().unwrap_or("video.mp4");
                            builder = builder.header(
                                "Content-Disposition",
                                format!(r#"attachment; filename="{}""#, filename),
                            );
                        }
                        return builder
                            .body(Body::from_stream(direct_resp.bytes_stream()))
                            .unwrap();
                    }
                }

                // Second fallback: try plain request without any custom headers
                tracing::warn!(
                    "[ts-proxy] Direct with headers also failed for {}, trying plain",
                    target_url
                );
                let mut plain = state.raw_client.get(&target_url);
                if let Some(range) = effective_range.as_deref() {
                    plain = plain.header("Range", range);
                }
                if let Ok(plain_resp) = plain.send().await {
                    let plain_status = plain_resp.status();
                    if plain_status.is_success()
                        || plain_status == reqwest::StatusCode::PARTIAL_CONTENT
                    {
                        let mut builder = Response::builder().status(StatusCode::OK);
                        if let Some(ct) = plain_resp.headers().get("content-type") {
                            if let Ok(ct_str) = ct.to_str() {
                                builder = builder.header(
                                    "content-type",
                                    infer_content_type(ct_str, &target_url),
                                );
                            }
                        }
                        builder = builder.header("Access-Control-Allow-Origin", "*");
                        builder =
                            builder.header("Cache-Control", "public, max-age=86400, immutable");
                        if let Some(cl) = plain_resp.headers().get("content-length") {
                            if let Ok(v) = cl.to_str() {
                                builder = builder.header("content-length", v);
                            }
                        }
                        if let Some(cr) = plain_resp.headers().get("content-range") {
                            if let Ok(v) = cr.to_str() {
                                builder = builder.header("content-range", v);
                            }
                        }
                        if params.download.is_some() {
                            let filename = params.filename.as_deref().unwrap_or("video.mp4");
                            builder = builder.header(
                                "Content-Disposition",
                                format!(r#"attachment; filename="{}""#, filename),
                            );
                        }
                        return builder
                            .body(Body::from_stream(plain_resp.bytes_stream()))
                            .unwrap();
                    }
                }
            }

            let mut builder = Response::builder();
            let mapped = StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::OK);
            builder = builder.status(mapped);

            if let Some(ct) = resp.headers().get("content-type") {
                if let Ok(ct_str) = ct.to_str() {
                    builder =
                        builder.header("content-type", infer_content_type(ct_str, &target_url));
                }
            }

            builder = builder.header("Access-Control-Allow-Origin", "*");

            // Cache progressive video segments for 24h so the browser doesn't
            // re-fetch the same byte ranges on every seek/retry.
            builder = builder.header("Cache-Control", "public, max-age=86400, immutable");

            if force_200 && mapped == StatusCode::PARTIAL_CONTENT {
                builder = builder.status(StatusCode::OK);
            }

            // Download mode: force attachment disposition
            if params.download.is_some() {
                let filename = params.filename.as_deref().unwrap_or("video.mp4");
                builder = builder.header(
                    "Content-Disposition",
                    format!(r#"attachment; filename="{}""#, filename),
                );
            }

            if sanitize && params.download.is_none() {
                match resp.bytes().await {
                    Ok(body) => {
                        let bytes = body.to_vec();
                        if sanitize::is_valid_ts(&bytes) {
                            let (cleaned, stripped) = sanitize::sanitize_ts_segment(&bytes);
                            if stripped > 0 {
                                tracing::info!(
                                    "[ts-proxy] Sanitized {} packets from {}",
                                    stripped,
                                    target_url
                                );
                            }
                            builder = builder.header("content-length", cleaned.len());
                            return builder.body(Body::from(cleaned)).unwrap();
                        }
                        builder = builder.header("content-length", bytes.len());
                        builder.body(Body::from(bytes)).unwrap()
                    }
                    Err(e) => (StatusCode::BAD_GATEWAY, e.to_string()).into_response(),
                }
            } else if params.download.is_some() {
                // Download mode: buffer the full upstream body so the response
                // has a known Content-Length (browser can show download size)
                // and we never hand the browser a half-streamed chunked body.
                let upstream_status = status;
                let upstream_headers = resp.headers().clone();
                match resp.bytes().await {
                    Ok(bytes) => {
                        let len = bytes.len();
                        builder = builder.header("content-length", len);
                        tracing::info!(
                            "[ts-proxy] Download buffered {} bytes for {} (status={})",
                            len,
                            target_url,
                            upstream_status.as_u16()
                        );
                        builder.body(Body::from(bytes)).unwrap()
                    }
                    Err(e) => {
                        tracing::warn!(
                            "[ts-proxy] Download body read failed for {}: {} (status={}, cl_header={:?})",
                            target_url,
                            e,
                            upstream_status.as_u16(),
                            upstream_headers.get("content-length").and_then(|v| v.to_str().ok())
                        );
                        (StatusCode::BAD_GATEWAY, e.to_string()).into_response()
                    }
                }
            } else {
                if let Some(cl) = resp.headers().get("content-length") {
                    if let Ok(v) = cl.to_str() {
                        builder = builder.header("content-length", v);
                    }
                }
                if let Some(cr) = resp.headers().get("content-range") {
                    if let Ok(v) = cr.to_str() {
                        builder = builder.header("content-range", v);
                    }
                }
                builder = builder.header("accept-ranges", "bytes");
                builder
                    .body(Body::from_stream(resp.bytes_stream()))
                    .unwrap()
            }
        }
        Err(e) => {
            // Retry once without Range header if the error might be range-related
            if params.range.is_some() {
                tracing::warn!(
                    "[ts-proxy] Initial request failed ({}), retrying without Range",
                    e
                );
                let mut retry = state.raw_client.get(&worker_routed_url(
                    &target_url,
                    &state.config.cf_worker_url,
                    &state.config.worker_upstream_hosts,
                ));
                retry = inject_cdn_headers(retry, &target_url, &headers);
                if let Some(ref h) = headers {
                    for (k, v) in h {
                        retry = retry.header(k, v);
                    }
                }
                if let Ok(resp) = retry.send().await {
                    let status = resp.status();
                    if status.is_success() || status == reqwest::StatusCode::PARTIAL_CONTENT {
                        let mut builder = Response::builder().status(StatusCode::OK);
                        if let Some(ct) = resp.headers().get("content-type") {
                            if let Ok(ct_str) = ct.to_str() {
                                builder = builder.header(
                                    "content-type",
                                    infer_content_type(ct_str, &target_url),
                                );
                            }
                        }
                        builder = builder.header("Access-Control-Allow-Origin", "*");
                        builder =
                            builder.header("Cache-Control", "public, max-age=86400, immutable");
                        if let Some(cl) = resp.headers().get("content-length") {
                            if let Ok(v) = cl.to_str() {
                                builder = builder.header("content-length", v);
                            }
                        }
                        if let Some(cr) = resp.headers().get("content-range") {
                            if let Ok(v) = cr.to_str() {
                                builder = builder.header("content-range", v);
                            }
                        }
                        if params.download.is_some() {
                            let filename = params.filename.as_deref().unwrap_or("video.mp4");
                            builder = builder.header(
                                "Content-Disposition",
                                format!(r#"attachment; filename="{}""#, filename),
                            );
                        }
                        return builder
                            .body(Body::from_stream(resp.bytes_stream()))
                            .unwrap();
                    }
                }
            }
            (StatusCode::BAD_GATEWAY, e.to_string()).into_response()
        }
    }
}

// ── Subtitle Proxy ───────────────────────────────────────────────────────────

pub async fn sub_handler(
    State(state): State<AppState>,
    Query(params): Query<ProxyQuery>,
    request_headers: HeaderMap,
) -> Response {
    // Handle CORS preflight for subtitle track requests.
    if request_headers
        .get("access-control-request-method")
        .is_some()
        || request_headers
            .get("access-control-request-headers")
            .is_some()
    {
        let mut builder = Response::builder().status(StatusCode::NO_CONTENT);
        builder = builder.header("Access-Control-Allow-Origin", "*");
        builder = builder.header("Access-Control-Allow-Methods", "GET, HEAD, OPTIONS");
        builder = builder.header("Access-Control-Allow-Headers", "*");
        builder = builder.header("Access-Control-Max-Age", "86400");
        return builder.body(Body::empty()).unwrap();
    }

    let (target_url, headers) = match resolve_proxy_target(&params, &state.secret) {
        Some(t) => t,
        None => return (StatusCode::FORBIDDEN, "Invalid or missing signed token").into_response(),
    };

    if is_internal_host(&target_url) {
        return (StatusCode::FORBIDDEN, "Blocked internal URL").into_response();
    }

    let mut req = state.raw_client.get(&worker_routed_url(
        &target_url,
        &state.config.cf_worker_url,
        &state.config.worker_upstream_hosts,
    ));
    req = inject_cdn_headers(req, &target_url, &headers);

    // Inject headers from token (overrides defaults)
    let mut referer_value = None;
    let mut has_origin = false;
    if let Some(ref h) = headers {
        for (k, v) in h {
            if k.eq_ignore_ascii_case("origin") {
                has_origin = true;
            }
            if k.eq_ignore_ascii_case("referer") {
                referer_value = Some(v.clone());
            }
            req = req.header(k, v);
        }
    }
    if !has_origin {
        if let Some(ref_url) = referer_value {
            if let Ok(origin_url) = Url::parse(&ref_url) {
                let mut origin = origin_url.origin().unicode_serialization();
                if origin.ends_with('/') {
                    origin.pop();
                }
                req = req.header("Origin", origin);
            }
        } else {
            // Fallback: derive Origin from the target host itself if Referer is missing
            if let Ok(target_u) = Url::parse(&target_url) {
                let mut origin = target_u.origin().unicode_serialization();
                if origin.ends_with('/') {
                    origin.pop();
                }
                req = req.header("Origin", origin);
            }
        }
    }

    match req.send().await {
        Ok(resp) => {
            let status = resp.status();
            if !status.is_success() {
                let preview = resp.text().await.unwrap_or_default();
                tracing::warn!(
                    "[sub-proxy] Worker returned non-success (status={}, url={}, preview={:?})",
                    status.as_u16(),
                    target_url,
                    preview.chars().take(120).collect::<String>(),
                );
                // Try direct request bypassing worker
                tracing::warn!("[sub-proxy] Trying direct request to {}", target_url);
                let mut direct = state.raw_client.get(&target_url);
                direct = inject_cdn_headers(direct, &target_url, &headers);
                if let Some(ref h) = headers {
                    for (k, v) in h {
                        direct = direct.header(k, v);
                    }
                }
                if let Ok(direct_resp) = direct.send().await {
                    let direct_status = direct_resp.status();
                    if direct_status.is_success() {
                        let body = direct_resp.text().await.unwrap_or_default();
                        let final_body = if body.trim_start().starts_with("WEBVTT") {
                            normalize_vtt_timestamps(&body)
                        } else if target_url.to_lowercase().contains(".srt")
                            || body.trim().starts_with("1")
                            || body.contains(" -->")
                        {
                            convert_srt_to_vtt(&body)
                        } else {
                            body
                        };
                        return Response::builder()
                            .status(StatusCode::OK)
                            .header("content-type", "text/vtt")
                            .header("Access-Control-Allow-Origin", "*")
                            .body(Body::from(final_body))
                            .unwrap();
                    }
                }
                return (
                    StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY),
                    format!("Subtitle upstream error: {}", status),
                )
                    .into_response();
            }

            let body = resp.text().await.unwrap_or_default();
            let final_body = if body.trim_start().starts_with("WEBVTT") {
                normalize_vtt_timestamps(&body)
            } else if target_url.to_lowercase().contains(".srt")
                || body.trim().starts_with("1")
                || body.contains(" --> ")
            {
                convert_srt_to_vtt(&body)
            } else {
                body
            };
            Response::builder()
                .status(StatusCode::OK)
                .header("content-type", "text/vtt")
                .header("Access-Control-Allow-Origin", "*")
                .body(Body::from(final_body))
                .unwrap()
        }
        Err(e) => {
            tracing::warn!("[sub-proxy] Request failed for {}: {}", target_url, e);
            (StatusCode::BAD_GATEWAY, e.to_string()).into_response()
        }
    }
}

// ── Playlist Rewriting ─────────────────────────────────────────────────────────

fn rewrite_playlist(
    content: &str,
    target_url: &str,
    proxy_base: &str,
    secret: &str,
    headers: Option<HashMap<String, String>>,
) -> String {
    let base_url = match Url::parse(target_url) {
        Ok(u) => u,
        Err(_) => return content.to_string(),
    };

    // Collect segment URLs for prefetch after rewrite
    let mut segment_urls: Vec<String> = Vec::new();

    let mut rewritten_lines: Vec<String> = Vec::new();
    let mut pending_byte_range: Option<String> = None;
    let mut next_byte_offset: Option<u64> = None;

    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(range_spec) = trimmed.strip_prefix("#EXT-X-BYTERANGE:") {
            pending_byte_range = parse_hls_byte_range(range_spec, &mut next_byte_offset);
            rewritten_lines.push(line.to_string());
            continue;
        }

        // Tags that carry a URI attribute
        if line.starts_with("#") {
            if let Some(rewritten_line) =
                rewrite_hls_tag_uri(line, proxy_base, secret, &base_url, &headers)
            {
                rewritten_lines.push(rewritten_line);
                continue;
            }
            rewritten_lines.push(line.to_string());
        } else if !line.trim().is_empty() {
            let line_trim = line.trim();
            let abs_url = unwrap_aisports_worker_url(&resolve_segment_url(&base_url, line_trim));
            let is_hls = looks_like_hls_playlist_url(&abs_url);
            let route = if is_hls { "m3u8-proxy" } else { "ts-proxy" };

            // MP3-wrapped-TS detection
            let is_mp3_wrapped_ts = abs_url.to_lowercase().contains(".mp3");
            let force_type_param = if is_mp3_wrapped_ts {
                "&forceType=video%2Fmp2t"
            } else {
                ""
            };

            // Track segment URLs for prefetch
            if !is_hls {
                segment_urls.push(abs_url.clone());
            }

            let use_raw = is_whitelisted_raw_url(&abs_url) && headers.is_none();
            // Per-rewrite nonce so the browser never replays a cached 4xx
            // against the deterministic h_token / raw-url path.
            let nonce = format!(
                "&_n={:x}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_nanos())
                    .unwrap_or(0)
            );
            let proxy_url = if use_raw {
                let encoded_url = urlencoding::encode(&abs_url);
                let header_part = if let Some(ref h) = headers {
                    format!(
                        "&headers={}",
                        urlencoding::encode(&serde_json::to_string(h).unwrap_or_default())
                    )
                } else {
                    String::new()
                };
                let range_part = pending_byte_range
                    .take()
                    .filter(|_| !is_hls)
                    .map(|range| format!("&range={}", urlencoding::encode(&range)))
                    .unwrap_or_default();
                format!(
                    "{}/{}?url={}&mode=url{}{}{}{}",
                    proxy_base, route, encoded_url, header_part, force_type_param, range_part, nonce
                )
            } else {
                let token = security::encrypt_url(&abs_url, secret, headers.clone());
                let segment_extra = if is_hls {
                    ""
                } else {
                    "&clampOpen=0&progressiveOpen=0&noSynth=1&tailPrefetch=0"
                };
                let range_part = pending_byte_range
                    .take()
                    .filter(|_| !is_hls)
                    .map(|range| format!("&range={}", urlencoding::encode(&range)))
                    .unwrap_or_default();
                format!(
                    "{}/{}?h_token={}{}{}{}{}",
                    proxy_base, route, token, segment_extra, force_type_param, range_part, nonce
                )
            };
            rewritten_lines.push(proxy_url);
        } else {
            rewritten_lines.push(line.to_string());
        }
    }

    let rewritten = rewritten_lines.join("\n");

    rewritten
}

/// Extract URI value from an HLS tag line like `#EXT-X-KEY:METHOD=AES-128,URI="value"`
/// Handles double quotes, single quotes, and unquoted values.
/// Returns `(full_line_replacement, extracted_uri)` if a URI is found.
fn rewrite_hls_tag_uri(
    line: &str,
    proxy_base: &str,
    secret: &str,
    base_url: &Url,
    headers: &Option<HashMap<String, String>>,
) -> Option<String> {
    let uri_pos = line.find("URI=")?;
    let prefix = &line[..uri_pos + 4];
    let rest = &line[uri_pos + 4..];
    if rest.is_empty() {
        return None;
    }

    let first = rest.chars().next()?;
    let (value, suffix) = if first == '"' || first == '\'' {
        let end = rest[1..].find(first)?;
        (&rest[1..1 + end], &rest[1 + end + 1..])
    } else {
        let end = rest.find(',').unwrap_or(rest.len());
        (&rest[..end], &rest[end..])
    };

    let abs_url = resolve_segment_url(base_url, value);
    let abs_url = unwrap_aisports_worker_url(&abs_url);
    let is_hls = looks_like_hls_playlist_url(&abs_url);
    let route = if is_hls { "m3u8-proxy" } else { "ts-proxy" };

    let proxy_url = if is_whitelisted_raw_url(&abs_url) && headers.is_none() {
        let encoded_url = urlencoding::encode(&abs_url);
        let header_part = if let Some(ref h) = headers {
            format!(
                "&headers={}",
                urlencoding::encode(&serde_json::to_string(h).unwrap_or_default())
            )
        } else {
            String::new()
        };
        format!(
            "{}/{}?url={}&mode=url{}",
            proxy_base, route, encoded_url, header_part
        )
    } else {
        let token = security::encrypt_url(&abs_url, secret, headers.clone());
        format!("{}/{}?h_token={}", proxy_base, route, token)
    };

    Some(format!("{}\"{}\"{}", prefix, proxy_url, suffix))
}

fn unwrap_aisports_worker_url(input: &str) -> String {
    let Ok(worker_url) = Url::parse(input) else {
        return input.to_string();
    };

    let host = worker_url.host_str().unwrap_or("").to_lowercase();
    if !host.contains("hue-universal-proxy") {
        return input.to_string();
    }

    let mut target = None;
    let mut sign = None;
    let mut t = None;
    for (key, value) in worker_url.query_pairs() {
        match key.as_ref() {
            "target" => target = Some(value.into_owned()),
            "sign" => sign = Some(value.into_owned()),
            "t" => t = Some(value.into_owned()),
            _ => {}
        }
    }

    let Some(target) = target else {
        return input.to_string();
    };

    let Ok(mut target_url) = Url::parse(&target) else {
        return input.to_string();
    };

    let target_host = target_url.host_str().unwrap_or("").to_lowercase();
    if !target_host.contains("live-pull.aisports.mobi") {
        return input.to_string();
    }

    {
        let existing: std::collections::HashSet<String> = target_url
            .query_pairs()
            .map(|(key, _)| key.into_owned())
            .collect();
        let mut pairs = target_url.query_pairs_mut();
        if let Some(sign) = sign {
            if !existing.contains("sign") {
                pairs.append_pair("sign", &sign);
            }
        }
        if let Some(t) = t {
            if !existing.contains("t") {
                pairs.append_pair("t", &t);
            }
        }
    }

    target_url.to_string()
}

fn parse_hls_byte_range(range_spec: &str, next_byte_offset: &mut Option<u64>) -> Option<String> {
    let spec = range_spec.trim();
    let (length_part, offset_part) = spec.split_once('@').unwrap_or((spec, ""));
    let length = length_part.trim().parse::<u64>().ok()?;
    let start = if offset_part.trim().is_empty() {
        (*next_byte_offset)?
    } else {
        offset_part.trim().parse::<u64>().ok()?
    };
    let end = start.checked_add(length)?.checked_sub(1)?;
    *next_byte_offset = end.checked_add(1);
    Some(format!("bytes={}-{}", start, end))
}

/// Resolve a segment URL relative to the base playlist URL, inheriting query params for signed URLs.
fn resolve_segment_url(base: &Url, relative: &str) -> String {
    let mut resolved = match base.join(relative) {
        Ok(u) => u,
        Err(_) => {
            // Fallback for absolute-path-style URLs
            if relative.starts_with('/') {
                let origin = base.origin().unicode_serialization();
                match Url::parse(&format!("{}{}", origin, relative)) {
                    Ok(u) => u,
                    Err(_) => return relative.to_string(),
                }
            } else {
                return relative.to_string();
            }
        }
    };

    // Inherit query parameters from parent playlist URL (crucial for signed HLS like VixSrc)
    for (key, value) in base.query_pairs() {
        if !resolved.query_pairs().any(|(k, _)| k == key) {
            resolved.query_pairs_mut().append_pair(&key, &value);
        }
    }

    resolved.to_string()
}

// ── Content-Type Inference ─────────────────────────────────────────────────────

fn infer_content_type(upstream_ct: &str, target_url: &str) -> &'static str {
    let bad = upstream_ct.is_empty()
        || upstream_ct.contains("octet-stream")
        || upstream_ct.contains("zip")
        || upstream_ct.starts_with("image/");
    if !bad {
        // Return a static str, but upstream_ct is dynamic…
        // For well-known types we can return static strings.
        if upstream_ct.contains("mpegurl") || upstream_ct.contains("x-mpegurl") {
            return "application/vnd.apple.mpegurl";
        }
        if upstream_ct.contains("mp2t") {
            return "video/mp2t";
        }
        if upstream_ct.contains("mp4") {
            return "video/mp4";
        }
        if upstream_ct.contains("webvtt") || upstream_ct.contains("vtt") {
            return "text/vtt";
        }
        if upstream_ct.contains("json") {
            return "application/json";
        }
    }

    let url_lower = target_url.to_lowercase();
    if url_lower.contains(".mkv") {
        return "video/x-matroska";
    }
    if url_lower.contains(".mp4") {
        return "video/mp4";
    }
    if url_lower.contains(".m3u8") {
        return "application/vnd.apple.mpegurl";
    }
    if url_lower.contains(".ts") {
        return "video/mp2t";
    }
    if url_lower.contains(".vtt") || url_lower.contains(".srt") {
        return "text/vtt";
    }
    "application/octet-stream"
}

// ── SRT to WebVTT ──────────────────────────────────────────────────────────────

fn convert_srt_to_vtt(srt: &str) -> String {
    let mut out = String::from("WEBVTT\n\n");
    let mut cue_num = 0;

    for block in srt.split("\n\n") {
        let lines: Vec<&str> = block.lines().collect();
        if lines.len() < 2 {
            continue;
        }
        // First line may be cue number; find the timing line
        let (timing_line, text_lines) = if lines[0].trim().parse::<u32>().is_ok() {
            (&lines[1], &lines[2..])
        } else {
            (&lines[0], &lines[1..])
        };

        if let Some((start, end)) = parse_srt_timing(timing_line) {
            cue_num += 1;
            out.push_str(&format!("{}\n{} --> {}\n", cue_num, start, end));
            for line in text_lines {
                out.push_str(line);
                out.push('\n');
            }
            out.push('\n');
        }
    }

    normalize_vtt_timestamps(&out)
}

fn normalize_vtt_timestamps(vtt: &str) -> String {
    vtt.lines()
        .map(|line| {
            if line.contains("-->") {
                line.replace(',', ".")
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn parse_srt_timing(line: &str) -> Option<(String, String)> {
    let line = line.trim();
    let arrow = line.find(" --> ")?;
    let start = &line[..arrow];
    let end = &line[arrow + 5..];
    Some((srt_time_to_vtt(start.trim()), srt_time_to_vtt(end.trim())))
}

fn srt_time_to_vtt(time: &str) -> String {
    // SRT: 00:00:00,000   -> WebVTT: 00:00:00.000
    time.replace(',', ".")
}

// ── Generic Proxy ────────────────────────────────────────────────────────────

/// Generic proxy handler for iframe and other non-media content.
/// Decrypts h_token and streams the upstream response back to the client.
pub async fn generic_proxy_handler(
    State(state): State<AppState>,
    Query(params): Query<ProxyQuery>,
    request_headers: HeaderMap,
) -> Response {
    // CORS preflight
    if request_headers
        .get("access-control-request-method")
        .is_some()
        || request_headers
            .get("access-control-request-headers")
            .is_some()
    {
        let mut builder = Response::builder().status(StatusCode::NO_CONTENT);
        builder = builder.header("Access-Control-Allow-Origin", "*");
        builder = builder.header("Access-Control-Allow-Methods", "GET, HEAD, OPTIONS");
        builder = builder.header("Access-Control-Allow-Headers", "*");
        builder = builder.header("Access-Control-Max-Age", "86400");
        return builder.body(Body::empty()).unwrap();
    }

    let (target_url, token_headers) = match resolve_proxy_target(&params, &state.secret) {
        Some(t) => t,
        None => return (StatusCode::FORBIDDEN, "Invalid or missing signed token").into_response(),
    };

    if is_internal_host(&target_url) {
        return (StatusCode::FORBIDDEN, "Blocked internal URL").into_response();
    }

    let mut req = state.raw_client.get(&worker_routed_url(
        &target_url,
        &state.config.cf_worker_url,
        &state.config.worker_upstream_hosts,
    ));
    req = inject_cdn_headers(req, &target_url, &token_headers);

    // Forward Range header from client (critical for mobile video downloads)
    if let Some(range) = request_headers.get("range").and_then(|v| v.to_str().ok()) {
        req = req.header("Range", range);
    }

    let mut referer_value = None;
    let mut has_origin = false;
    if let Some(ref h) = token_headers {
        for (k, v) in h.iter() {
            if k.eq_ignore_ascii_case("origin") {
                has_origin = true;
            }
            if k.eq_ignore_ascii_case("referer") {
                referer_value = Some(v.clone());
            }
            req = req.header(k, v);
        }
    }

    if !has_origin {
        if let Some(ref ref_url) = referer_value {
            if let Ok(origin_url) = Url::parse(ref_url) {
                let mut origin = origin_url.origin().unicode_serialization();
                if origin.ends_with('/') {
                    origin.pop();
                }
                req = req.header("Origin", origin);
            }
        } else if let Ok(target_u) = Url::parse(&target_url) {
            let mut origin = target_u.origin().unicode_serialization();
            if origin.ends_with('/') {
                origin.pop();
            }
            req = req.header("Origin", origin);
        }
    }

    match req.send().await {
        Ok(resp) => {
            let status =
                StatusCode::from_u16(resp.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
            let mut builder = Response::builder().status(status);

            // Forward content-type and cache headers
            for (k, v) in resp.headers() {
                let key = k.as_str();
                if key.eq_ignore_ascii_case("content-type")
                    || key.eq_ignore_ascii_case("content-length")
                    || key.eq_ignore_ascii_case("content-range")
                    || key.eq_ignore_ascii_case("cache-control")
                    || key.eq_ignore_ascii_case("etag")
                    || key.eq_ignore_ascii_case("last-modified")
                    || key.eq_ignore_ascii_case("accept-ranges")
                {
                    builder = builder.header(k, v);
                }
            }

            builder = builder.header("Access-Control-Allow-Origin", "*");

            // Download mode: buffer the full body so the browser gets a known
            // Content-Length and can show progress. Stream everything else.
            if params.download.is_some() {
                let filename = params.filename.as_deref().unwrap_or("video.mp4");
                builder = builder.header(
                    "Content-Disposition",
                    format!(r#"attachment; filename="{}""#, filename),
                );
                match resp.bytes().await {
                    Ok(bytes) => {
                        builder = builder.header("content-length", bytes.len());
                        builder.body(Body::from(bytes)).unwrap()
                    }
                    Err(e) => {
                        tracing::warn!("[proxy] Download body read failed: {}", e);
                        (StatusCode::BAD_GATEWAY, "Upstream body unreadable").into_response()
                    }
                }
            } else {
                builder.body(Body::from_stream(resp.bytes_stream())).unwrap()
            }
        }
        Err(e) => {
            tracing::warn!("[proxy] Request failed: {}", e);
            (StatusCode::BAD_GATEWAY, "Upstream request failed").into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_srt_time_to_vtt() {
        assert_eq!(srt_time_to_vtt("00:00:01,500"), "00:00:01.500");
        assert_eq!(srt_time_to_vtt("01:23:45,000"), "01:23:45.000");
    }

    #[test]
    fn test_parse_srt_timing() {
        let line = "00:00:01,500 --> 00:00:04,000";
        let (start, end) = parse_srt_timing(line).unwrap();
        assert_eq!(start, "00:00:01.500");
        assert_eq!(end, "00:00:04.000");
    }

    #[test]
    fn test_convert_srt_to_vtt() {
        let srt = r#"1
00:00:01,500 --> 00:00:04,000
Hello world

2
00:00:05,000 --> 00:00:07,000
Second cue"#;
        let vtt = convert_srt_to_vtt(srt);
        assert!(vtt.starts_with("WEBVTT\n\n"));
        assert!(vtt.contains("1\n00:00:01.500 --> 00:00:04.000\nHello world\n"));
        assert!(vtt.contains("2\n00:00:05.000 --> 00:00:07.000\nSecond cue\n"));
    }

    #[test]
    fn test_normalize_vtt_timestamps() {
        let vtt = "WEBVTT\n\n1\n00:00:01,500 --> 00:00:04,000\nHello\n";
        let normalized = normalize_vtt_timestamps(vtt);
        assert!(normalized.contains("00:00:01.500 --> 00:00:04.000"));
    }

    #[test]
    fn test_dynamic_proxy_base_prefers_public_configured_origin() {
        let mut headers = HeaderMap::new();
        headers.insert("host", "www.vexli.online".parse().unwrap());
        headers.insert("x-forwarded-proto", "https".parse().unwrap());

        let base = dynamic_proxy_base(&headers, "https://api.vexli.online");

        assert_eq!(base, "https://api.vexli.online");
    }

    #[test]
    fn test_dynamic_proxy_base_preserves_configured_public_port() {
        let mut headers = HeaderMap::new();
        headers.insert("host", "www.vexli.online".parse().unwrap());

        let base = dynamic_proxy_base(&headers, "https://api.vexli.online:24609");

        assert_eq!(base, "https://api.vexli.online:24609");
    }

    #[test]
    fn test_dynamic_proxy_base_uses_headers_for_local_fallback() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-host", "dev.example.test".parse().unwrap());
        headers.insert("x-forwarded-proto", "https".parse().unwrap());

        let base = dynamic_proxy_base(&headers, "http://localhost:24609");

        assert_eq!(base, "https://dev.example.test");
    }

    #[tokio::test]
    async fn test_rewrite_playlist_token_mode() {
        let content = "#EXTM3U\n#EXTINF:10,\nsegment.ts\n#EXT-X-KEY:METHOD=AES-128,URI=\"key.bin\"";
        let secret = "test-secret";
        let rewritten = rewrite_playlist(
            content,
            "https://example.com/playlist.m3u8",
            "http://proxy.local",
            secret,
            None,
        );
        assert!(rewritten.contains("http://proxy.local/ts-proxy?h_token="));
        assert!(rewritten.contains("http://proxy.local/ts-proxy?h_token="));
    }

    #[tokio::test]
    async fn test_rewrite_playlist_never_emits_raw_url_mode() {
        let content = "#EXTM3U\nsegment.ts";
        let rewritten = rewrite_playlist(
            content,
            "https://example.com/playlist.m3u8",
            "http://proxy.local",
            "secret",
            None,
        );
        assert!(rewritten.contains("http://proxy.local/ts-proxy?h_token="));
        assert!(!rewritten.contains("http://proxy.local/ts-proxy?url="));
        assert!(!rewritten.contains("&mode=url"));
    }

    #[tokio::test]
    async fn test_rewrite_playlist_uses_signed_urls_when_headers_are_present() {
        let content = "#EXTM3U\n#EXTINF:10,\nsegment.ts";
        let mut headers = HashMap::new();
        headers.insert(
            "Referer".to_string(),
            "https://thesports.today/".to_string(),
        );

        let rewritten = rewrite_playlist(
            content,
            "https://live-pull.aisports.mobi/moviebox/device01/playlist.m3u8",
            "http://proxy.local",
            "secret",
            Some(headers),
        );

        assert!(rewritten.contains("http://proxy.local/ts-proxy?h_token="));
        assert!(!rewritten.contains("http://proxy.local/ts-proxy?url="));
        assert!(!rewritten.contains("&headers="));
    }

    #[test]
    fn test_unwrap_aisports_worker_url_moves_segment_signature_to_target() {
        let input = "https://hue-universal-proxy.testpjmail.workers.dev/?target=https%3A%2F%2Flive-pull.aisports.mobi%2Fmoviebox%2Fdevice01%2Fsegment.ts&referer=https%3A%2F%2Flive-pull.aisports.mobi&origin=https%3A%2F%2Flive-pull.aisports.mobi&sign=abc&t=123";

        let unwrapped = unwrap_aisports_worker_url(input);

        assert_eq!(
            unwrapped,
            "https://live-pull.aisports.mobi/moviebox/device01/segment.ts?sign=abc&t=123"
        );
    }

    #[test]
    fn test_proxy_target_rejects_non_whitelisted_raw_url() {
        let params = ProxyQuery {
            h_token: None,
            url: Some("https://example.com/playlist.m3u8".to_string()),
            headers: None,
            mode: Some("url".to_string()),
            download: None,
            filename: None,
            adsurgery: None,
        };

        assert!(resolve_proxy_target(&params, "secret").is_none());
    }

    #[test]
    fn test_proxy_target_accepts_whitelisted_vixsrc_raw_url() {
        let params = ProxyQuery {
            h_token: None,
            url: Some("https://vixsrc.to/playlist/123.m3u8".to_string()),
            headers: Some(r#"{"Referer":"https://vixsrc.to/"}"#.to_string()),
            mode: Some("url".to_string()),
            download: None,
            filename: None,
            adsurgery: None,
        };

        let result = resolve_proxy_target(&params, "secret");
        assert!(result.is_some());
        let (url, headers) = result.unwrap();
        assert_eq!(url, "https://vixsrc.to/playlist/123.m3u8");
        assert!(headers.is_some());
    }

    #[test]
    fn test_ts_target_rejects_non_whitelisted_raw_url() {
        let params = TsProxyQuery {
            h_token: None,
            url: Some("https://example.com/segment.ts".to_string()),
            headers: None,
            mode: Some("url".to_string()),
            range: None,
            clamp_open: None,
            progressive_open: None,
            no_synth: None,
            force_200: None,
            tail_prefetch: None,
            sanitize: None,
            download: None,
            filename: None,
        };

        assert!(resolve_ts_target(&params, "secret").is_none());
    }

    #[test]
    fn test_ts_query_accepts_camel_case_playback_flags() {
        let params: TsProxyQuery = serde_json::from_value(serde_json::json!({
            "noSynth": 1,
            "force200": 1,
            "clampOpen": 0,
            "progressiveOpen": 0,
            "tailPrefetch": 0
        }))
        .unwrap();

        assert_eq!(params.no_synth, Some(1));
        assert_eq!(params.force_200, Some(1));
        assert_eq!(params.clamp_open, Some(0));
        assert_eq!(params.progressive_open, Some(0));
        assert_eq!(params.tail_prefetch, Some(0));
    }

    #[test]
    fn test_ts_target_accepts_whitelisted_moviebox_cdn_raw_url() {
        let params = TsProxyQuery {
            h_token: None,
            url: Some("https://hakunaymatata.com/seg.ts".to_string()),
            headers: None,
            mode: Some("url".to_string()),
            range: None,
            clamp_open: None,
            progressive_open: None,
            no_synth: None,
            force_200: None,
            tail_prefetch: None,
            sanitize: None,
            download: None,
            filename: None,
        };

        assert!(resolve_ts_target(&params, "secret").is_some());
    }

    #[tokio::test]
    async fn test_rewrite_playlist_preserves_explicit_byte_ranges() {
        let content = "#EXTM3U\n#EXTINF:1.0,\n#EXT-X-BYTERANGE:759708@5590180\nindex0.ts";
        let rewritten = rewrite_playlist(
            content,
            "https://cdn.example.com/show/playlist.m3u8",
            "http://proxy.local",
            "secret",
            None,
        );

        assert!(rewritten.contains("range=bytes%3D5590180-6349887"));
    }

    #[test]
    fn test_infer_content_type() {
        assert_eq!(
            infer_content_type("application/vnd.apple.mpegurl", "foo.m3u8"),
            "application/vnd.apple.mpegurl"
        );
        assert_eq!(infer_content_type("", "foo.ts"), "video/mp2t");
        assert_eq!(infer_content_type("", "foo.mp4"), "video/mp4");
        assert_eq!(
            infer_content_type("application/octet-stream", "foo.mkv"),
            "video/x-matroska"
        );
    }
}
