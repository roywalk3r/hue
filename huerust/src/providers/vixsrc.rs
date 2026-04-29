//! Hue Stream API
//! Professional Streaming Aggregator
//!
//! @author roywalk3r <roywalker440@gmail.com>
//! @repository https://github.com/roywalk3r/hue.git
//! @license MIT

use reqwest::Client;
use anyhow::Result;
use regex::Regex;
use tracing::{info, warn};
use crate::models::{StreamResult, Subtitle};
use std::collections::HashMap;

const USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/147.0.0.0 Safari/537.36";
const VIXSRC_REFERER: &str = "https://vixsrc.to/";

pub struct VixsrcClient {
    client: Client,
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

        // 1. Get the embed path from the API (Next.js flow)
        let api_url = if media_type == "movie" {
            format!("https://vixsrc.to/api/movie/{}", tmdb_id)
        } else {
            format!("https://vixsrc.to/api/tv/{}/{}/{}", tmdb_id, season, episode)
        };

        let referer = if media_type == "movie" {
            format!("https://vixsrc.to/movie/{}", tmdb_id)
        } else {
            format!("https://vixsrc.to/tv/{}/{}/{}", tmdb_id, season, episode)
        };

        let api_resp = self.client
            .get(&api_url)
            .header("User-Agent", USER_AGENT)
            .header("Referer", &referer)
            .header("Accept", "application/json")
            .send()
            .await?;

        if !api_resp.status().is_success() {
            warn!("[Vixsrc Rust] API request failed with status: {}", api_resp.status());
            return Ok(vec![]);
        }

        let api_data: serde_json::Value = api_resp.json().await?;
        let embed_path = match api_data["src"].as_str() {
            Some(p) => p,
            None => {
                warn!("[Vixsrc Rust] No 'src' field in API response for TMDB {}", tmdb_id);
                return Ok(vec![]);
            }
        };

        // 2. Fetch the embed page
        let embed_url = format!("https://vixsrc.to{}", embed_path);
        let embed_resp = self.client
            .get(&embed_url)
            .header("User-Agent", USER_AGENT)
            .header("Referer", VIXSRC_REFERER)
            .send()
            .await?;

        let html = embed_resp.text().await?;

        // 3. Extract master playlist data from window.masterPlaylist
        let mut master_playlist_url: Option<String> = None;

        if html.contains("window.masterPlaylist") {
            info!("[Vixsrc Rust] Found window.masterPlaylist in embed page");

            let url_re = Regex::new(r#"url\s*:\s*['"]([^'"]+)['"]"#)?;
            let token_re = Regex::new(r#"(?:['"]?token['"]?|params:\s*\{\s*['"]token['"])\s*:\s*['"]([^'"]+)['"]"#)?;
            let expires_re = Regex::new(r#"(?:['"]?expires['"]?|params:\s*\{\s*[^}]*['"]expires['"])\s*:\s*['"]([^'"]+)['"]"#)?;

            let base_url = url_re.captures(&html).and_then(|c| c.get(1)).map(|m| m.as_str());
            let token = token_re.captures(&html).and_then(|c| c.get(1)).map(|m| m.as_str());
            let expires = expires_re.captures(&html).and_then(|c| c.get(1)).map(|m| m.as_str());

            if let (Some(base), Some(t), Some(e)) = (base_url, token, expires) {
                info!("[Vixsrc Rust] Extracted base_url={}, token={}, expires={}",
                    &base[..base.len().min(40)],
                    &t[..t.len().min(20)],
                    e
                );

                let constructed = if base.contains('?') {
                    format!("{}&token={}&expires={}&h=1&lang=en", base, t, e)
                } else {
                    format!("{}?token={}&expires={}&h=1&lang=en", base, t, e)
                };

                master_playlist_url = Some(constructed);
            }
        }

        // Method 2: Fallback to direct .m3u8 URLs anywhere in HTML (original behavior)
        if master_playlist_url.is_none() {
            let m3u8_re = Regex::new(r#"(https?://[^'"\s]+\.m3u8[^'"\s]*)"#)?;
            if let Some(m) = m3u8_re.captures(&html).and_then(|c| c.get(1)) {
                master_playlist_url = Some(m.as_str().to_string());
                info!("[Vixsrc Rust] Found direct .m3u8 URL");
            }
        }

        let final_url = match master_playlist_url {
            Some(u) => u,
            None => {
                warn!("[Vixsrc Rust] No master playlist URL found for TMDB {}", tmdb_id);
                return Ok(vec![]);
            }
        };

        let subs = self.fetch_subtitles(tmdb_id).await.unwrap_or_default();

        let mut headers = HashMap::new();
        headers.insert("Referer".to_string(), embed_url.clone());
        headers.insert("User-Agent".to_string(), USER_AGENT.to_string());

        let mut extra_metadata = HashMap::new();
        extra_metadata.insert("proxyMode".to_string(), "url".to_string());

        Ok(vec![StreamResult {
            url: final_url,
            title: "Aura - Auto (HLS)".to_string(),
            quality: "Auto".to_string(),
            provider: "Aura".to_string(),
            format: "hls".to_string(),
            subtitles: if subs.is_empty() { None } else { Some(subs) },
            headers: Some(headers),
            extra_metadata: Some(extra_metadata),
        }])
    }

    async fn fetch_subtitles(&self, tmdb_id: u64) -> Result<Vec<Subtitle>> {
        let url = format!("https://sub.wyzie.ru/search?id={}", tmdb_id);
        let resp = self.client
            .get(&url)
            .header("User-Agent", USER_AGENT)
            .send()
            .await?;

        let data: serde_json::Value = resp.json().await?;
        let subs = data
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|item| {
                        let url = item["url"].as_str()?.to_string();
                        let display = item["display"].as_str().unwrap_or("English");
                        let lang_lower = display.to_lowercase();
                        let lang_code = if lang_lower.contains("english") { "en" }
                            else if lang_lower.contains("spanish") { "es" }
                            else if lang_lower.contains("french") { "fr" }
                            else if lang_lower.contains("german") { "de" }
                            else if lang_lower.contains("arabic") { "ar" }
                            else if lang_lower.contains("portuguese") { "pt" }
                            else { "en" };
                        Some(Subtitle {
                            url,
                            lang: lang_code.to_string(),
                            label: display.to_string(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(subs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::Client;

    #[tokio::test]
    async fn test_vixsrc_the_boys() {
        let client = Client::new();
        let vixsrc = VixsrcClient::new(client);
        let results = vixsrc.get_streams_normalized(76479, "tv", 1, 1).await.unwrap();
        println!("Results: {:?}", results);
        assert!(!results.is_empty());
    }
}
