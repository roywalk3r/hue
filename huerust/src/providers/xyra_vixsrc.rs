//! Aura (Vixsrc) via the xyra.stream streamhub proxy.
//!
//! Direct `vixsrc.to` is Cloudflare-blocked on the production VPS egress.
//! Xyra's streamhub proxy fetches the master playlist server-side and returns
//! a signed `vixsrc.to/playlist/{id}?token=…&expires=…&h=1` URL that the
//! existing proxy already knows how to rewrite (vixsrc.to + vix-content.net
//! segments, AES-128 key, m3u8-proxy paths).
//!
//! Endpoint:
//!   GET https://api.xyra.stream/v1/streamhub/vixsrc
//!     ?api_key=key1&tmdb_id={id}&type=movie|series[&season=&episode=]
//!
//! Response:
//!   { success, provider, tmdbId, count, streams: [{url, quality, headers}] }

use crate::models::StreamResult;
use anyhow::Result;
use reqwest_middleware::ClientWithMiddleware;
use serde::Deserialize;
use std::collections::HashMap;
use std::time::Duration;
use tracing::{info, warn};

const XYRA_BASE: &str = "https://api.xyra.stream/v1/streamhub/vixsrc";
const XYRA_KEY: &str = "key1";
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/137.0.0.0 Safari/537.36";

#[derive(Debug, Deserialize)]
struct XyraStream {
    url: String,
    #[serde(default)]
    quality: Option<String>,
    #[serde(default)]
    headers: Option<HashMap<String, String>>,
    #[serde(default)]
    title: Option<String>,
}

#[derive(Debug, Deserialize)]
struct XyraResp {
    #[serde(default)]
    #[allow(dead_code)]
    success: bool,
    #[serde(default)]
    streams: Vec<XyraStream>,
}

pub struct XyraVixsrcClient {
    client: ClientWithMiddleware,
}

impl XyraVixsrcClient {
    pub fn new() -> Self {
        let reqwest_client = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(Duration::from_secs(15))
            .connect_timeout(Duration::from_secs(8))
            .tcp_keepalive(Duration::from_secs(60))
            .pool_max_idle_per_host(4)
            .pool_idle_timeout(Duration::from_secs(90))
            .build()
            .unwrap();
        Self {
            client: crate::http_client::wrap_with_retry(reqwest_client),
        }
    }

    pub async fn get_streams_normalized(
        &self,
        tmdb_id: u64,
        media_type: &str,
        season: u32,
        episode: u32,
    ) -> Result<Vec<StreamResult>> {
        let kind = if media_type == "movie" { "movie" } else { "series" };
        let mut url = format!("{XYRA_BASE}?api_key={XYRA_KEY}&tmdb_id={tmdb_id}&type={kind}");
        if media_type != "movie" {
            url.push_str(&format!("&season={season}&episode={episode}"));
        }

        info!("[xyra_vixsrc] GET {url}");
        let resp = self.client.get(&url).send().await?;
        let status = resp.status();
        if !status.is_success() {
            warn!("[xyra_vixsrc] returned {status}");
            return Ok(vec![]);
        }
        let body: XyraResp = resp.json().await?;
        Ok(body
            .streams
            .into_iter()
            .map(|s| {
                let mut headers = s.headers.unwrap_or_default();
                headers
                    .entry("User-Agent".to_string())
                    .or_insert_with(|| USER_AGENT.to_string());
                StreamResult {
                    url: s.url,
                    title: s.title.unwrap_or_else(|| "Aura".to_string()),
                    quality: s.quality.unwrap_or_else(|| "Auto".to_string()),
                    provider: "Aura".to_string(),
                    format: "hls".to_string(),
                    subtitles: None,
                    headers: Some(headers),
                    extra_metadata: None,
                }
            })
            .collect())
    }
}
