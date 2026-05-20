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
use crate::models::StreamResult;
use tracing::{info, warn};
use std::collections::HashMap;
use url::Url;

const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";
const VIXSRC_REFERER: &str = "https://vixsrc.to/";

pub struct VixsrcClient {
    client: Client,
}

#[derive(Debug, Deserialize)]
struct VixsrcApiResponse {
    src: Option<String>,
}

impl VixsrcClient {
    pub fn new(client: Client) -> Self {
        Self { client }
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

        if let Ok(resp) = self.client
            .get(&api_url)
            .header("User-Agent", USER_AGENT)
            .header("Referer", &page_url)
            .send()
            .await
        {
            if resp.status().is_success() {
                if let Ok(api) = resp.json::<VixsrcApiResponse>().await {
                    if let Some(src) = api.src.filter(|s| !s.trim().is_empty()) {
                        let embed_url = if src.starts_with("http://") || src.starts_with("https://") {
                            src
                        } else {
                            format!("https://vixsrc.to{}", src)
                        };

                        match self.extract_hls_from_embed(&embed_url, &page_url).await {
                            Ok(final_url) => {
                                headers.insert("Referer".to_string(), embed_url);

                                return Ok(vec![StreamResult {
                                    url: final_url,
                                    title: "Aura - Auto (HLS)".to_string(),
                                    quality: "Auto".to_string(),
                                    provider: "Aura".to_string(),
                                    format: "hls".to_string(),
                                    subtitles: None,
                                    headers: Some(headers),
                                    extra_metadata: None,
                                }]);
                            }
                            Err(e) => warn!("[Vixsrc Rust] HLS extraction from embed failed: {}", e),
                        }

                        // Fallback: embed page didn't yield a playable stream — skip this provider
                        warn!("[Vixsrc Rust] Could not extract HLS from embed for TMDB {}", tmdb_id);
                    }
                }
            }
        }

        let resp = self.client
            .get(&page_url)
            .header("User-Agent", USER_AGENT)
            .header("Referer", VIXSRC_REFERER)
            .send()
            .await?;

        let html = resp.text().await?;

        let re_url = Regex::new(r#"(?s)masterPlaylist\s*[=:]\s*\{.*?["']?url["']?\s*:\s*["']([^"']+)["']"#)?;
        let re_token = Regex::new(r#"masterPlaylist\s*[=:]\s*\{[^}]*?["']token["']\s*:\s*["']([^"']+)["']"#)?;

        let stream_url = match re_url.captures(&html).and_then(|c| c.get(1)) {
            Some(m) => m.as_str().to_string(),
            None => {
                warn!("[Vixsrc Rust] masterPlaylist.url not found for TMDB {}", tmdb_id);
                return Ok(vec![]);
            }
        };

        let final_url = if let Some(token_match) = re_token.captures(&html).and_then(|c| c.get(1)) {
            let t = token_match.as_str();
            if stream_url.contains('?') {
                format!("{}&token={}", stream_url, t)
            } else {
                format!("{}?token={}", stream_url, t)
            }
        } else {
            stream_url
        };

        Ok(vec![StreamResult {
            url: final_url,
            title: "Aura - Auto (HLS)".to_string(),
            quality: "Auto".to_string(),
            provider: "Aura".to_string(),
            format: "hls".to_string(),
            subtitles: None,
            headers: Some(headers),
            extra_metadata: None,
        }])
    }

    async fn extract_hls_from_embed(&self, embed_url: &str, page_url: &str) -> Result<String> {
        let resp = self.client
            .get(embed_url)
            .header("User-Agent", USER_AGENT)
            .header("Referer", page_url)
            .send()
            .await?;

        if !resp.status().is_success() {
            warn!("[Vixsrc Rust] embed page returned {}", resp.status());
            anyhow::bail!("Vixsrc embed page failed");
        }

        let html = resp.text().await?;
        let re_url = Regex::new(r#"(?s)masterPlaylist\s*[=:]\s*\{.*?["']?url["']?\s*:\s*["']([^"']+)["']"#)?;
        let re_param = |name: &str| -> Result<Regex> {
            Ok(Regex::new(&format!(r#"["']{}["']\s*:\s*["']([^"']*)["']"#, regex::escape(name)))?)
        };

        let playlist = re_url
            .captures(&html)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().to_string())
            .ok_or_else(|| anyhow::anyhow!("Vixsrc masterPlaylist.url not found"))?;

        let mut url = Url::parse(&playlist)?;
        {
            let mut query = url.query_pairs_mut();
            for name in ["token", "expires", "asn"] {
                if let Some(value) = re_param(name)?
                    .captures(&html)
                    .and_then(|c| c.get(1))
                    .map(|m| m.as_str())
                    .filter(|v| !v.is_empty())
                {
                    query.append_pair(name, value);
                }
            }

            let lang = Url::parse(embed_url)
                .ok()
                .and_then(|u| {
                    u.query_pairs()
                        .find(|(k, _)| k == "lang")
                        .map(|(_, v)| v.to_string())
                })
                .unwrap_or_else(|| "en".to_string());
            query.append_pair("h", "1");
            query.append_pair("lang", &lang);
        }

        Ok(url.to_string())
    }
}
