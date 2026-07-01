//! Dramacool9 client — xyra.stream's second drama indexer (dramacool9.com.ro).
//!
//! Endpoints (xyra):
//!   GET /v1/dramacool9/search?query=...        → { success, data: [{id, title, image, time}], pagination }
//!   GET /v1/dramacool9/info?id=<slug>          → { success, data: { title, episodes: [{title, episode_id}] } }
//!   GET /v1/dramacool9/stream?id=&episode_id=  → { success, data: { title, servers: {Name: {stream, embeded_link, m3u8, sub}} } }
//!
//! The `servers` map is a flat dict of {server_name: server_info}. Each server has:
//!   - `stream`      — direct m3u8 URL (the one we want)
//!   - `embeded_link`— iframe URL (fallback for has_m3u8=false servers)
//!   - `m3u8: bool`  — only `true` entries carry a real m3u8
//!   - `sub`         — sometimes null, sometimes has subtitle tracks
//!
//! Episodes are returned as a flat list (no per-episode season). The `episode_id`
//! follows the convention `{show_id}-episode-{N}` so we can synthesize it from
//! `episode` when the user requests S1E15.

use crate::models::StreamResult;
use anyhow::Result;
use reqwest_middleware::ClientWithMiddleware;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{info, warn};

const XYRA_BASE: &str = "https://api.xyra.stream/v1/dramacool9";
const XYRA_KEY: &str = "key1";
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

#[derive(Debug, Serialize, Deserialize)]
pub struct XyraResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    #[allow(dead_code)]
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct D9SearchItem {
    pub id: String,
    pub title: String,
    #[serde(default)]
    #[allow(dead_code)]
    pub image: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    pub time: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct D9Episode {
    pub episode_id: String,
    pub title: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct D9DramaInfo {
    pub title: String,
    pub episodes: Vec<D9Episode>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct D9Server {
    pub stream: Option<String>,
    pub embeded_link: Option<String>,
    #[serde(default)]
    pub m3u8: bool,
    #[serde(default)]
    pub sub: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct D9StreamData {
    pub title: String,
    pub servers: HashMap<String, D9Server>,
    #[serde(default)]
    #[allow(dead_code)]
    pub has_m3u8: bool,
}

pub struct Dramacool9Client {
    client: ClientWithMiddleware,
}

impl Dramacool9Client {
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

    async fn make_request<T: for<'de> Deserialize<'de>>(
        &self,
        endpoint: &str,
        params: Vec<(&str, &str)>,
    ) -> Result<XyraResponse<T>> {
        let mut url = format!("{}{}?api_key={}", XYRA_BASE, endpoint, XYRA_KEY);
        for (k, v) in params {
            url.push_str(&format!("&{}={}", k, urlencoding::encode(v)));
        }

        info!("[dramacool9] GET {}", url);
        let resp = self.client.get(&url).send().await?;
        let status = resp.status();
        if !status.is_success() {
            warn!("[dramacool9] returned {}", status);
            return Ok(XyraResponse {
                success: false,
                data: None,
                message: Some(status.to_string()),
            });
        }
        let data = resp.json::<XyraResponse<T>>().await?;
        Ok(data)
    }

    /// Fetch the raw response body for browse endpoints.
    /// xyra's browse endpoints are inconsistent — some return the rails
    /// directly (`{"drama": [...], "kshow": [...]}`), others wrap in
    /// `{"success": true, "data": {rails}}`. Normalize to always return
    /// the inner rails object (or array) so the frontend gets a stable shape.
    async fn browse_request(
        &self,
        endpoint: &str,
        params: Vec<(&str, &str)>,
    ) -> Result<serde_json::Value> {
        let mut url = format!("{}{}?api_key={}", XYRA_BASE, endpoint, XYRA_KEY);
        for (k, v) in params {
            url.push_str(&format!("&{}={}", k, urlencoding::encode(v)));
        }

        info!("[dramacool9] GET {}", url);
        let resp = self.client.get(&url).send().await?;
        let status = resp.status();
        if !status.is_success() {
            warn!("[dramacool9] browse {} returned {}", endpoint, status);
            return Ok(serde_json::Value::Null);
        }
        let body: serde_json::Value = resp.json().await?;
        // Unwrap {"success":true, "data": ...} if present.
        if let Some(obj) = body.as_object() {
            if let Some(data) = obj.get("data") {
                if !data.is_null() {
                    return Ok(data.clone());
                }
            }
        }
        Ok(body)
    }

    pub async fn search(&self, query: &str) -> Result<Vec<D9SearchItem>> {
        let res = self
            .make_request::<Vec<D9SearchItem>>("/search", vec![("query", query)])
            .await?;
        Ok(res.data.unwrap_or_default())
    }

    pub async fn get_info(&self, id: &str) -> Result<Option<D9DramaInfo>> {
        let res = self
            .make_request::<D9DramaInfo>("/info", vec![("id", id)])
            .await?;
        Ok(res.data)
    }

    pub async fn get_stream(
        &self,
        id: &str,
        episode_id: &str,
    ) -> Result<Option<D9StreamData>> {
        let res = self
            .make_request::<D9StreamData>(
                "/stream",
                vec![("id", id), ("episode_id", episode_id)],
            )
            .await?;
        Ok(res.data)
    }

    /// Browse homepage — returns the raw xyra payload (drama/kshow/ongoing/countries/genres).
    /// Pass-through JSON since Rust does not consume the individual rails.
    pub async fn home(&self) -> Result<serde_json::Value> {
        self.browse_request("/home", vec![]).await
    }

    /// Popular dramas list.
    pub async fn popular(&self) -> Result<serde_json::Value> {
        self.browse_request("/popular", vec![]).await
    }

    /// List dramas filtered by country and/or genre. Both are optional.
    pub async fn list(
        &self,
        country: Option<&str>,
        genre: Option<&str>,
    ) -> Result<serde_json::Value> {
        let mut params: Vec<(&str, &str)> = Vec::new();
        if let Some(c) = country {
            params.push(("country", c));
        }
        if let Some(g) = genre {
            params.push(("genre", g));
        }
        self.browse_request("/list", params).await
    }

    /// Discover — filtered, paginated browse.
    pub async fn discover(&self) -> Result<serde_json::Value> {
        self.browse_request("/discover", vec![]).await
    }

    /// Direct lookup by show id (slug) — used by the drama route handler
    /// when the caller already knows the exact slug (no search round-trip).
    pub async fn get_streams_by_id(
        &self,
        show_id: &str,
        episode: u32,
    ) -> Result<Vec<StreamResult>> {
        let info = match self.get_info(show_id).await? {
            Some(i) => i,
            None => return Ok(vec![]),
        };

        if info.episodes.is_empty() {
            warn!("[dramacool9] no episodes in info for {}", show_id);
            return Ok(vec![]);
        }

        // Try to match by "Episode N" or "Episode N: ..." in the episode title.
        // Fall back to index `episode-1` (1-based) if no match.
        let needle = format!("episode {}", episode);
        let ep_id = info
            .episodes
            .iter()
            .find(|e| e.title.to_lowercase().contains(&needle))
            .map(|e| e.episode_id.clone())
            .or_else(|| {
                let idx = (episode as usize).saturating_sub(1);
                info.episodes.get(idx).map(|e| e.episode_id.clone())
            });

        let ep_id = match ep_id {
            Some(id) => id,
            None => return Ok(vec![]),
        };

        info!("[dramacool9] resolved episode_id={} for E{}", ep_id, episode);

        let data = match self.get_stream(show_id, &ep_id).await? {
            Some(d) => d,
            None => return Ok(vec![]),
        };

        let mut results = Vec::new();
        for (server_name, server) in &data.servers {
            if !server.m3u8 {
                continue;
            }
            let stream_url = match &server.stream {
                Some(u) if !u.is_empty() => u.clone(),
                _ => continue,
            };

            let mut headers = HashMap::new();
            headers.insert("Referer".to_string(), "https://dramacool9.com.ro/".to_string());
            headers.insert("User-Agent".to_string(), USER_AGENT.to_string());

            let title = format!("Orient9 - {} - E{}", server_name, episode);
            results.push(StreamResult {
                url: stream_url,
                title,
                quality: "Auto".to_string(),
                provider: "Orient9".to_string(),
                format: "hls".to_string(),
                subtitles: None,
                headers: Some(headers),
                extra_metadata: Some({
                    let mut m = HashMap::new();
                    m.insert("server".to_string(), server_name.clone());
                    m.insert("episode_id".to_string(), ep_id.clone());
                    m
                }),
            });
        }

        info!("[dramacool9] returning {} streams", results.len());
        Ok(results)
    }

    /// Search + info + stream fetch.
    /// `season` is ignored — Dramacool9 returns a flat episode list.
    /// If `episode_id` is given (precise), use it; otherwise match by episode title.
    pub async fn get_streams_normalized(
        &self,
        title: &str,
        _season: u32,
        episode: u32,
    ) -> Result<Vec<StreamResult>> {
        info!("[dramacool9] Searching for: \"{}\" E{}", title, episode);

        let search_results = self.search(title).await?;
        if search_results.is_empty() {
            return Ok(vec![]);
        }

        let best_match = &search_results[0];
        info!("[dramacool9] best match: {} ({})", best_match.title, best_match.id);

        let info = match self.get_info(&best_match.id).await? {
            Some(i) => i,
            None => return Ok(vec![]),
        };

        if info.episodes.is_empty() {
            warn!("[dramacool9] no episodes in info for {}", best_match.id);
            return Ok(vec![]);
        }

        // Try to match by "Episode N" or "Episode N: ..." in the episode title.
        // Fall back to index `episode-1` (1-based) if no match.
        let needle = format!("episode {}", episode);
        let ep_id = info
            .episodes
            .iter()
            .find(|e| e.title.to_lowercase().contains(&needle))
            .map(|e| e.episode_id.clone())
            .or_else(|| {
                let idx = (episode as usize).saturating_sub(1);
                info.episodes.get(idx).map(|e| e.episode_id.clone())
            });

        let ep_id = match ep_id {
            Some(id) => id,
            None => return Ok(vec![]),
        };

        info!("[dramacool9] resolved episode_id={} for E{}", ep_id, episode);

        let data = match self.get_stream(&best_match.id, &ep_id).await? {
            Some(d) => d,
            None => return Ok(vec![]),
        };

        let mut results = Vec::new();
        for (server_name, server) in &data.servers {
            // Only take m3u8 servers with a direct stream URL.
            if !server.m3u8 {
                continue;
            }
            let stream_url = match &server.stream {
                Some(u) if !u.is_empty() => u.clone(),
                _ => continue,
            };

            let mut headers = HashMap::new();
            // Referer / UA help the m3u8 CDN serve content reliably.
            headers.insert("Referer".to_string(), "https://dramacool9.com.ro/".to_string());
            headers.insert("User-Agent".to_string(), USER_AGENT.to_string());

            let title = format!("Orient9 - {} - E{}", server_name, episode);
            results.push(StreamResult {
                url: stream_url,
                title,
                quality: "Auto".to_string(),
                provider: "Orient9".to_string(),
                format: "hls".to_string(),
                subtitles: None,
                headers: Some(headers),
                extra_metadata: Some({
                    let mut m = HashMap::new();
                    m.insert("server".to_string(), server_name.clone());
                    m.insert("episode_id".to_string(), ep_id.clone());
                    m
                }),
            });
        }

        info!("[dramacool9] returning {} streams", results.len());
        Ok(results)
    }
}
