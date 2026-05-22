//! Hue Stream API
//! Professional Streaming Aggregator
//!
//! @author roywalk3r <roywalker440@gmail.com>
//! @repository https://github.com/roywalk3r/hue.git
//! @license MIT

#![allow(dead_code)]

use serde::Deserialize;
use reqwest::header::{HeaderMap, HeaderValue, ORIGIN, REFERER};
use reqwest::Client;
use anyhow::Result;
use std::collections::HashMap;
use tracing::{info, warn};
use crate::models::{StreamResult, Subtitle};

const BASE_URL: &str = "https://mp4hydra.org";
const USER_AGENT: &str = "Mozilla/5.0 (Linux; Android 6.0; Nexus 5 Build/MRA58N) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/137.0.0.0 Mobile Safari/537.36";

#[derive(Debug, Deserialize)]
struct HydraResponse {
    playlist: Vec<HydraItem>,
    servers: HashMap<String, String>,
}

#[derive(Debug, Deserialize)]
struct HydraItem {
    title: String,
    #[serde(rename = "show_title")]
    #[allow(dead_code)]
    show_title: Option<String>,
    src: String,
    quality: Option<String>,
    label: Option<String>,
    #[serde(default)]
    subs: Vec<HydraSub>,
}

#[derive(Debug, Deserialize)]
struct HydraSub {
    label: String,
    src: String,
}

pub struct Mp4HydraClient {
    client: Client,
}

impl Mp4HydraClient {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }

    fn generate_slug(title: &str) -> String {
        title.to_lowercase()
            .replace(|c: char| !c.is_alphanumeric() && c != ' ' && c != '-', "")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join("-")
            .replace("--", "-")
    }

    fn build_headers(&self, media_type: &str, slug: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert("Accept", HeaderValue::from_static("*/*"));
        headers.insert("Accept-Language", HeaderValue::from_static("en-GB,en-US;q=0.9,en;q=0.8"));
        headers.insert("Sec-Fetch-Dest", HeaderValue::from_static("empty"));
        headers.insert("Sec-Fetch-Mode", HeaderValue::from_static("cors"));
        headers.insert("Sec-Fetch-Site", HeaderValue::from_static("same-origin"));
        headers.insert(ORIGIN, HeaderValue::from_static(BASE_URL));
        let referer = format!("{}/{}/{}", BASE_URL, media_type, slug);
        headers.insert(REFERER, HeaderValue::from_str(&referer).unwrap_or_else(|_| HeaderValue::from_static(BASE_URL)));
        headers
    }

    async fn fetch_hydra(&self, slug: &str, media_type: &str, season: u32, episode: u32) -> Result<Option<HydraResponse>> {
        let url = format!("{}/info2?v=8", BASE_URL);
        let payload = serde_json::json!([{
            "s": slug,
            "t": media_type,
            "se": season,
            "ep": episode
        }]);

        let form = reqwest::multipart::Form::new()
            .text("v", "8")
            .text("z", payload.to_string());

        let headers = self.build_headers(media_type, slug);

        let resp = self.client.post(&url)
            .headers(headers)
            .multipart(form)
            .send()
            .await?;

        if !resp.status().is_success() {
            return Ok(None);
        }

        let data: HydraResponse = resp.json().await?;
        if data.playlist.is_empty() {
            return Ok(None);
        }

        Ok(Some(data))
    }

    fn process_response(&self, data: HydraResponse, title: &str, media_type: &str, season: u32, episode: u32) -> Vec<StreamResult> {
        let mut results = Vec::new();
        let servers = [
            ("Beta", "#1"),
            ("Beta#3", "#2"),
        ];

        let headers = {
            let mut h = HashMap::new();
            h.insert("Referer".to_string(), format!("{}/", BASE_URL));
            h.insert("User-Agent".to_string(), USER_AGENT.to_string());
            Some(h)
        };

        if media_type == "tv" {
            let padded_season = format!("{:02}", season);
            let padded_episode = format!("{:02}", episode);
            let target = format!("S{}E{}", padded_season, padded_episode);

            let target_item = data.playlist.iter().find(|item| {
                item.title.to_uppercase() == target.to_uppercase()
            });

            if let Some(item) = target_item {
                for (server_name, server_num) in &servers {
                    if let Some(base) = data.servers.get(*server_name) {
                        let video_url = format!("{}{}", base, item.src);
                        let quality = item.quality.clone().or_else(|| item.label.clone()).unwrap_or_else(|| "Auto".to_string());

                        let mut subtitles = None;
                        if !item.subs.is_empty() {
                            subtitles = Some(item.subs.iter().map(|sub| Subtitle {
                                url: format!("{}{}", base, sub.src),
                                lang: sub.label.clone(),
                                label: sub.label.clone(),
                            }).collect());
                        }

                        results.push(StreamResult {
                            url: video_url,
                            title: format!("{} - {} - {} [Hydra {}]", title, target, quality, server_num),
                            quality: quality.clone(),
                            provider: "Hydra".to_string(),
                            format: "mp4".to_string(),
                            subtitles,
                            headers: headers.clone(),
                            extra_metadata: None,
                        });
                    }
                }
            }
        } else {
            for item in &data.playlist {
                for (server_name, server_num) in &servers {
                    if let Some(base) = data.servers.get(*server_name) {
                        let video_url = format!("{}{}", base, item.src);
                        let quality = item.quality.clone().or_else(|| item.label.clone()).unwrap_or_else(|| "Auto".to_string());

                        let mut subtitles = None;
                        if !item.subs.is_empty() {
                            subtitles = Some(item.subs.iter().map(|sub| Subtitle {
                                url: format!("{}{}", base, sub.src),
                                lang: sub.label.clone(),
                                label: sub.label.clone(),
                            }).collect());
                        }

                        results.push(StreamResult {
                            url: video_url,
                            title: format!("{} - {} [Hydra {}]", title, quality, server_num),
                            quality: quality.clone(),
                            provider: "Hydra".to_string(),
                            format: "mp4".to_string(),
                            subtitles,
                            headers: headers.clone(),
                            extra_metadata: None,
                        });
                    }
                }
            }
        }

        results
    }

    pub async fn get_streams_normalized(&self, title: &str, original_title: Option<&str>, year: Option<u32>, media_type: &str, season: u32, episode: u32) -> Result<Vec<StreamResult>> {
        info!("[Hydra Rust] Searching for: \"{}\" ({}) S{} E{}", title, media_type, season, episode);

        let slug = if let Some(y) = year {
            format!("{}-{}", Self::generate_slug(title), y)
        } else {
            Self::generate_slug(title)
        };

        // Primary attempt
        if let Some(data) = self.fetch_hydra(&slug, media_type, season, episode).await? {
            return Ok(self.process_response(data, title, media_type, season, episode));
        }

        // Fallback 1: try original title
        if let Some(orig) = original_title {
            if orig != title {
                warn!("[Hydra Rust] Primary failed, retrying with original title: {}", orig);
                let orig_slug = if let Some(y) = year {
                    format!("{}-{}", Self::generate_slug(orig), y)
                } else {
                    Self::generate_slug(orig)
                };
                if let Some(data) = self.fetch_hydra(&orig_slug, media_type, season, episode).await? {
                    return Ok(self.process_response(data, orig, media_type, season, episode));
                }
            }
        }

        // Fallback 2: try without year for movies
        if media_type == "movie" && year.is_some() {
            warn!("[Hydra Rust] Retrying with title only (no year): {}", title);
            let plain_slug = Self::generate_slug(title);
            if let Some(data) = self.fetch_hydra(&plain_slug, media_type, season, episode).await? {
                return Ok(self.process_response(data, title, media_type, season, episode));
            }
        }

        Ok(vec![])
    }
}
