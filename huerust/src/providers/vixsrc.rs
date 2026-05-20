//! Hue Stream API
//! Professional Streaming Aggregator
//!
//! @author roywalk3r <roywalker440@gmail.com>
//! @repository https://github.com/roywalk3r/hue.git
//! @license MIT

use anyhow::Result;
use reqwest::Client;
use regex::Regex;
use serde::Deserialize;
use serde_json::Value;
use crate::models::{StreamResult, Subtitle};
use tracing::{info, warn};
use std::collections::HashMap;
use std::sync::LazyLock;

const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";
const VIXSRC_REFERER: &str = "https://vixsrc.to/";

// ── Cached regexes (compile once at first use) ──
static RE_MASTER_PL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"window\.masterPlaylist\s*=\s*(\{[\s\S]*?\n\s*\})\s*[;\n]"#).unwrap()
});
static RE_STREAMS_ARR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"window\.streams\s*=\s*(\[[\s\S]*?\]);"#).unwrap()
});
static RE_TOKEN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"['"]?token['"]?\s*:\s*['"]([^'"]+)['"]"#).unwrap()
});
static RE_EXPIRES: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"['"]?expires['"]?\s*:\s*['"]([^'"]+)['"]"#).unwrap()
});
static RE_M3U8_RAW: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(https?://[^'"\s]+\.m3u8[^'"\s]*)"#).unwrap()
});
static RE_SUBTITLE_URL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"window\.subtitleUrl\s*=\s*['"]([^'"]+)['"]"#).unwrap()
});
static RE_SUBTITLE_ARR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"subtitles\s*:\s*(\[[^\]]*\])"#).unwrap()
});
static RE_SUBTITLE_VAL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"subtitle[s]?\s*[:=]\s*['"]([^'"]+)['"]"#).unwrap()
});
static RE_MASTER_URL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"url\s*:\s*['"]([^'"]+)['"]"#).unwrap()
});

pub struct VixsrcClient {
    client: Client,
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
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    /// Fetch subtitles from external API (sub.vdrk.site).
    async fn fetch_external_subtitles(&self, tmdb_id: u64, media_type: &str, season: u32, episode: u32) -> Option<Vec<Subtitle>> {
        let url = if media_type == "movie" {
            format!("https://sub.vdrk.site/v2/movie/{}", tmdb_id)
        } else {
            format!("https://sub.vdrk.site/v2/tv/{}/{}/{}", tmdb_id, season, episode)
        };

        match self.client.get(&url).header("User-Agent", USER_AGENT).send().await {
            Ok(resp) if resp.status().is_success() => {
                match resp.json::<Vec<ExternalSubTrack>>().await {
                    Ok(tracks) if !tracks.is_empty() => {
                        let subs: Vec<Subtitle> = tracks.into_iter().map(|t| {
                            let lang = t.label.to_lowercase().split_whitespace().next().unwrap_or("en").to_string();
                            Subtitle {
                                url: t.file,
                                lang: lang.clone(),
                                label: t.label,
                            }
                        }).collect();
                        info!("[Vixsrc Rust] External subs: {} tracks for TMDB {}", subs.len(), tmdb_id);
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
    ) -> Result<Vec<StreamResult>> {
        info!("[Vixsrc Rust] Fetching: TMDB {} ({}) S{} E{}", tmdb_id, media_type, season, episode);

        let page_url = if media_type == "movie" {
            format!("https://vixsrc.to/movie/{}", tmdb_id)
        } else {
            format!("https://vixsrc.to/tv/{}/{}/{}", tmdb_id, season, episode)
        };

        let api_url = if media_type == "movie" {
            format!("https://vixsrc.to/api/movie/{}", tmdb_id)
        } else {
            format!("https://vixsrc.to/api/tv/{}/{}/{}", tmdb_id, season, episode)
        };

        let mut headers = HashMap::new();
        headers.insert("Referer".to_string(), page_url.clone());
        headers.insert("User-Agent".to_string(), USER_AGENT.to_string());

        // ── Step 1: API call to get embed path ──
        let embed_url = match self.client
            .get(&api_url)
            .header("User-Agent", USER_AGENT)
            .header("Referer", &page_url)
            .send()
            .await
        {
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
                    let external_subs = self.fetch_external_subtitles(tmdb_id, media_type, season, episode).await;
                    let mut results = Vec::new();
                    for s in streams {
                        let mut h = headers.clone();
                        h.insert("Referer".to_string(), page_url.clone());
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
                Ok(_) => warn!("[Vixsrc Rust] Embed page yielded no streams for TMDB {}", tmdb_id),
                Err(e) => warn!("[Vixsrc Rust] Embed extraction failed: {}", e),
            }
        }

        // ── Step 3: Fallback — scrape the detail page directly ──
        let resp = self.client
            .get(&page_url)
            .header("User-Agent", USER_AGENT)
            .header("Referer", VIXSRC_REFERER)
            .send()
            .await?;

        let html = resp.text().await?;

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
                            master_url = format!("{}{sep}token={t}&expires={e}&h=1&lang=en", master_url, sep=sep, t=t, e=e);
                        }
                        results.push(InternalStream {
                            url: master_url,
                            quality: "1080p".to_string(),
                            title: "Aura - Master - 1080p".to_string(),
                        });
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
                            let authed = if let (Some(ref t), Some(ref e)) = (&auth_token, &auth_expires) {
                                let sep = if u.contains('?') { '&' } else { '?' };
                                format!("{}{sep}token={t}&expires={e}&h=1&lang=en", u, sep=sep, t=t, e=e)
                            } else {
                                u
                            };
                            let name = server.name.as_deref().unwrap_or("Server");
                            results.push(InternalStream {
                                url: authed,
                                quality: "Auto".to_string(),
                                title: format!("Aura - {} - Auto", name),
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

        let external_subs = self.fetch_external_subtitles(tmdb_id, media_type, season, episode).await;

        let out: Vec<StreamResult> = results.into_iter().map(|s| StreamResult {
            url: s.url,
            title: s.title,
            quality: s.quality,
            provider: "Aura".to_string(),
            format: "hls".to_string(),
            subtitles: external_subs.clone(),
            headers: Some(headers.clone()),
            extra_metadata: None,
        }).collect();

        Ok(out)
    }

    /// Fetch embed page and extract streams + subtitles.
    /// Returns (streams, optional_subtitles).
    async fn extract_from_embed_page(
        &self,
        embed_url: &str,
        page_url: &str,
    ) -> Result<(Vec<InternalStream>, Option<Vec<Subtitle>>)> {
        let resp = self.client
            .get(embed_url)
            .header("User-Agent", USER_AGENT)
            .header("Referer", page_url)
            .send()
            .await?;

        if !resp.status().is_success() {
            anyhow::bail!("Vixsrc embed page returned {}", resp.status());
        }

        let html = resp.text().await?;
        let mut streams: Vec<InternalStream> = Vec::new();
        let mut auth_token: Option<String> = None;
        let mut auth_expires: Option<String> = None;

        // ── Extract auth params + master URL from window.masterPlaylist ──
        // Vixsrc uses JS object notation (single quotes) so serde_json fails;
        // fall back to regex extraction from the raw block text.
        let re_master = &*RE_MASTER_PL;
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
                            master_url = format!("{}{sep}token={t}&expires={e}&h=1&lang=en", master_url, sep=sep, t=t, e=e);
                        }
                        streams.push(InternalStream {
                            url: master_url,
                            quality: "1080p".to_string(),
                            title: "Aura - Master - 1080p".to_string(),
                        });
                    }
                }
            }
        }

        // ── Parse window.streams as JSON array ──
        let re_streams = &*RE_STREAMS_ARR;
        if let Some(caps) = re_streams.captures(&html) {
            if let Some(block) = caps.get(1) {
                if let Ok(servers) = serde_json::from_str::<Vec<VixsrcStream>>(block.as_str()) {
                    for server in servers {
                        if let Some(u) = server.url.filter(|s| !s.is_empty()) {
                            let authed = if let (Some(ref t), Some(ref e)) = (&auth_token, &auth_expires) {
                                let sep = if u.contains('?') { '&' } else { '?' };
                                format!("{}{sep}token={t}&expires={e}&h=1&lang=en", u, sep=sep, t=t, e=e)
                            } else {
                                u
                            };
                            let name = server.name.as_deref().unwrap_or("Server");
                            // dedupe by URL
                            if !streams.iter().any(|s| s.url == authed) {
                                streams.push(InternalStream {
                                    url: authed,
                                    quality: "Auto".to_string(),
                                    title: format!("Aura - {} - Auto", name),
                                });
                            }
                        }
                    }
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
