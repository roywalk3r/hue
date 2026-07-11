/*
 * HUE ENGINE ❄
 * System Designation: The Architecture of Aggregation
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */

//! PpVTo Sports Provider — Live sports streams via pooembed.eu
//!
//! Protocol (reverse-engineered May 2026):
//!   1. GET api.ppv.to/api/streams/{uri} → stream metadata, embed info, match timings
//!   2. Browser daemon loads pooembed.eu iframe → WASM decrypts → JW Player URL
//!   3. Daemon returns the decrypted m3u8 stream URL
//!
//! The ChaCha20 key (32 bytes) and nonce (12 bytes) in gasm.wasm are obfuscated
//! (XORed or dynamically assembled — 500K+ brute-force attempts failed).
//! The production solution uses a Node.js daemon (daemon/daemon.js) that keeps
//! a warm headless Chromium and extracts URLs via the live JW Player flow.

use reqwest_middleware::ClientWithMiddleware;
use anyhow::{Result, anyhow};
use moka::future::Cache;
use tracing::{info, warn};
use std::time::Duration;

use crate::constants::UA_DESKTOP;

const PPV_API: &str = "https://api.ppv.to/api/streams";
const DAEMON_URL: &str = "http://127.0.0.1:9321";

// ─── API types ───────────────────────────────────────────────────────────────────

#[derive(Debug, serde::Deserialize)]
pub struct PpvStreamData {
    pub id: u64,
    pub name: String,
    pub tag: Option<String>,
    #[serde(rename = "starts_at")]
    pub start_timestamp: Option<i64>,
    #[serde(rename = "ends_at")]
    pub end_timestamp: Option<i64>,
    #[serde(rename = "category_name")]
    pub category_name: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct PpvApiResponse {
    success: bool,
    data: Option<PpvStreamData>,
}

// ─── Client ──────────────────────────────────────────────────────────────────────

pub struct PpvToClient {
    client: ClientWithMiddleware,
    /// Cache: match_path → stream_url
    url_cache: Cache<String, String>,
}

impl PpvToClient {
    pub fn new(client: ClientWithMiddleware) -> Self {
        Self {
            client,
            // Cache stream URLs for the match window (default 6h)
            url_cache: Cache::builder()
                .max_capacity(512)
                .time_to_live(Duration::from_secs(6 * 3600))
                .build(),
        }
    }

    // ── Daemon call ───────────────────────────────────────────────────────────

    /// Call the extraction daemon to get the decrypted stream URL.
    async fn extract_via_daemon(&self, match_path: &str) -> Result<String> {
        // Check cache first
        if let Some(url) = self.url_cache.get(match_path).await {
            info!("[PpVTo] Cache hit: {}", match_path);
            return Ok(url);
        }

        let daemon_path = format!("{}/{}", DAEMON_URL, match_path);

        let resp = self.client
            .get(&daemon_path)
            .timeout(Duration::from_secs(40))
            .send()
            .await
            .map_err(|e| anyhow!("Daemon unreachable ({}): {}. Start: cd daemon && npm start", DAEMON_URL, e))?;

        if !resp.status().is_success() {
            return Err(anyhow!("Daemon returned {}: {}", resp.status(),
                resp.text().await.unwrap_or_default()));
        }

        let url = resp.text().await?;
        let url = url.trim().to_string();

        if url.is_empty() || !url.starts_with("http") {
            return Err(anyhow!("Daemon returned invalid URL: {}", url));
        }

        // Cache it
        self.url_cache.insert(match_path.to_string(), url.clone()).await;
        info!("[PpVTo] Extracted: {} → {}...", match_path, &url[..url.len().min(80)]);

        Ok(url)
    }

    // ── Public API ────────────────────────────────────────────────────────────

    /// Fetch stream metadata for a specific match URI.
    pub async fn get_metadata(&self, uri: &str) -> Result<PpvStreamData> {
        let url = format!("{}/{}", PPV_API, uri);

        let resp = self.client
            .get(&url)
            .header("User-Agent", UA_DESKTOP)
            .header("Accept", "application/json")
            .timeout(Duration::from_secs(10))
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(anyhow!("ppv.to API returned {}", resp.status()));
        }

        let api_resp: PpvApiResponse = resp.json().await?;

        if !api_resp.success {
            return Err(anyhow!("ppv.to API success=false for {}", uri));
        }

        api_resp.data
            .ok_or_else(|| anyhow!("ppv.to API returned no data for {}", uri))
    }

    /// Full pipeline: metadata → daemon → StreamResult.
    pub async fn get_streams(&self, uri: &str) -> Result<Vec<crate::models::StreamResult>> {
        info!("[PpVTo] Fetching streams for: {}", uri);

        // Step 1: Get metadata from ppv.to
        let metadata = match self.get_metadata(uri).await {
            Ok(m) => m,
            Err(e) => {
                warn!("[PpVTo] Metadata fetch failed: {}", e);
                return Ok(vec![]);
            }
        };

        info!("[PpVTo] Match: {} (ID {})", metadata.name, metadata.id);

        // Step 2: Extract decrypted stream URL via daemon
        let stream_url = match self.extract_via_daemon(uri).await {
            Ok(url) => url,
            Err(e) => {
                warn!("[PpVTo] Extraction failed: {}", e);
                return Ok(vec![]);
            }
        };

        let format = if stream_url.contains(".m3u8") { "hls" } else { "mp4" };

        let sport = metadata.tag.as_deref()
            .or(metadata.category_name.as_deref())
            .unwrap_or("Live");

        let title = format!("{} | {} | {}", metadata.name, sport, format.to_uppercase());

        let mut headers = std::collections::HashMap::new();
        headers.insert("Referer".to_string(), "https://pooembed.eu/".to_string());
        headers.insert("User-Agent".to_string(), UA_DESKTOP.to_string());
        headers.insert("Origin".to_string(), "https://pooembed.eu".to_string());

        let stream = crate::models::StreamResult {
            url: stream_url,
            title,
            quality: "Auto".to_string(),
            provider: "PpVTo".to_string(),
            format: format.to_string(),
            subtitles: None,
            headers: Some(headers),
            extra_metadata: Some({
                let mut m = std::collections::HashMap::new();
                m.insert("match_id".to_string(), metadata.id.to_string());
                m.insert("sport".to_string(), sport.to_string());
                if let Some(start) = metadata.start_timestamp {
                    m.insert("start".to_string(), start.to_string());
                }
                if let Some(end) = metadata.end_timestamp {
                    m.insert("end".to_string(), end.to_string());
                }
                m
            }),
        };

        info!("[PpVTo] Returning stream: {} ({})", stream.title, stream.format);
        Ok(vec![stream])
    }

}
