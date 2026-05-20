//! Vidsrc-Embed Client — delegates browser-based stream extraction to Node.js daemon.
//!
//! Flow:
//!   POST daemon /scrape/vidsrc-embed { tmdbId, mediaType, season, episode }
//!   → { success, streams: [{ url, quality, subtitles, headers }] }

use reqwest::Client;
use anyhow::Result;
use tracing::{info, warn};
use crate::models::StreamResult;
use std::collections::HashMap;

const DAEMON_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36";

pub struct VidsrcEmbedClient {
    client: Client,
    daemon_url: String,
}

impl VidsrcEmbedClient {
    pub fn new() -> Self {
        let daemon_url = std::env::var("MICROSERVICE_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:24610".to_string());
        Self {
            client: Client::builder()
                .user_agent(DAEMON_UA)
                .timeout(std::time::Duration::from_secs(60))
                .connect_timeout(std::time::Duration::from_secs(10))
                .tcp_keepalive(std::time::Duration::from_secs(60))
                .pool_max_idle_per_host(2)
                .pool_idle_timeout(std::time::Duration::from_secs(90))
                .build()
                .unwrap(),
            daemon_url,
        }
    }

    pub async fn get_streams(
        &self,
        tmdb_id: &str,
        media_type: &str,
        season: Option<u32>,
        episode: Option<u32>,
    ) -> Result<Vec<StreamResult>> {
        info!("[Nebula] Delegating to daemon for TMDB {} ({}) S{:?} E{:?}", tmdb_id, media_type, season, episode);

        let payload = serde_json::json!({
            "tmdbId": tmdb_id,
            "mediaType": if media_type == "tv" || media_type == "series" { "tv" } else { "movie" },
            "season": season,
            "episode": episode,
        });

        let resp = self.client
            .post(format!("{}/scrape/vidsrc-embed", self.daemon_url))
            .json(&payload)
            .send()
            .await?;

        #[derive(serde::Deserialize)]
        struct DaemonResponse {
            success: bool,
            streams: Vec<DaemonStream>,
        }

        #[derive(serde::Deserialize)]
        struct DaemonStream {
            url: String,
            quality: Option<String>,
            subtitles: Option<Vec<DaemonSubtitle>>,
            headers: Option<HashMap<String, String>>,
        }

        #[derive(serde::Deserialize)]
        struct DaemonSubtitle {
            url: String,
            lang: Option<String>,
            label: Option<String>,
        }

        if !resp.status().is_success() {
            return Err(anyhow::anyhow!("[Nebula] Daemon returned {}", resp.status()));
        }

        let data: DaemonResponse = resp.json().await?;

        if !data.success || data.streams.is_empty() {
            warn!("[Nebula] Daemon returned no streams");
            return Ok(vec![]);
        }

        let mut results = Vec::new();
        for s in data.streams {
            let subs = s.subtitles.map(|ss| {
                ss.into_iter()
                    .map(|sub| crate::models::Subtitle {
                        url: sub.url,
                        lang: sub.lang.unwrap_or_else(|| "en".to_string()),
                        label: sub.label.unwrap_or_else(|| "English".to_string()),
                    })
                    .collect::<Vec<_>>()
            });

            results.push(StreamResult {
                url: s.url,
                title: format!("Nebula - {}", s.quality.as_deref().unwrap_or("Auto")),
                quality: s.quality.unwrap_or_else(|| "Auto".to_string()),
                provider: "Nebula".to_string(),
                format: "hls".to_string(),
                subtitles: if subs.as_ref().map(|v| v.is_empty()).unwrap_or(true) { None } else { subs },
                headers: s.headers,
                extra_metadata: None,
            });
        }

        info!("[Nebula] Daemon returned {} streams", results.len());
        Ok(results)
    }
}
