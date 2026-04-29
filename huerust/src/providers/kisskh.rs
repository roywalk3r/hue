//! Hue Stream API
//! Professional Streaming Aggregator
//!
//! @author roywalk3r <roywalker440@gmail.com>
//! @repository https://github.com/roywalk3r/hue.git
//! @license MIT

use serde::{Deserialize, Serialize};
use reqwest::Client;
use anyhow::Result;
use base64::{Engine as _, engine::general_purpose};
use tracing::info;
use crate::models::{StreamResult, Subtitle};
use std::collections::HashMap;

const API_BASE: &str = "https://kisskh.ovh/api";
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

#[derive(Debug, Serialize, Deserialize)]
pub struct KissKhSearchItem {
    pub id: u64,
    pub title: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct KissKhEpisode {
    pub id: u64,
    pub number: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct KissKhDramaDetail {
    pub episodes: Vec<KissKhEpisode>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct KissKhStreamPayload {
    #[serde(rename = "Video")]
    pub video: Option<String>,
    #[serde(rename = "ThirdParty")]
    pub third_party: Option<String>,
    #[serde(rename = "Subtitles")]
    pub subtitles: Option<Vec<KissKhSubtitle>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct KissKhSubtitle {
    pub src: String,
    pub label: Option<String>,
    pub lang: Option<String>,
}

pub struct KissKhClient {
    client: Client,
}

impl KissKhClient {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .user_agent(USER_AGENT)
                .tcp_keepalive(std::time::Duration::from_secs(60))
                .build()
                .unwrap(),
        }
    }

    pub async fn search(&self, title: &str) -> Result<Vec<KissKhSearchItem>> {
        let url = format!("{}/DramaList/Search?q={}", API_BASE, urlencoding::encode(title));
        let resp = self.client.get(&url).send().await?;
        let data = resp.json::<Vec<KissKhSearchItem>>().await?;
        Ok(data)
    }

    pub async fn get_episode_id(&self, show_id: u64, ep_num: u32) -> Result<Option<u64>> {
        let url = format!("{}/DramaList/Drama/{}?is_series=1", API_BASE, show_id);
        let resp = self.client.get(&url).send().await?;
        let data = resp.json::<KissKhDramaDetail>().await?;
        
        let ep = data.episodes.iter().find(|e| e.number.floor() as u32 == ep_num);
        Ok(ep.map(|e| e.id))
    }

    pub async fn get_stream(&self, ep_id: u64) -> Result<Option<KissKhStreamPayload>> {
        let url = format!("{}/DramaList/Episode/{}?kkey=0", API_BASE, ep_id);
        let resp = self.client.get(&url).send().await?;
        
        let text = resp.text().await?;
        if text.contains("<!DOCTYPE") {
            return Ok(None);
        }

        if let Ok(data) = serde_json::from_str::<KissKhStreamPayload>(&text) {
            return Ok(Some(data));
        }

        if let Ok(decoded_bytes) = general_purpose::STANDARD.decode(&text) {
            if let Ok(decoded_text) = String::from_utf8(decoded_bytes) {
                if let Ok(data) = serde_json::from_str::<KissKhStreamPayload>(&decoded_text) {
                    return Ok(Some(data));
                }
            }
        }

        Ok(None)
    }

    pub async fn get_streams_normalized(&self, title: &str, _season: u32, ep_num: u32) -> Result<Vec<StreamResult>> {
        info!("[KissKH Rust] Searching for: \"{}\" E{}", title, ep_num);
        
        let results = self.search(title).await?;
        if results.is_empty() { return Ok(vec![]); }

        let normalize = |s: &str| -> String {
            s.to_lowercase().chars().filter(|c| c.is_alphanumeric()).collect()
        };
        let target = normalize(title);
        let mut best_match = None;
        let mut best_score = -1;

        for item in results {
            let item_norm = normalize(&item.title);
            let mut score = 0;
            if item_norm == target {
                score = 1000;
            } else if item_norm.contains(&target) || target.contains(&item_norm) {
                score = 500 - (item_norm.len() as i32 - target.len() as i32).abs();
            }

            if score > best_score {
                best_score = score;
                best_match = Some(item);
            }
        }

        let best_match = match best_match {
            Some(m) if best_score > 0 => m,
            _ => return Ok(vec![]),
        };

        let ep_id = match self.get_episode_id(best_match.id, ep_num).await? {
            Some(id) => id,
            None => return Ok(vec![]),
        };

        match self.get_stream(ep_id).await? {
            Some(data) => {
                let mut results = Vec::new();
                let mut headers = HashMap::new();
                headers.insert("Referer".to_string(), "https://kisskh.ovh".to_string());
                headers.insert("User-Agent".to_string(), USER_AGENT.to_string());

                let subs = data.subtitles.map(|s_list| {
                    s_list.into_iter().map(|s| Subtitle {
                        url: s.src,
                        lang: s.lang.unwrap_or_else(|| "en".to_string()),
                        label: s.label.unwrap_or_else(|| "English".to_string()),
                    }).collect()
                });

                if let Some(video_url) = data.video {
                    results.push(StreamResult {
                        url: video_url,
                        title: "Zenith - Auto".to_string(),
                        quality: "Auto".to_string(),
                        provider: "Zenith".to_string(),
                        format: "hls".to_string(),
                        subtitles: subs,
                        headers: Some(headers),
                        extra_metadata: None,
                    });
                }
                Ok(results)
            }
            None => Ok(vec![])
        }
    }
}
