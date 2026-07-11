/*
 * HUE ENGINE ❄
 * System Designation: The Architecture of Aggregation
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */

#![allow(dead_code)]
//! Videasy Client — delegates WASM decryption to Node.js daemon.
//!
//! Flow:
//!   1. GET api.videasy.net/cdn/sources-with-title?title=... → hex response
//!   2. POST daemon /scrape/videasy { hexResponse, tmdbId } → decrypted streams

use crate::models::StreamResult;
use anyhow::Result;
use reqwest_middleware::ClientWithMiddleware;
use std::collections::HashMap;
use tracing::{info, warn};

const API_BASE: &str = "https://api.videasy.net";
const DAEMON_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36";

pub struct VideasyClient {
    client: ClientWithMiddleware,
    daemon_url: String,
}

impl VideasyClient {
    pub fn new() -> Self {
        let daemon_url = std::env::var("MICROSERVICE_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:24610".to_string());
        let client = reqwest::Client::builder()
            .user_agent(DAEMON_UA)
            .timeout(std::time::Duration::from_secs(30))
            .connect_timeout(std::time::Duration::from_secs(10))
            .tcp_keepalive(std::time::Duration::from_secs(60))
            .pool_max_idle_per_host(2)
            .pool_idle_timeout(std::time::Duration::from_secs(90))
            .build()
            .unwrap();
        Self {
            client: crate::http_client::wrap_with_retry(client),
            daemon_url,
        }
    }

    pub async fn get_streams_normalized(
        &self,
        title: &str,
        media_type: &str,
        year: Option<u32>,
        tmdb_id: u64,
        imdb_id: Option<&str>,
        season: u32,
        episode: u32,
    ) -> Result<Vec<StreamResult>> {
        info!(
            "[Vortex Rust] Fetching: \"{}\" ({}) S{} E{}",
            title, media_type, season, episode
        );

        let media_type_param = if media_type == "tv" || media_type == "series" {
            "tv"
        } else {
            "movie"
        };

        // Step 1: Fetch hex response from videasy.net API
        let mut api_url = format!(
            "{}/cdn/sources-with-title?title={}&mediaType={}&year={}&tmdbId={}",
            API_BASE,
            urlencoding::encode(title),
            media_type_param,
            year.map(|y| y.to_string()).unwrap_or_default(),
            tmdb_id,
        );

        if let Some(imdb) = imdb_id {
            api_url.push_str(&format!("&imdbId={}", imdb));
        }
        if media_type_param == "tv" {
            api_url.push_str(&format!("&seasonId={}&episodeId={}", season, episode));
        }

        info!("[Vortex Rust] API: {}", api_url);

        let resp = self
            .client
            .get(&api_url)
            .header("Accept", "*/*")
            .header("Referer", "https://cineby.sc/")
            .send()
            .await?;

        let hex_response = resp.text().await?;

        if hex_response.is_empty() || hex_response.len() < 100 {
            warn!("[Vortex Rust] Empty or invalid hex response");
            return Ok(vec![]);
        }

        // Step 2: Delegate to daemon for WASM decryption
        let payload = serde_json::json!({
            "hexResponse": hex_response.trim(),
            "tmdbId": tmdb_id,
        });

        let daemon_resp = self
            .client
            .post(format!("{}/scrape/videasy", self.daemon_url))
            .json(&payload)
            .send()
            .await?;

        #[derive(serde::Deserialize)]
        struct DaemonResponse {
            success: bool,
            streams: Vec<DaemonStream>,
            subtitles: Vec<DaemonSubtitle>,
        }

        #[derive(serde::Deserialize)]
        struct DaemonStream {
            url: String,
            quality: Option<String>,
        }

        #[derive(serde::Deserialize)]
        struct DaemonSubtitle {
            url: String,
            lang: String,
            label: String,
        }

        if !daemon_resp.status().is_success() {
            return Err(anyhow::anyhow!(
                "[Vortex Rust] Daemon returned {}",
                daemon_resp.status()
            ));
        }

        let data: DaemonResponse = daemon_resp.json().await?;

        if !data.success {
            return Err(anyhow::anyhow!("[Vortex Rust] Daemon decryption failed"));
        }

        let mut headers = HashMap::new();
        headers.insert("Referer".to_string(), "https://cineby.sc/".to_string());
        headers.insert("User-Agent".to_string(), DAEMON_UA.to_string());

        let subs: Vec<crate::models::Subtitle> = data
            .subtitles
            .into_iter()
            .map(|s| crate::models::Subtitle {
                url: s.url,
                lang: s.lang,
                label: s.label,
            })
            .collect();

        let mut results: Vec<StreamResult> = data
            .streams
            .into_iter()
            .map(|s| StreamResult {
                url: s.url,
                title: format!("Vortex - {}", s.quality.as_deref().unwrap_or("Auto")),
                quality: s.quality.unwrap_or_else(|| "Auto".to_string()),
                provider: "Vortex".to_string(),
                format: "hls".to_string(),
                subtitles: if subs.is_empty() {
                    None
                } else {
                    Some(subs.clone())
                },
                headers: Some(headers.clone()),
                extra_metadata: None,
            })
            .collect();

        // Attach subtitles only to first stream
        if !subs.is_empty() && !results.is_empty() {
            results[0].subtitles = Some(subs);
        }

        info!("[Vortex Rust] Found {} streams", results.len());
        Ok(results)
    }
}
