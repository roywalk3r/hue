//! StreamHub client — TMDB-keyed aggregator over 7+ upstream sub-providers.
//!
//! Endpoints (xyra.stream upgrade):
//!   GET /v1/streamhub/streams?api_key=key1&tmdb_id=...&type=movie|tv[&season=&episode=]
//!
//! Returns up to ~19 direct CDN URLs aggregated from sub-providers
//! (4khdhub, dahmermovies, videasy, vixsrc, notorrent, lordflix, vidlink,
//! showbox). Each stream carries its own `headers` map (User-Agent, Referer,
//! Accept, Range) that the proxy must replay — without them the upstream CDN
//! will 403.

use crate::models::StreamResult;
use anyhow::Result;
use reqwest_middleware::ClientWithMiddleware;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{info, warn};

const XYRA_BASE: &str = "https://api.xyra.stream/v1/streamhub";
const API_KEY: &str = "key1";
const USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

#[derive(Debug, Deserialize)]
pub struct StreamHubResponse {
    #[serde(default)]
    pub success: bool,
    #[serde(default)]
    pub count: u32,
    pub streams: Vec<StreamHubStream>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamHubStream {
    pub name: String,
    pub title: String,
    pub url: String,
    #[serde(default)]
    pub quality: Option<String>,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub format: Option<String>,
    #[serde(default)]
    pub headers: HashMap<String, String>,
}

pub struct StreamHubClient {
    client: ClientWithMiddleware,
}

impl StreamHubClient {
    pub fn new() -> Self {
        let reqwest_client = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(std::time::Duration::from_secs(15))
            .connect_timeout(std::time::Duration::from_secs(8))
            .tcp_keepalive(std::time::Duration::from_secs(60))
            .pool_max_idle_per_host(4)
            .pool_idle_timeout(std::time::Duration::from_secs(90))
            .build()
            .unwrap();
        Self {
            client: crate::http_client::wrap_with_retry(reqwest_client),
        }
    }

    /// Fetch all streams xyra can resolve for the given TMDB id.
    /// For TV, `season` and `episode` are required to get episode-specific
    /// streams (xyra returns season packs otherwise).
    pub async fn get_streams(
        &self,
        tmdb_id: u64,
        content_type: &str,
        season: u32,
        episode: u32,
    ) -> Result<Vec<StreamHubStream>> {
        let mut url = format!(
            "{}/streams?api_key={}&tmdb_id={}&type={}",
            XYRA_BASE, API_KEY, tmdb_id, content_type
        );
        if content_type == "tv" {
            url.push_str(&format!("&season={}&episode={}", season, episode));
        }
        info!("[streamhub] GET {}", url);
        let resp = self.client.get(&url).send().await?;
        let status = resp.status();
        if !status.is_success() {
            warn!("[streamhub] returned {}", status);
            return Ok(vec![]);
        }
        let body = resp.text().await?;
        info!("[streamhub] response {} bytes", body.len());
        let wrapped: serde_json::Result<StreamHubResponse> = serde_json::from_str(&body);
        let streams = match wrapped {
            Ok(w) => {
                if !w.success {
                    warn!("[streamhub] success=false in body");
                }
                w.streams
            }
            Err(e) => {
                warn!("[streamhub] json decode failed: {}", e);
                return Ok(vec![]);
            }
        };
        info!("[streamhub] returned {} streams", streams.len());
        Ok(streams)
    }
}

/// Convert a StreamHub stream into a generic StreamResult.
/// `provider` is the public display name ("StreamHub") for sticky-pref
/// mapping. The upstream sub-provider (4khdhub, dahmermovies, etc.) is
/// preserved in `extra_metadata["upstream"]` for the player UI.
/// `name` is the human display label (e.g. "4KHDHub - HubCloud - 1080p")
/// and is folded into the title for the player.
pub fn streamhub_to_result(s: &StreamHubStream) -> StreamResult {
    let upstream = s.provider.clone().unwrap_or_else(|| s.name.clone());
    let quality = s
        .quality
        .clone()
        .unwrap_or_else(|| "Auto".to_string());
    let mut extra = HashMap::new();
    extra.insert("upstream".to_string(), upstream.clone());
    extra.insert("label".to_string(), s.name.clone());
    StreamResult {
        url: s.url.clone(),
        title: format!("{} — {}", upstream, s.title),
        quality,
        provider: "StreamHub".to_string(),
        format: s.format.clone().unwrap_or_else(|| "mp4".to_string()),
        subtitles: None,
        headers: if s.headers.is_empty() {
            None
        } else {
            Some(s.headers.clone())
        },
        extra_metadata: Some(extra),
    }
}
