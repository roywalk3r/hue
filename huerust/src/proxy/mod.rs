//! Hue Stream API
//! Professional Streaming Aggregator
//!
//! @author roywalk3r <roywalker440@gmail.com>
//! @repository https://github.com/roywalk3r/hue.git
//! @license MIT

use axum::{
    body::Body,
    extract::{Query, State},
    http::{StatusCode, header::CONTENT_TYPE},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use std::collections::HashMap;
use url::Url;

use crate::{security, app::AppState};
use std::env;

pub mod sanitize;
pub mod adsurgery;
pub mod wrap;

// ── Query Types ──────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct ProxyQuery {
    pub h_token: Option<String>,
    pub url: Option<String>,
    pub headers: Option<String>, // JSON-encoded headers for url mode
    pub mode: Option<String>, // "url" for url+headers mode
    pub download: Option<String>,
    pub filename: Option<String>,
    pub adsurgery: Option<u32>,
}

#[derive(Deserialize)]
pub struct TsProxyQuery {
    pub h_token: Option<String>,
    pub url: Option<String>,
    pub range: Option<String>,
    #[allow(dead_code)]
    pub clamp_open: Option<u32>,
    #[allow(dead_code)]
    pub progressive_open: Option<u32>,
    pub no_synth: Option<u32>,
    pub force_200: Option<u32>,
    #[allow(dead_code)]
    pub tail_prefetch: Option<u32>,
    pub sanitize: Option<u32>,
}

// ── Shared helpers ───────────────────────────────────────────────────────────

/// Extract target URL + headers from either token or raw url mode
fn resolve_proxy_target(
    params: &ProxyQuery,
    secret: &str,
) -> Option<(String, Option<HashMap<String, String>>)> {
    // Prefer h_token mode
    if let Some(token) = &params.h_token {
        if let Some(data) = security::decrypt_url(token, secret) {
            return Some((data.url, data.headers));
        }
    }
    // Fallback to url mode
    if let Some(url) = &params.url {
        let headers = params.headers.as_ref().and_then(|h| {
            serde_json::from_str::<HashMap<String, String>>(h).ok()
        });
        return Some((url.clone(), headers));
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
        return Some((url.clone(), None));
    }
    None
}

/// Inject CDN-specific headers based on target host
fn inject_cdn_headers(mut req: reqwest::RequestBuilder, target_url: &str, token_headers: &Option<HashMap<String, String>>) -> reqwest::RequestBuilder {
    let host = match Url::parse(target_url) {
        Ok(u) => u.host_str().unwrap_or("").to_string(),
        Err(_) => return req,
    };

    tracing::info!("[inject_cdn_headers] Host: {}, has_referer: {}", host, has_header(token_headers, "referer"));

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
                if origin.ends_with('/') { origin.pop(); }
                tracing::info!("[inject_cdn_headers] Added derived Origin: {}", origin);
                req = req.header("Origin", origin);
            }
        }
    } else {
        // We have a Referer from token, but maybe no Origin. Match its logic.
        if let Some(h) = token_headers {
            tracing::info!("[inject_cdn_headers] Has Referer, checking for Origin. Keys: {:?}", h.keys().collect::<Vec<_>>());
            if !has_header(token_headers, "origin") {
                if let Some(r) = h.get("Referer").or_else(|| h.get("referer")) {
                    if let Ok(origin_url) = Url::parse(r) {
                        let mut origin = origin_url.origin().unicode_serialization();
                        if origin.ends_with('/') { origin.pop(); }
                        tracing::info!("[inject_cdn_headers] Derived Origin from Referer: {} -> {}", r, origin);
                        req = req.header("Origin", origin);
                    } else {
                        tracing::warn!("[inject_cdn_headers] Failed to parse Origin from Referer: {}", r);
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

    req.header("Accept", "*/*")
        .header("Sec-Fetch-User", "?1")
}

fn has_header(headers: &Option<HashMap<String, String>>, name: &str) -> bool {
    headers
        .as_ref()
        .map(|h| h.keys().any(|k| k.eq_ignore_ascii_case(name)))
        .unwrap_or(false)
}

/// Build &headers= JSON query fragment for url mode
fn build_headers_query(headers: &Option<HashMap<String, String>>) -> String {
    let h = match headers {
        Some(h) => h,
        None => return String::new(),
    };
    let skip: &[&str] = &["host", "connection", "content-length", "accept-encoding"];
    let filtered: HashMap<String, String> = h
        .iter()
        .filter(|(k, _)| !skip.contains(&k.to_lowercase().as_str()))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    if filtered.is_empty() {
        return String::new();
    }
    format!("&headers={}", urlencoding::encode(&serde_json::to_string(&filtered).unwrap_or_default()))
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

// ── M3U8 Proxy ───────────────────────────────────────────────────────────────

pub async fn m3u8_handler(
    State(state): State<AppState>,
    Query(params): Query<ProxyQuery>,
) -> Response {
    let (target_url, headers) = match resolve_proxy_target(&params, &state.secret) {
        Some(t) => t,
        None => return (StatusCode::FORBIDDEN, "Invalid or missing token/url").into_response(),
    };

    if is_internal_host(&target_url) {
        return (StatusCode::FORBIDDEN, "Blocked internal URL").into_response();
    }

    let is_vix = is_vixsrc_target(&target_url, &headers);
    let use_url_mode = params.mode.as_deref() == Some("url") || is_vix;

    let mut req = state.client.get(&target_url);
    req = inject_cdn_headers(req, &target_url, &headers);

    // Inject headers from token (overrides defaults)
    let mut referer_value = None;
    let mut has_origin = false;
    if let Some(h) = headers.clone() {
        for (k, v) in h {
            if k.eq_ignore_ascii_case("origin") { has_origin = true; }
            if k.eq_ignore_ascii_case("referer") { referer_value = Some(v.clone()); }
            req = req.header(k, v);
        }
    }

    // Vixsrc Special: If it's Vixsrc, we MUST ensure the Referer is correct
    if is_vix && referer_value.is_none() {
        req = req.header("Referer", "https://vixsrc.to/");
        referer_value = Some("https://vixsrc.to/".to_string());
    }

    if !has_origin {
        if let Some(ref_url) = referer_value {
            if let Ok(origin_url) = Url::parse(&ref_url) {
                let mut origin = origin_url.origin().unicode_serialization();
                if origin.ends_with('/') { origin.pop(); }
                req = req.header("Origin", origin);
            }
        } else {
            // Fallback: derive Origin from the target host itself if Referer is missing
            if let Ok(target_u) = Url::parse(&target_url) {
                let mut origin = target_u.origin().unicode_serialization();
                if origin.ends_with('/') { origin.pop(); }
                req = req.header("Origin", origin);
            }
        }
    }

    match req.send().await {
        Ok(resp) => {
            let status = resp.status();
            let headers_ref = resp.headers().clone();
            let ct = headers_ref
                .get("content-type")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("");

            let body = match resp.text().await {
                Ok(b) => b,
                Err(_) => return (StatusCode::BAD_GATEWAY, "Upstream body unreadable").into_response(),
            };

            let is_hls = ct.contains("mpegurl")
                || ct.contains("x-mpegurl")
                || ct.contains("vnd.apple.mpegurl")
                || body.trim().starts_with("#EXTM3U");

            let mut body = body;

            // Ad-surgery if requested or globally enabled
            let adsurgery_enabled = params.adsurgery.unwrap_or(0) == 1
                || env::var("SANITIZE_STREAMS").map_or(false, |v| v == "true" || v == "1");

            if is_hls && adsurgery_enabled {
                let (cleaned, removed) = adsurgery::sanitize_manifest(&body);
                if removed > 0 {
                    tracing::debug!("[m3u8-proxy] Ad-surgery removed {} segments from {}", removed, target_url);
                }
                body = cleaned;
            }

            let final_body = if is_hls {
                rewrite_playlist(&body, &target_url, &state.proxy_base_url, &state.secret, headers, use_url_mode)
            } else {
                body
            };

            let mut builder = Response::builder().status(StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::OK));
            builder = builder.header("Access-Control-Allow-Origin", "*");

            // Download mode: force attachment disposition
            if params.download.is_some() {
                let filename = params.filename.as_deref().unwrap_or("playlist.m3u8");
                builder = builder.header(
                    "Content-Disposition",
                    format!(r#"attachment; filename="{}""#, filename),
                );
            }

            builder = builder.header(CONTENT_TYPE, infer_content_type(ct, &target_url));
            builder.body(Body::from(final_body)).unwrap()
        }
        Err(e) => (StatusCode::BAD_GATEWAY, e.to_string()).into_response(),
    }
}

// ── TS / Segment Proxy ───────────────────────────────────────────────────────

/// Block localhost/internal IPs for security
fn is_internal_host(target_url: &str) -> bool {
    if let Ok(u) = Url::parse(target_url) {
        if let Some(host) = u.host_str() {
            let h = host.to_lowercase();
            return h == "localhost"
                || h == "127.0.0.1"
                || h == "0.0.0.0"
                || h.starts_with("192.168.")
                || h.starts_with("10.");
        }
    }
    false
}

pub async fn ts_handler(
    State(state): State<AppState>,
    Query(params): Query<TsProxyQuery>,
) -> Response {
    let (target_url, headers) = match resolve_ts_target(&params, &state.secret) {
        Some(t) => t,
        None => {
            return (StatusCode::FORBIDDEN, "Invalid or missing token/url").into_response();
        }
    };

    if is_internal_host(&target_url) {
        return (StatusCode::FORBIDDEN, "Blocked internal URL").into_response();
    }

    let force_200 = params.force_200.unwrap_or(0) == 1;
    let _no_synth = params.no_synth.unwrap_or(0) == 1;
    let sanitize = params.sanitize.unwrap_or(0) == 1;

    let is_vix = is_vixsrc_target(&target_url, &headers);

    tracing::info!("[ts-proxy] Target: {}", target_url);
    tracing::info!("[ts-proxy] Token headers: {:?}", headers);

    let mut req = state.client.get(&target_url);
    req = inject_cdn_headers(req, &target_url, &headers);

    let mut has_origin = false;
    let mut referer_value = None;
    if let Some(ref h) = headers {
        for (k, v) in h {
            req = req.header(k, v);
            if k.eq_ignore_ascii_case("origin") { has_origin = true; }
            if k.eq_ignore_ascii_case("referer") { referer_value = Some(v.clone()); }
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
                if origin.ends_with('/') { origin.pop(); }
                req = req.header("Origin", origin);
            }
        } else {
            if let Ok(target_u) = Url::parse(&target_url) {
                let mut origin = target_u.origin().unicode_serialization();
                if origin.ends_with('/') { origin.pop(); }
                req = req.header("Origin", origin);
            }
        }
    }

    if let Some(range) = &params.range {
        req = req.header("Range", range);
    }

    match req.send().await {
        Ok(resp) => {
            let status = resp.status();

            // Handle redirects manually — block localhost destinations
            if status.is_redirection() {
                if let Some(loc) = resp.headers().get("location") {
                    if let Ok(loc_str) = loc.to_str() {
                        if is_internal_host(loc_str) {
                            tracing::error!("[ts-proxy] Blocked redirect to localhost: {} from {}", loc_str, target_url);
                            return (StatusCode::FORBIDDEN, "Blocked redirect to internal network").into_response();
                        }
                        // Follow redirect (one level)
                        let follow = state.client.get(loc_str);
                        if let Ok(resp2) = follow.send().await {
                            let status2 = resp2.status();
                            let mut builder = Response::builder().status(StatusCode::from_u16(status2.as_u16()).unwrap_or(StatusCode::OK));
                            if let Some(ct) = resp2.headers().get("content-type") {
                                if let Ok(ct_str) = ct.to_str() {
                                    builder = builder.header("content-type", infer_content_type(ct_str, &target_url));
                                }
                            }
                            builder = builder.header("Access-Control-Allow-Origin", "*");
                            return builder.body(Body::from_stream(resp2.bytes_stream())).unwrap();
                        }
                    }
                }
            }

            let mut builder = Response::builder();
            let mapped = StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::OK);
            builder = builder.status(mapped);

            if let Some(ct) = resp.headers().get("content-type") {
                if let Ok(ct_str) = ct.to_str() {
                    builder = builder.header("content-type", infer_content_type(ct_str, &target_url));
                }
            }

            builder = builder.header("Access-Control-Allow-Origin", "*");

            if force_200 && mapped == StatusCode::PARTIAL_CONTENT {
                builder = builder.status(StatusCode::OK);
            }

            if sanitize {
                match resp.bytes().await {
                    Ok(body) => {
                        let bytes = body.to_vec();
                        if sanitize::is_valid_ts(&bytes) {
                            let (cleaned, stripped) = sanitize::sanitize_ts_segment(&bytes);
                            if stripped > 0 {
                                tracing::info!("[ts-proxy] Sanitized {} packets from {}", stripped, target_url);
                            }
                            builder = builder.header("content-length", cleaned.len());
                            return builder.body(Body::from(cleaned)).unwrap();
                        }
                        builder = builder.header("content-length", bytes.len());
                        builder.body(Body::from(bytes)).unwrap()
                    }
                    Err(e) => (StatusCode::BAD_GATEWAY, e.to_string()).into_response(),
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
                if resp.headers().contains_key("accept-ranges") {
                    builder = builder.header("accept-ranges", "bytes");
                }
                builder.body(Body::from_stream(resp.bytes_stream())).unwrap()
            }
        }
        Err(e) => {
            // Retry once without Range header if the error might be range-related
            if params.range.is_some() {
                tracing::warn!("[ts-proxy] Initial request failed ({}), retrying without Range", e);
                let mut retry = state.client.get(&target_url);
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
                                builder = builder.header("content-type", infer_content_type(ct_str, &target_url));
                            }
                        }
                        builder = builder.header("Access-Control-Allow-Origin", "*");
                        if let Some(cl) = resp.headers().get("content-length") {
                            if let Ok(v) = cl.to_str() { builder = builder.header("content-length", v); }
                        }
                        if let Some(cr) = resp.headers().get("content-range") {
                            if let Ok(v) = cr.to_str() { builder = builder.header("content-range", v); }
                        }
                        return builder.body(Body::from_stream(resp.bytes_stream())).unwrap();
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
) -> Response {
    let (target_url, headers) = match resolve_proxy_target(&params, &state.secret) {
        Some(t) => t,
        None => return (StatusCode::FORBIDDEN, "Invalid or missing token/url").into_response(),
    };

    let mut req = state.client.get(&target_url);
    req = inject_cdn_headers(req, &target_url, &headers);

    // Inject headers from token (overrides defaults)
    let mut referer_value = None;
    let mut has_origin = false;
    if let Some(h) = headers {
        for (k, v) in h {
            if k.eq_ignore_ascii_case("origin") { has_origin = true; }
            if k.eq_ignore_ascii_case("referer") { referer_value = Some(v.clone()); }
            req = req.header(k, v);
        }
    }
    if !has_origin {
        if let Some(ref_url) = referer_value {
            if let Ok(origin_url) = Url::parse(&ref_url) {
                let mut origin = origin_url.origin().unicode_serialization();
                if origin.ends_with('/') { origin.pop(); }
                req = req.header("Origin", origin);
            }
        } else {
            // Fallback: derive Origin from the target host itself if Referer is missing
            if let Ok(target_u) = Url::parse(&target_url) {
                let mut origin = target_u.origin().unicode_serialization();
                if origin.ends_with('/') { origin.pop(); }
                req = req.header("Origin", origin);
            }
        }
    }

    match req.send().await {
        Ok(resp) => {
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
                .header("content-type", "text/vtt")
                .header("Access-Control-Allow-Origin", "*")
                .body(Body::from(final_body))
                .unwrap()
        }
        Err(e) => (StatusCode::BAD_GATEWAY, e.to_string()).into_response(),
    }
}

// ── Playlist Rewriting ─────────────────────────────────────────────────────────

fn rewrite_playlist(
    content: &str,
    target_url: &str,
    proxy_base: &str,
    secret: &str,
    headers: Option<HashMap<String, String>>,
    use_url_mode: bool,
) -> String {
    let base_url = match Url::parse(target_url) {
        Ok(u) => u,
        Err(_) => return content.to_string(),
    };

    let header_part = if use_url_mode {
        build_headers_query(&headers)
    } else {
        String::new()
    };
    let mode_part = if use_url_mode { "&mode=url" } else { "" };

    // Collect segment URLs for prefetch after rewrite
    let mut segment_urls: Vec<String> = Vec::new();

    let rewritten = content
        .lines()
        .map(|line| {
            // Tags that carry a URI attribute
            if line.starts_with("#") {
                if line.contains("URI=\"") {
                    let parts: Vec<&str> = line.split("URI=\"").collect();
                    if parts.len() > 1 {
                        let uri_parts: Vec<&str> = parts[1].split('"').collect();
                        let relative_uri = uri_parts[0];
                        let abs_url = resolve_segment_url(&base_url, relative_uri);

                        let is_hls = abs_url.to_lowercase().contains(".m3u8");
                        let proxy_url = if use_url_mode {
                            let route = if is_hls { "m3u8-proxy" } else { "ts-proxy" };
                            format!(
                                "{}/{}?url={}{}{}&clampOpen=0&progressiveOpen=0&noSynth=1&force200=1&tailPrefetch=0",
                                proxy_base,
                                route,
                                urlencoding::encode(&abs_url),
                                header_part,
                                mode_part
                            )
                        } else {
                            let token = security::encrypt_url(&abs_url, secret, headers.clone());
                            let route = if is_hls { "m3u8-proxy" } else { "ts-proxy" };
                            format!("{}/{}?h_token={}", proxy_base, route, token)
                        };
                        return line.replace(&format!("\"{}\"", relative_uri), &format!("\"{}\"", proxy_url));
                    }
                }
                line.to_string()
            } else if !line.trim().is_empty() {
                let line_trim = line.trim();
                let abs_url = resolve_segment_url(&base_url, line_trim);
                let is_hls = abs_url.to_lowercase().contains(".m3u8");
                let route = if is_hls { "m3u8-proxy" } else { "ts-proxy" };

                // MP3-wrapped-TS detection
                let is_mp3_wrapped_ts = abs_url.to_lowercase().contains(".mp3");
                let force_type_param = if is_mp3_wrapped_ts { "&forceType=video%2Fmp2t" } else { "" };

                // Track segment URLs for prefetch
                if !is_hls {
                    segment_urls.push(abs_url.clone());
                }

                if use_url_mode {
                    let segment_extra = if is_hls { "" } else { "&clampOpen=0&progressiveOpen=0&noSynth=1&tailPrefetch=0" };
                    format!(
                        "{}/{}?url={}{}{}{}{}",
                        proxy_base,
                        route,
                        urlencoding::encode(&abs_url),
                        header_part,
                        mode_part,
                        segment_extra,
                        force_type_param
                    )
                } else {
                    let token = security::encrypt_url(&abs_url, secret, headers.clone());
                    let segment_extra = if is_hls { "" } else { "&clampOpen=0&progressiveOpen=0&noSynth=1&tailPrefetch=0" };
                    format!("{}/{}?h_token={}{}{}", proxy_base, route, token, segment_extra, force_type_param)
                }
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    // RAM Turbo: prefetch first 5 segments asynchronously
    if !segment_urls.is_empty() {
        let client = reqwest::Client::new();
        let headers_clone = headers.clone();
        tokio::spawn(async move {
            for url in segment_urls.into_iter().take(5) {
                let mut req = client.get(&url);
                if let Some(ref h) = headers_clone {
                    for (k, v) in h {
                        req = req.header(k, v);
                    }
                }
                let _ = req.send().await;
            }
        });
    }

    rewritten
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

    #[tokio::test]
    async fn test_rewrite_playlist_token_mode() {
        let content = "#EXTM3U\n#EXTINF:10,\nsegment.ts\n#EXT-X-KEY:METHOD=AES-128,URI=\"key.bin\"";
        let secret = "test-secret";
        let rewritten = rewrite_playlist(content, "https://example.com/playlist.m3u8", "http://proxy.local", secret, None, false);
        assert!(rewritten.contains("http://proxy.local/ts-proxy?h_token="));
        assert!(rewritten.contains("http://proxy.local/ts-proxy?h_token="));
    }

    #[tokio::test]
    async fn test_rewrite_playlist_url_mode() {
        let content = "#EXTM3U\nsegment.ts";
        let rewritten = rewrite_playlist(content, "https://example.com/playlist.m3u8", "http://proxy.local", "secret", None, true);
        assert!(rewritten.contains("http://proxy.local/ts-proxy?url="));
        assert!(rewritten.contains("&mode=url"));
    }

    #[test]
    fn test_infer_content_type() {
        assert_eq!(infer_content_type("application/vnd.apple.mpegurl", "foo.m3u8"), "application/vnd.apple.mpegurl");
        assert_eq!(infer_content_type("", "foo.ts"), "video/mp2t");
        assert_eq!(infer_content_type("", "foo.mp4"), "video/mp4");
        assert_eq!(infer_content_type("application/octet-stream", "foo.mkv"), "video/x-matroska");
    }
}
