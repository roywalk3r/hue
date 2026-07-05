//! Hue Stream API
//! Professional Streaming Aggregator
//!
//! @author roywalk3r <roywalker440@gmail.com>
//! @repository https://github.com/roywalk3r/hue.git
//! @license MIT

use crate::models::{StreamResult, Subtitle};
use anyhow::Result;
use brotli::Decompressor;
use flate2::read::GzDecoder;
use regex::Regex;
use reqwest_middleware::ClientWithMiddleware;
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::io::Read;
use std::sync::LazyLock;
use tracing::{info, warn};
use zstd::stream::read::Decoder as ZstdDecoder;

const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/137.0.0.0 Safari/537.36";
const VIXSRC_REFERER: &str = "https://vixsrc.to/";

/// vixsrc.to returns compressed bodies without always setting Content-Encoding.
/// Detect gzip / zstd / brotli by magic and decode before regex scraping.
fn maybe_decompress_body(body: &[u8]) -> String {
    if body.is_empty() {
        return String::new();
    }
    // gzip magic: 0x1f 0x8b
    if body.len() >= 2 && body[0] == 0x1f && body[1] == 0x8b {
        let mut decoder = GzDecoder::new(body);
        let mut out = Vec::new();
        if decoder.read_to_end(&mut out).is_ok() {
            return String::from_utf8_lossy(&out).to_string();
        }
    }
    // zstd magic: 0x28 0xb5 0x2f 0xfd
    if body.len() >= 4 && body[0] == 0x28 && body[1] == 0xb5 && body[2] == 0x2f && body[3] == 0xfd {
        if let Ok(mut decoder) = ZstdDecoder::new(body) {
            let mut out = Vec::new();
            if decoder.read_to_end(&mut out).is_ok() && !out.is_empty() {
                return String::from_utf8_lossy(&out).to_string();
            }
        }
    }
    // brotli has no fixed magic; try decoding and fall back to lossy UTF-8.
    {
        let mut decoder = Decompressor::new(body, 4096);
        let mut out = Vec::new();
        if decoder.read_to_end(&mut out).is_ok() && !out.is_empty() {
            return String::from_utf8_lossy(&out).to_string();
        }
    }
    String::from_utf8_lossy(body).to_string()
}

/// Realistic browser headers so Cloudflare does not reject our API calls.
/// Parse ?rendition=1080p / 720p / 480p etc. from a vixsrc.to playlist URL.
fn extract_rendition_quality(url: &str) -> Option<String> {
    url.split('?')
        .nth(1)?
        .split('&')
        .find(|p| p.starts_with("rendition="))
        .and_then(|p| p.split_once('=').map(|(_, v)| v.to_string()))
}

fn vixsrc_browser_headers(referer: &str) -> Vec<(&'static str, String)> {
    vec![
        ("Accept", "application/json, text/plain, */*".to_string()),
        ("Accept-Language", "en-US,en;q=0.9".to_string()),
        ("Accept-Encoding", "gzip, deflate, br, zstd".to_string()),
        ("Referer", referer.to_string()),
        ("Origin", "https://vixsrc.to".to_string()),
        ("Sec-Fetch-Dest", "empty".to_string()),
        ("Sec-Fetch-Mode", "cors".to_string()),
        ("Sec-Fetch-Site", "same-origin".to_string()),
        ("User-Agent", USER_AGENT.to_string()),
    ]
}

// ── Cached regexes (compile once at first use) ──
static RE_MASTER_PL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"window\.masterPlaylist\s*=\s*(\{[\s\S]*?\n\s*\})\s*[;\n]"#).unwrap()
});
static RE_STREAMS_ARR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"window\.streams\s*=\s*(\[[\s\S]*?\]);"#).unwrap());
static RE_TOKEN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"['"]?token['"]?\s*:\s*['"]([^'"]+)['"]"#).unwrap());
static RE_EXPIRES: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"['"]?expires['"]?\s*:\s*['"]([^'"]+)['"]"#).unwrap());
static RE_M3U8_RAW: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(https?://[^'"\s]+\.m3u8[^'"\s]*)"#).unwrap());
static RE_SUBTITLE_URL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"window\.subtitleUrl\s*=\s*['"]([^'"]+)['"]"#).unwrap());
static RE_SUBTITLE_ARR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"subtitles\s*:\s*(\[[^\]]*\])"#).unwrap());
static RE_SUBTITLE_VAL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"subtitle[s]?\s*[:=]\s*['"]([^'"]+)['"]"#).unwrap());
static RE_MASTER_URL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"url\s*:\s*['"]([^'"]+)['"]"#).unwrap());

pub struct VixsrcClient {
    client: ClientWithMiddleware,
}

#[derive(Debug, Deserialize)]
struct VixsrcApiResponse {
    src: Option<String>,
}

#[derive(Debug, Deserialize)]
struct VixsrcStream {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ExternalSubTrack {
    label: String,
    file: String,
}

impl VixsrcClient {
    pub fn new(client: ClientWithMiddleware) -> Self {
        Self { client }
    }

    /// Fetch subtitles from external API (sub.vdrk.site).
    async fn fetch_external_subtitles(
        &self,
        tmdb_id: u64,
        media_type: &str,
        season: u32,
        episode: u32,
    ) -> Option<Vec<Subtitle>> {
        let url = if media_type == "movie" {
            format!("https://sub.vdrk.site/v2/movie/{}", tmdb_id)
        } else {
            format!(
                "https://sub.vdrk.site/v2/tv/{}/{}/{}",
                tmdb_id, season, episode
            )
        };

        match self
            .client
            .get(&url)
            .header("User-Agent", USER_AGENT)
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                match resp.json::<Vec<ExternalSubTrack>>().await {
                    Ok(tracks) if !tracks.is_empty() => {
                        let subs: Vec<Subtitle> = tracks
                            .into_iter()
                            .map(|t| {
                                let lang = t
                                    .label
                                    .to_lowercase()
                                    .split_whitespace()
                                    .next()
                                    .unwrap_or("en")
                                    .to_string();
                                Subtitle {
                                    url: t.file,
                                    lang: lang.clone(),
                                    label: t.label,
                                }
                            })
                            .collect();
                        info!(
                            "[Vixsrc Rust] External subs: {} tracks for TMDB {}",
                            subs.len(),
                            tmdb_id
                        );
                        return Some(subs);
                    }
                    Ok(_) => {}
                    Err(e) => warn!("[Vixsrc Rust] External sub JSON parse failed: {}", e),
                }
            }
            Ok(resp) => warn!("[Vixsrc Rust] External sub API returned {}", resp.status()),
            Err(e) => warn!("[Vixsrc Rust] External sub request failed: {}", e),
        }
        None
    }

    pub async fn get_streams_normalized(
        &self,
        tmdb_id: u64,
        media_type: &str,
        season: u32,
        episode: u32,
        title: Option<&str>,
    ) -> Result<Vec<StreamResult>> {
        info!(
            "[Vixsrc Rust] Fetching: TMDB {} ({}) S{} E{}",
            tmdb_id, media_type, season, episode
        );

        let page_url = if media_type == "movie" {
            format!("https://vixsrc.to/movie/{}", tmdb_id)
        } else {
            format!("https://vixsrc.to/tv/{}/{}/{}", tmdb_id, season, episode)
        };

        let api_url = if media_type == "movie" {
            let mut url = format!("https://vixsrc.to/api/movie/{}?h=1", tmdb_id);
            if let Some(t) = title {
                url.push_str(&format!("&t={}", urlencoding::encode(t)));
            }
            url
        } else {
            format!(
                "https://vixsrc.to/api/tv/{}/{}/{}?h=1",
                tmdb_id, season, episode
            )
        };

        let mut headers = HashMap::new();
        for (k, v) in vixsrc_browser_headers(&page_url) {
            headers.insert(k.to_string(), v);
        }

        // ── Step 1: API call to get embed path ──
        let embed_url = match {
            let mut req = self.client.get(&api_url);
            for (k, v) in vixsrc_browser_headers(&page_url) {
                req = req.header(k, v);
            }
            req.send().await
        } {
            Ok(resp) if resp.status().is_success() => {
                match resp.json::<VixsrcApiResponse>().await {
                    Ok(api) => api.src.filter(|s| !s.trim().is_empty()).map(|src| {
                        if src.starts_with("http://") || src.starts_with("https://") {
                            src
                        } else {
                            format!("https://vixsrc.to{}", src)
                        }
                    }),
                    Err(e) => {
                        warn!("[Vixsrc Rust] API JSON parse failed: {}", e);
                        None
                    }
                }
            }
            Ok(resp) => {
                warn!("[Vixsrc Rust] API returned {}", resp.status());
                None
            }
            Err(e) => {
                warn!("[Vixsrc Rust] API request failed: {}", e);
                None
            }
        };

        // ── Step 2: Extract streams + subtitles from embed page ──
        if let Some(ref url) = embed_url {
            match self.extract_from_embed_page(url, &page_url).await {
                Ok((streams, _embed_subtitles)) if !streams.is_empty() => {
                    let external_subs = self
                        .fetch_external_subtitles(tmdb_id, media_type, season, episode)
                        .await;
                    let mut results = Vec::new();
                    for s in streams {
                        let mut h = headers.clone();
                        h.insert("Referer".to_string(), page_url.clone());
                        // Every stream we extract is a real HLS playlist — the
                        // master's `vixsrc.to/playlist/…?type=video` variants
                        // (which the proxy rewrites: AES key + vix-content.net
                        // segments), or a raw m3u8 fallback. Serve native HLS,
                        // not an iframe embed.
                        results.push(StreamResult {
                            url: s.url,
                            title: s.title,
                            quality: s.quality,
                            provider: "Aura".to_string(),
                            format: "hls".to_string(),
                            subtitles: external_subs.clone(),
                            headers: Some(h),
                            extra_metadata: None,
                        });
                    }
                    return Ok(results);
                }
                Ok(_) => warn!(
                    "[Vixsrc Rust] Embed page yielded no streams for TMDB {}",
                    tmdb_id
                ),
                Err(e) => warn!("[Vixsrc Rust] Embed extraction failed: {}", e),
            }
        }

        // ── Step 3: Fallback — scrape the detail page directly ──
        let resp = {
            let mut req = self.client.get(&page_url);
            for (k, v) in vixsrc_browser_headers(VIXSRC_REFERER) {
                req = req.header(k, v);
            }
            req.send().await?
        };

        let body_bytes = resp.bytes().await?;
        let html = maybe_decompress_body(&body_bytes);

        let mut results = Vec::new();

        // Extract auth params + master URL from window.masterPlaylist using regex
        let re_master = &*RE_MASTER_PL;
        let mut auth_token: Option<String> = None;
        let mut auth_expires: Option<String> = None;
        if let Some(caps) = re_master.captures(&html) {
            if let Some(block) = caps.get(1) {
                let block_text = block.as_str();
                auth_token = RE_TOKEN
                    .captures(block_text)
                    .and_then(|c| c.get(1))
                    .map(|m| m.as_str().to_string());
                auth_expires = RE_EXPIRES
                    .captures(block_text)
                    .and_then(|c| c.get(1))
                    .map(|m| m.as_str().to_string());
                if let Some(m) = RE_MASTER_URL.captures(block_text) {
                    if let Some(url_match) = m.get(1) {
                        let mut master_url = url_match.as_str().to_string();
                        if let (Some(ref t), Some(ref e)) = (&auth_token, &auth_expires) {
                            let sep = if master_url.contains('?') { '&' } else { '?' };
                            master_url = format!(
                                "{}{sep}token={t}&expires={e}&h=1&lang=en",
                                master_url,
                                sep = sep,
                                t = t,
                                e = e
                            );
                        }
                        let header_tuples: Vec<(String, String)> = headers
                            .iter()
                            .map(|(k, v)| (k.clone(), v.clone()))
                            .collect();
                        results.extend(
                            expand_master_variants(
                                &self.client,
                                &master_url,
                                &header_tuples,
                                "Aura - Master",
                            )
                            .await,
                        );
                    }
                }
            }
        }

        // Try parsing window.streams as JSON array
        let re_streams = &*RE_STREAMS_ARR;
        if let Some(caps) = re_streams.captures(&html) {
            if let Some(block) = caps.get(1) {
                if let Ok(servers) = serde_json::from_str::<Vec<VixsrcStream>>(block.as_str()) {
                    for server in servers {
                        if let Some(u) = server.url.filter(|s| !s.is_empty()) {
                            let authed =
                                if let (Some(ref t), Some(ref e)) = (&auth_token, &auth_expires) {
                                    let sep = if u.contains('?') { '&' } else { '?' };
                                    format!(
                                        "{}{sep}token={t}&expires={e}&h=1&lang=en",
                                        u,
                                        sep = sep,
                                        t = t,
                                        e = e
                                    )
                                } else {
                                    u
                                };
                            let name = server.name.as_deref().unwrap_or("Server");
                            // VidXsrc URLs include ?rendition=1080p/480p/720p when a
                            // specific resolution is available. Extract it so quality is
                            // meaningful instead of always "Auto".
                            let quality = extract_rendition_quality(&authed)
                                .unwrap_or_else(|| "Auto".to_string());
                            results.push(InternalStream {
                                url: authed,
                                quality: quality.clone(),
                                title: format!("Aura - {} - {}", name, quality),
                            });
                        }
                    }
                }
            }
        }

        // Fallback: scan for any m3u8 URLs in the page
        if results.is_empty() {
            let re_m3u8 = &*RE_M3U8_RAW;
            let seen: std::collections::HashSet<String> = re_m3u8
                .find_iter(&html)
                .map(|m| m.as_str().to_string())
                .collect();
            for url in seen {
                results.push(InternalStream {
                    url,
                    quality: "Auto".to_string(),
                    title: "Aura - Direct - Auto".to_string(),
                });
            }
        }

        if results.is_empty() {
            warn!("[Vixsrc Rust] No streams found for TMDB {}", tmdb_id);
            return Ok(vec![]);
        }

        let external_subs = self
            .fetch_external_subtitles(tmdb_id, media_type, season, episode)
            .await;

        let out: Vec<StreamResult> = results
            .into_iter()
            .map(|s| StreamResult {
                url: s.url,
                title: s.title,
                quality: s.quality,
                provider: "Aura".to_string(),
                format: "hls".to_string(),
                subtitles: external_subs.clone(),
                headers: Some(headers.clone()),
                extra_metadata: None,
            })
            .collect();

        Ok(out)
    }

    /// Fetch embed page and extract streams + subtitles.
    /// Returns (streams, optional_subtitles).
    async fn extract_from_embed_page(
        &self,
        embed_url: &str,
        page_url: &str,
    ) -> Result<(Vec<InternalStream>, Option<Vec<Subtitle>>)> {
        let resp = {
            let mut req = self.client.get(embed_url);
            for (k, v) in vixsrc_browser_headers(page_url) {
                req = req.header(k, v);
            }
            req.send().await?
        };

        if !resp.status().is_success() {
            anyhow::bail!("Vixsrc embed page returned {}", resp.status());
        }

        let body_bytes = resp.bytes().await?;
        let html = maybe_decompress_body(&body_bytes);
        let preview: String = html.chars().take(400).collect();
        info!(
            "[Vixsrc Rust] embed page length={} preview={}",
            html.len(),
            preview
        );
        let mut streams: Vec<InternalStream> = Vec::new();
        let mut auth_token: Option<String> = None;
        let mut auth_expires: Option<String> = None;

        // ── Extract auth params + master URL from window.masterPlaylist ──
        // Vixsrc uses JS object notation (single quotes) so serde_json fails;
        // fall back to regex extraction from the raw block text.
        let re_master = &*RE_MASTER_PL;
        let master_block = re_master
            .captures(&html)
            .and_then(|c| c.get(1).map(|m| m.as_str().to_string()));

        if let Some(ref block_text) = master_block {
            auth_token = RE_TOKEN
                .captures(block_text)
                .and_then(|c| c.get(1))
                .map(|m| m.as_str().to_string());
            auth_expires = RE_EXPIRES
                .captures(block_text)
                .and_then(|c| c.get(1))
                .map(|m| m.as_str().to_string());
            if let Some(m) = RE_MASTER_URL.captures(block_text) {
                if let Some(url_match) = m.get(1) {
                    let mut master_url = url_match.as_str().to_string();
                    if let (Some(ref t), Some(ref e)) = (&auth_token, &auth_expires) {
                        let sep = if master_url.contains('?') { '&' } else { '?' };
                        master_url = format!(
                            "{}{sep}token={t}&expires={e}&h=1&lang=en",
                            master_url,
                            sep = sep,
                            t = t,
                            e = e
                        );
                    }
                    let browser_headers: Vec<(String, String)> =
                        vixsrc_browser_headers(VIXSRC_REFERER)
                            .into_iter()
                            .map(|(k, v)| (k.to_string(), v))
                            .collect();
                    streams.extend(
                        expand_master_variants(
                            &self.client,
                            &master_url,
                            &browser_headers,
                            "Aura - Master",
                        )
                        .await,
                    );
                }
            }
        } else {
            // Fallback: regex the whole embed HTML for `token:"..."`,
            // `expires:"..."`, and `url:"..."` (used when masterPlaylist
            // block is absent). Mirrors the Node embed API extraction.
            info!("[Vixsrc Rust] no window.masterPlaylist — falling back to whole-HTML regex");
            auth_token = RE_TOKEN
                .captures(&html)
                .and_then(|c| c.get(1))
                .map(|m| m.as_str().to_string());
            auth_expires = RE_EXPIRES
                .captures(&html)
                .and_then(|c| c.get(1))
                .map(|m| m.as_str().to_string());
            if let Some(m) = RE_MASTER_URL.captures(&html) {
                if let Some(url_match) = m.get(1) {
                    let mut master_url = url_match.as_str().to_string();
                    if let (Some(ref t), Some(ref e)) = (&auth_token, &auth_expires) {
                        let sep = if master_url.contains('?') { '&' } else { '?' };
                        master_url = format!(
                            "{}{sep}token={t}&expires={e}&h=1&lang=en",
                            master_url,
                            sep = sep,
                            t = t,
                            e = e
                        );
                    }
                    let browser_headers: Vec<(String, String)> =
                        vixsrc_browser_headers(VIXSRC_REFERER)
                            .into_iter()
                            .map(|(k, v)| (k.to_string(), v))
                            .collect();
                    streams.extend(
                        expand_master_variants(
                            &self.client,
                            &master_url,
                            &browser_headers,
                            "Aura - Master",
                        )
                        .await,
                    );
                }
            }
        }

        // ── Parse window.streams as JSON array ──
        let re_streams = &*RE_STREAMS_ARR;
        info!(
            "[Vixsrc Rust] window.masterPlaylist found={} window.streams found={} token_found={}",
            master_block.is_some(),
            re_streams.captures(&html).is_some(),
            auth_token.is_some(),
        );
        if let Some(caps) = re_streams.captures(&html) {
            if let Some(block) = caps.get(1) {
                info!(
                    "[Vixsrc Rust] window.streams block: {}",
                    block.as_str().chars().take(200).collect::<String>()
                );
                match serde_json::from_str::<Vec<VixsrcStream>>(block.as_str()) {
                    Ok(servers) => {
                        info!(
                            "[Vixsrc Rust] parsed {} servers from window.streams",
                            servers.len()
                        );
                        for server in servers {
                            if let Some(u) = server.url.filter(|s| !s.is_empty()) {
                                let authed = if let (Some(ref t), Some(ref e)) =
                                    (&auth_token, &auth_expires)
                                {
                                    let sep = if u.contains('?') { '&' } else { '?' };
                                    format!(
                                        "{}{sep}token={t}&expires={e}&h=1&lang=en",
                                        u,
                                        sep = sep,
                                        t = t,
                                        e = e
                                    )
                                } else {
                                    u
                                };
                                let name = server.name.as_deref().unwrap_or("Server");
                                // VidXsrc URLs include ?rendition=1080p/480p/720p when a
                                // specific resolution is available. Extract it so quality is
                                // meaningful instead of always "Auto".
                                let quality = extract_rendition_quality(&authed)
                                    .unwrap_or_else(|| "Auto".to_string());
                                // dedupe by URL
                                if !streams.iter().any(|s| s.url == authed) {
                                    streams.push(InternalStream {
                                        url: authed,
                                        quality: quality.clone(),
                                        title: format!("Aura - {} - {}", name, quality),
                                    });
                                }
                            }
                        }
                    }
                    Err(e) => warn!("[Vixsrc Rust] window.streams JSON parse failed: {}", e),
                }
            }
        }

        // ── Fallback: scan for raw m3u8 URLs ──
        if streams.is_empty() {
            let re_m3u8 = &*RE_M3U8_RAW;
            let seen: std::collections::HashSet<String> = re_m3u8
                .find_iter(&html)
                .map(|m| m.as_str().to_string())
                .collect();
            for url in seen {
                streams.push(InternalStream {
                    url,
                    quality: "Auto".to_string(),
                    title: "Aura - Direct - Auto".to_string(),
                });
            }
        }

        let subtitles = extract_subtitles(&html);
        Ok((streams, subtitles))
    }
}

#[derive(Debug)]
struct InternalStream {
    url: String,
    quality: String,
    title: String,
}

/// Parse resolution (e.g. 1920x1080) into "1080p" quality label.
fn resolution_to_quality(resolution: &str) -> Option<String> {
    let parts: Vec<&str> = resolution.split('x').collect();
    if parts.len() == 2 {
        if let Ok(h) = parts[1].parse::<i32>() {
            return Some(format!("{}p", h));
        }
    }
    None
}

/// Fetch a vixsrc master playlist and expand its variants into separate streams
/// with quality derived from RESOLUTION/BANDWIDTH attributes.
async fn expand_master_variants(
    client: &ClientWithMiddleware,
    master_url: &str,
    headers: &[(String, String)],
    title_prefix: &str,
) -> Vec<InternalStream> {
    let mut req = client.get(master_url);
    for (k, v) in headers {
        if let Ok(name) = reqwest::header::HeaderName::from_bytes(k.as_bytes()) {
            if let Ok(value) = reqwest::header::HeaderValue::from_str(v) {
                req = req.header(name, value);
            }
        }
    }

    let body = match req.send().await {
        Ok(resp) if resp.status().is_success() => resp.text().await.unwrap_or_default(),
        _ => return vec![],
    };

    let mut variants = Vec::new();
    let mut last_inf: Option<String> = None;
    for line in body.lines() {
        let line = line.trim();
        if line.starts_with("#EXT-X-STREAM-INF") {
            last_inf = Some(line.to_string());
        } else if !line.is_empty() && !line.starts_with('#') {
            if let Some(ref inf) = last_inf {
                let resolution = Regex::new(r"RESOLUTION=(\d+x\d+)")
                    .ok()
                    .and_then(|re| re.captures(inf))
                    .and_then(|c| c.get(1))
                    .map(|m| m.as_str());
                let bandwidth = Regex::new(r"BANDWIDTH=(\d+)")
                    .ok()
                    .and_then(|re| re.captures(inf))
                    .and_then(|c| c.get(1))
                    .map(|m| m.as_str().parse::<u64>().unwrap_or(0));

                let quality = resolution
                    .and_then(resolution_to_quality)
                    .unwrap_or_else(|| {
                        bandwidth
                            .map(|bw| match bw {
                                0..=400_000 => "240p".to_string(),
                                400_001..=800_000 => "360p".to_string(),
                                800_001..=1_500_000 => "480p".to_string(),
                                1_500_001..=3_000_000 => "720p".to_string(),
                                3_000_001..=5_000_000 => "1080p".to_string(),
                                _ => "4K".to_string(),
                            })
                            .unwrap_or_else(|| "Auto".to_string())
                    });

                // Resolve relative variant URLs against the master URL.
                let variant_url = if line.starts_with("http://") || line.starts_with("https://") {
                    line.to_string()
                } else {
                    match reqwest::Url::parse(master_url) {
                        Ok(base) => base
                            .join(line)
                            .map(|u| u.to_string())
                            .unwrap_or_else(|_| line.to_string()),
                        Err(_) => line.to_string(),
                    }
                };

                variants.push(InternalStream {
                    url: variant_url,
                    quality: quality.clone(),
                    title: format!("{} - {}", title_prefix, quality),
                });
            }
            last_inf = None;
        }
    }

    if variants.is_empty() {
        // Fallback: if the URL isn't actually a multi-variant master, keep it as-is.
        return vec![InternalStream {
            url: master_url.to_string(),
            quality: "1080p".to_string(),
            title: format!("{} - 1080p", title_prefix),
        }];
    }

    variants
}

/// Extract subtitle URLs from embed page HTML.
fn extract_subtitles(html: &str) -> Option<Vec<Subtitle>> {
    // window.subtitleUrl = "..."
    if let Some(caps) = RE_SUBTITLE_URL.captures(html) {
        if let Some(m) = caps.get(1) {
            return Some(vec![Subtitle {
                url: m.as_str().to_string(),
                lang: "en".to_string(),
                label: "English".to_string(),
            }]);
        }
    }

    // subtitles: [{url:"...",lang:"..."}]
    if let Some(caps) = RE_SUBTITLE_ARR.captures(html) {
        if let Some(block) = caps.get(1) {
            if let Ok(arr) = serde_json::from_str::<Vec<Value>>(block.as_str()) {
                let subs: Vec<Subtitle> = arr
                    .into_iter()
                    .filter_map(|v| {
                        let url = v.get("url").and_then(|u| u.as_str())?;
                        let lang = v.get("lang").and_then(|l| l.as_str()).unwrap_or("en");
                        let label = v.get("label").and_then(|l| l.as_str()).unwrap_or(lang);
                        Some(Subtitle {
                            url: url.to_string(),
                            lang: lang.to_string(),
                            label: label.to_string(),
                        })
                    })
                    .collect();
                if !subs.is_empty() {
                    return Some(subs);
                }
            }
        }
    }

    // subtitle = "..."
    if let Some(caps) = RE_SUBTITLE_VAL.captures(html) {
        if let Some(m) = caps.get(1) {
            return Some(vec![Subtitle {
                url: m.as_str().to_string(),
                lang: "en".to_string(),
                label: "English".to_string(),
            }]);
        }
    }

    None
}
