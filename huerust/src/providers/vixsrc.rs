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

const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";
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

        let url = if media_type == "movie" {
            format!("https://vixsrc.to/movie/{}", tmdb_id)
        } else {
            format!("https://vixsrc.to/tv/{}/{}/{}", tmdb_id, season, episode)
        };

        let resp = self.client
            .get(&url)
            .header("User-Agent", USER_AGENT)
            .header("Referer", VIXSRC_REFERER)
            .send()
            .await?;

        let html = resp.text().await?;

        let re_url = Regex::new(r#"masterPlaylist\s*[=:]\s*\{[^}]*?"url"\s*:\s*"([^"]+)""#)?;
        let re_token = Regex::new(r#"masterPlaylist\s*[=:]\s*\{[^}]*?"token"\s*:\s*"([^"]+)""#)?;

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

        let subs = self.fetch_subtitles(tmdb_id).await.unwrap_or_default();

        let mut headers = HashMap::new();
        headers.insert("Referer".to_string(), VIXSRC_REFERER.to_string());
        headers.insert("User-Agent".to_string(), USER_AGENT.to_string());

        Ok(vec![StreamResult {
            url: final_url,
            title: "Aura - Auto (HLS)".to_string(),
            quality: "Auto".to_string(),
            provider: "Aura".to_string(),
            format: "hls".to_string(),
            subtitles: if subs.is_empty() { None } else { Some(subs) },
            headers: Some(headers),
extra_metadata: None,
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
