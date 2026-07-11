/*
 * HUE ENGINE ❄
 * System Designation: The Architecture of Aggregation
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */

//! NoTorrent provider — Stremio-style addon returning MP4/HLS streams.
//!
//! Flow:
//!   1. TMDB → IMDB ID (from the TmdbNormalizedInfo passed in)
//!   2. GET https://addon-osvh.onrender.com/stream/{movie|series}/{imdbId}[/{s}:{e}].json
//!   3. Filter out externalUrl items, github/google URLs, then emit one
//!      stream per item with parsed language + quality.

use crate::clients::tmdb::TmdbNormalizedInfo;
use crate::models::{StreamResult, Subtitle};
use anyhow::Result;
use reqwest_middleware::ClientWithMiddleware;
use serde::Deserialize;
use tracing::{info, warn};

const NOTORRENT_API: &str = "https://addon-osvh.onrender.com";

#[derive(Clone)]
pub struct NoTorrentClient {
    client: ClientWithMiddleware,
}

#[derive(Deserialize)]
struct AddonStream {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default, rename = "externalUrl")]
    external_url: Option<String>,
    #[serde(default)]
    behavior_hints: Option<BehaviorHints>,
}

#[derive(Deserialize, Default)]
struct BehaviorHints {
    #[serde(default)]
    headers: Option<serde_json::Value>,
    #[serde(default, rename = "proxyHeaders")]
    proxy_headers: Option<ProxyHeaders>,
}

#[derive(Deserialize, Default)]
struct ProxyHeaders {
    #[serde(default)]
    request: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct AddonResponse {
    #[serde(default)]
    streams: Vec<AddonStream>,
}

impl NoTorrentClient {
    pub fn new(client: ClientWithMiddleware) -> Self {
        Self { client }
    }

    pub async fn get_streams(
        &self,
        tmdb_info: &TmdbNormalizedInfo,
        media_type: &str,
        season: u32,
        episode: u32,
    ) -> Result<Vec<StreamResult>> {
        info!(
            "[NoTorrent] Fetching: TMDB {} ({}) S{} E{}",
            tmdb_info.id, media_type, season, episode
        );

        let imdb_id = match &tmdb_info.imdb_id {
            Some(s) if !s.is_empty() => s.clone(),
            _ => {
                warn!("[NoTorrent] No imdb_id in tmdb_info");
                return Ok(vec![]);
            }
        };

        let api_url = if media_type == "tv" {
            format!("{NOTORRENT_API}/stream/series/{imdb_id}:{season}:{episode}.json")
        } else {
            format!("{NOTORRENT_API}/stream/movie/{imdb_id}.json")
        };

        let resp = self
            .client
            .get(&api_url)
            .timeout(std::time::Duration::from_secs(20))
            .send()
            .await?;
        if !resp.status().is_success() {
            warn!("[NoTorrent] API returned {}", resp.status());
            return Ok(vec![]);
        }
        let data: AddonResponse = resp.json().await?;
        let raw_list = data.streams;
        let mut out = Vec::new();
        for item in raw_list {
            if item.external_url.is_some() || item.url.is_none() {
                continue;
            }
            let url = item.url.unwrap_or_default();
            if url.contains("github.com") || url.contains("googleusercontent") {
                continue;
            }
            // vid1.php/vid2.php and the /vid/ paths on hostingersite serve raw
            // HLS manifests whose segments are ad HTML pages
            // (page-N.html?token=?token= on scalableimpactgroup.site) — fake
            // playlists that fail in HLS.js and hang silently in an <iframe>.
            if url.contains("vid1.php?url=")
                || url.contains("vid2.php?url=")
                || url.contains("/vid/series/")
                || url.contains("/vid/movie/")
                // 321moviesfree "streams" point at HTML detail pages, not media
                || url.contains("321moviesfree.com/detail")
                // p.111477.xyz serves tiny range probes but 429s every real
                // transfer — passes reachability checks, then never plays
                || url.contains("p.111477.xyz/bulk")
            {
                continue;
            }
            let clean_title = clean_text(item.title.as_deref().unwrap_or(""));
            let quality = extract_quality(&clean_title);
            let language = if let Some(cap) = regex_lang(&clean_title) {
                let mut chars = cap.chars();
                let first = chars.next().unwrap_or(' ').to_uppercase().to_string();
                format!("{first}{}", chars.as_str())
            } else {
                "Default".to_string()
            };
            let mut headers = serde_json::Map::new();
            if let Some(bh) = &item.behavior_hints {
                if let Some(h) = &bh.headers {
                    if let Some(obj) = h.as_object() {
                        for (k, v) in obj {
                            if let Some(s) = v.as_str() {
                                headers.insert(k.clone(), serde_json::Value::String(s.to_string()));
                            }
                        }
                    }
                }
                if let Some(ph) = &bh.proxy_headers {
                    if let Some(req) = &ph.request {
                        if let Some(obj) = req.as_object() {
                            for (k, v) in obj {
                                if let Some(s) = v.as_str() {
                                    headers.insert(
                                        k.clone(),
                                        serde_json::Value::String(s.to_string()),
                                    );
                                }
                            }
                        }
                    }
                }
            }
            let headers_map: std::collections::HashMap<String, String> = headers
                .into_iter()
                .map(|(k, v)| (k, v.as_str().unwrap_or("").to_string()))
                .collect();
            let name = if language == "Default" {
                "NoTorrent".to_string()
            } else {
                format!("NoTorrent • {language}")
            };
            let format = if url.contains("vixsrc.to/playlist/")
                || url.contains("vixsrc.to/movie/")
                || url.contains("vixsrc.to/tv/")
                || url.contains("filme/")
            {
                // NoTorrent's iframe-wrapped streams (Vixsrc, filme) are HTML
                // player pages, not raw HLS. Marking as iframe lets the player
                // embed directly in <iframe> and bypass HLS.js
                // fragParsingError retries.
                "iframe"
            } else if url.contains(".m3u8")
                // tr.notorrent2.workers.dev is a relay that 302s to a
                // 321moviesfree.com `*.m3u8`; the .m3u8 isn't visible in the
                // outer URL so we have to detect the host pattern. Without
                // this the proxy returns HLS bytes but the client is told
                // format="mp4" and the player fails to parse the manifest.
                || url.contains("tr.notorrent2.workers.dev")
                || url.contains("stream-limit-vid.321moviesfree.com")
            {
                "hls"
            } else {
                "mp4"
            };
            out.push(StreamResult {
                url,
                title: quality.clone(),
                quality,
                provider: "NoTorrent".to_string(),
                format: format.to_string(),
                subtitles: None::<Vec<Subtitle>>,
                headers: if headers_map.is_empty() {
                    None
                } else {
                    Some(headers_map)
                },
                extra_metadata: None,
            });
            let _ = name;
        }
        info!("[NoTorrent] Total: {}", out.len());
        Ok(out)
    }
}

fn clean_text(s: &str) -> String {
    s.chars()
        .filter(|c| !matches!(c, '\u{1F300}'..='\u{1F9FF}' | '\u{2600}'..='\u{26FF}'))
        .collect::<String>()
        .trim()
        .to_string()
}

fn extract_quality(s: &str) -> String {
    let bytes = s.as_bytes();
    for i in 0..bytes.len() {
        if bytes[i] == b'p' {
            let mut start = i;
            while start > 0 && bytes[start - 1].is_ascii_digit() {
                start -= 1;
            }
            if i - start >= 3 {
                return format!("{}p", &s[start..i]);
            }
        }
    }
    if s.to_uppercase().contains("FREE") {
        "Auto".to_string()
    } else {
        "Unknown".to_string()
    }
}

fn regex_lang(s: &str) -> Option<String> {
    // Match (English), (Spanish), etc.
    if let Some(start) = s.rfind('(') {
        if let Some(end) = s.rfind(')') {
            if end > start {
                return Some(s[start + 1..end].to_string());
            }
        }
    }
    None
}
