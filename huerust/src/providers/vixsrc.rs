//! Hue Stream API
//! Professional Streaming Aggregator
//!
//! @author roywalk3r <roywalker440@gmail.com>
//! @repository https://github.com/roywalk3r/hue.git
//! @license MIT

use crate::models::{StreamResult, Subtitle};
use anyhow::Result;
use reqwest_middleware::ClientWithMiddleware;
use serde::Deserialize;
use std::collections::HashMap;
use tracing::{info, warn};

const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/137.0.0.0 Safari/537.36";

/// Realistic browser headers so Cloudflare does not reject our API calls.
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

pub struct VixsrcClient {
    client: ClientWithMiddleware,
}

#[derive(Debug, Deserialize)]
struct VixsrcApiResponse {
    src: Option<String>,
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

        // ── Step 2: Emit the embed page as an iframe stream ──
        // vixsrc signs tokens and segments server-side, and Cloudflare
        // slow-walks our non-browser TLS on both the provider client and the
        // proxy — so we can neither mint nor reliably re-proxy the HLS. The
        // fresh /embed/{id}?token=… page, loaded directly in the browser with a
        // real TLS fingerprint, is the only reliably-playable path: vixsrc's own
        // known-good player handles the manifest, key, and segments.
        let Some(url) = embed_url else {
            warn!("[Vixsrc Rust] API returned no embed src for TMDB {}", tmdb_id);
            return Ok(vec![]);
        };

        info!("[Vixsrc Rust] emitting iframe embed for TMDB {}", tmdb_id);
        let mut h = headers.clone();
        h.insert("Referer".to_string(), page_url.clone());
        let external_subs = self
            .fetch_external_subtitles(tmdb_id, media_type, season, episode)
            .await;

        Ok(vec![StreamResult {
            url,
            title: "Aura".to_string(),
            quality: "Auto".to_string(),
            provider: "Aura".to_string(),
            format: "iframe".to_string(),
            subtitles: external_subs,
            headers: Some(h),
            extra_metadata: None,
        }])
    }
}
