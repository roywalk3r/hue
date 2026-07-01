//! Xyra Subtitle client — OpenSubtitles-backed subtitle search and download.
//!
//! Endpoints (xyra.stream upgrade):
//!   GET /v1/subtitle/search?imdbid|tmdb_id|query[&type=&lang=&season=&episode=]
//!   GET /v1/subtitle/download?url=...
//!
//! Note: `/search` returns a **raw JSON array** (unlike other xyra endpoints
//! which wrap in `{success, data, message}`). The download endpoint is not
//! exposed to the Flutter app — `proxifySubtitleUrl` already wraps plaintext
//! URLs through the existing `/sub-proxy` and that's sufficient for the player.

use anyhow::Result;
use reqwest_middleware::ClientWithMiddleware;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

const XYRA_BASE: &str = "https://api.xyra.stream/v1/subtitle";
const API_KEY: &str = "key1";
const USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubtitleResult {
    pub id: String,
    #[serde(rename = "fileName")]
    pub file_name: String,
    pub lang: String,
    pub format: String,
    #[serde(rename = "downloadUrl")]
    pub download_url: String,
    #[serde(rename = "downloadCount", default)]
    pub download_count: u64,
    #[serde(default)]
    pub score: f64,
    #[serde(rename = "hearingImpaired", default)]
    pub hearing_impaired: bool,
    #[serde(rename = "movieName", default)]
    pub movie_name: Option<String>,
    #[serde(rename = "movieYear", default)]
    pub movie_year: Option<String>,
    #[serde(default)]
    pub season: Option<String>,
    #[serde(default)]
    pub episode: Option<String>,
    #[serde(rename = "imdbId", default)]
    pub imdb_id: Option<String>,
}

pub struct XyraSubtitleClient {
    client: ClientWithMiddleware,
}

impl XyraSubtitleClient {
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

    /// Search for subtitles. At least one of `imdbid`, `tmdb_id`, or `query`
    /// must be supplied (xyra returns a 400 otherwise).
    pub async fn search(
        &self,
        imdbid: Option<&str>,
        tmdb_id: Option<u64>,
        query: Option<&str>,
        content_type: Option<&str>,
        lang: Option<&str>,
        season: Option<u32>,
        episode: Option<u32>,
    ) -> Result<Vec<SubtitleResult>> {
        if imdbid.is_none() && tmdb_id.is_none() && query.is_none() {
            warn!("[xyra subtitle] search called with no identifier");
            return Ok(vec![]);
        }

        let mut url = format!("{}/search?api_key={}", XYRA_BASE, API_KEY);
        if let Some(v) = imdbid {
            url.push_str(&format!("&imdbid={}", urlencoding::encode(v)));
        }
        if let Some(v) = tmdb_id {
            url.push_str(&format!("&tmdb_id={}", v));
        }
        if let Some(v) = query {
            url.push_str(&format!("&query={}", urlencoding::encode(v)));
        }
        if let Some(v) = content_type {
            url.push_str(&format!("&type={}", urlencoding::encode(v)));
        }
        if let Some(v) = lang {
            url.push_str(&format!("&lang={}", urlencoding::encode(v)));
        }
        if let Some(v) = season {
            url.push_str(&format!("&season={}", v));
        }
        if let Some(v) = episode {
            url.push_str(&format!("&episode={}", v));
        }

        info!("[xyra subtitle] GET {}", url);
        let resp = self.client.get(&url).send().await?;
        let status = resp.status();
        if !status.is_success() {
            warn!("[xyra subtitle] search returned {}", status);
            return Ok(vec![]);
        }

        // xyra returns a bare array for /search (not the {success,data} envelope).
        let results: Vec<SubtitleResult> = resp.json().await?;
        info!("[xyra subtitle] search returned {} results", results.len());
        Ok(results)
    }

    /// Download a subtitle file. xyra auto-decompresses gzip. Returns raw SRT/VTT.
    pub async fn download(&self, url: &str) -> Result<String> {
        let full = format!("{}/download?api_key={}&url={}", XYRA_BASE, API_KEY, urlencoding::encode(url));
        info!("[xyra subtitle] download {}", url);
        let resp = self.client.get(&full).send().await?;
        if !resp.status().is_success() {
            anyhow::bail!("xyra subtitle download failed: {}", resp.status());
        }
        Ok(resp.text().await?)
    }
}
