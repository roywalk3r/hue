//! Hue Stream API
//! Professional Streaming Aggregator
//!
//! @author roywalk3r <roywalker440@gmail.com>
//! @repository https://github.com/roywalk3r/hue.git
//! @license MIT

use serde::{Deserialize, Serialize};
use reqwest::Client;
use anyhow::Result;
use tracing::{info, warn};
use crate::models::StreamResult;
use std::collections::HashMap;

const XYRA_BASE: &str = "https://api.xyra.stream/v1/dramacool";
const XYRA_KEY: &str = "key1";
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

#[derive(Debug, Serialize, Deserialize)]
pub struct XyraResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub message: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct XyraSearchItem {
    pub id: String,
    pub title: String,
    #[serde(rename = "releaseYear")]
    pub release_year: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct XyraEpisode {
    pub episode_id: String,
    pub title: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct XyraDramaInfo {
    pub episodes: Vec<XyraEpisode>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct XyraStreamData {
    pub link: Option<String>,
    pub server_url: Option<String>,
}

pub struct DramacoolClient {
    client: Client,
    scraper_url: String,
}

impl DramacoolClient {
    pub fn new() -> Self {
        let scraper_url = std::env::var("MICROSERVICE_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:24610".to_string());
        Self {
            client: Client::builder()
                .user_agent(USER_AGENT)
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap(),
            scraper_url,
        }
    }

    async fn make_request<T: for<'de> Deserialize<'de>>(&self, endpoint: &str, params: Vec<(&str, &str)>) -> Result<XyraResponse<T>> {
        let mut url = format!("{}{}?api_key={}", XYRA_BASE, endpoint, XYRA_KEY);
        for (k, v) in params {
            url.push_str(&format!("&{}={}", k, urlencoding::encode(v)));
        }

        let resp = self.client.get(&url).send().await?;
        let data = resp.json::<XyraResponse<T>>().await?;
        Ok(data)
    }

    pub async fn search(&self, query: &str) -> Result<Vec<XyraSearchItem>> {
        let res = self.make_request::<Vec<XyraSearchItem>>("/search", vec![("query", query)]).await?;
        Ok(res.data.unwrap_or_default())
    }

    pub async fn get_info(&self, id: &str) -> Result<Option<XyraDramaInfo>> {
        let res = self.make_request::<XyraDramaInfo>("/info", vec![("id", id)]).await?;
        Ok(res.data)
    }

    pub async fn get_stream(&self, episode_id: &str) -> Result<Option<XyraStreamData>> {
        let res = self.make_request::<XyraStreamData>("/stream", vec![("episode_id", episode_id)]).await?;
        Ok(res.data)
    }

    pub async fn get_streams_normalized(&self, title: &str, _season: u32, episode: u32) -> Result<Vec<StreamResult>> {
        info!("[Dramacool Rust] Searching for: \"{}\" E{}", title, episode);
        
        let search_results = self.search(title).await?;
        if search_results.is_empty() { return Ok(vec![]); }

        let best_match = &search_results[0];
        
        let info = match self.get_info(&best_match.id).await? {
            Some(i) => i,
            None => return Ok(vec![]),
        };

        let ep_id = info.episodes.iter().find(|e| {
            e.title.to_lowercase().contains(&format!("episode {}", episode))
        }).map(|e| e.episode_id.clone());

        let ep_id = match ep_id {
            Some(id) => id,
            None => return Ok(vec![]),
        };

        match self.get_stream(&ep_id).await? {
            Some(data) => {
                let mut results = Vec::new();
                if let Some(link) = data.link {
                    if link.contains(".m3u8") {
                        let mut headers = HashMap::new();
                        headers.insert("Referer".to_string(), "https://dramacool.sh/".to_string());
                        headers.insert("User-Agent".to_string(), USER_AGENT.to_string());

                        results.push(StreamResult {
                            url: link,
                            title: "Orient - Auto".to_string(),
                            quality: "Auto".to_string(),
                            provider: "Orient".to_string(),
                            format: "hls".to_string(),
                            subtitles: None,
                            headers: Some(headers),
                        });
                        return Ok(results);
                    }
                }
                
                // Fallback to Microservice
                if let Some(server_url) = data.server_url {
                    info!("[Dramacool Rust] API direct link failed, delegating to Node.js Scraper Microservice for URL: {}", server_url);
                    
                    let payload = serde_json::json!({
                        "url": server_url
                    });

                    match self.client.post(format!("{}/scrape/dramacool", self.scraper_url))
                        .json(&payload)
                        .send()
                        .await {
                            Ok(scraper_resp) => {
                                if scraper_resp.status().is_success() {
                                    #[derive(Deserialize)]
                                    struct ScraperResponse {
                                        success: bool,
                                        streams: Vec<StreamResult>,
                                    }
                                    if let Ok(data) = scraper_resp.json::<ScraperResponse>().await {
                                        if data.success {
                                            return Ok(data.streams);
                                        }
                                    }
                                }
                            },
                            Err(e) => warn!("[Dramacool Rust] Scraper microservice call failed: {}", e),
                        }
                }
                
                Ok(results)
            }
            None => Ok(vec![])
        }
    }
}
