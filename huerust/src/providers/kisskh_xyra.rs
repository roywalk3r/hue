//! KissKH (xyra) client — second KissKH integration via xyra.stream.
//!
//! Endpoints (xyra):
//!   GET /v1/kisskh/search?query=...     → { success, data: [{title, link, image, status, episode, subtitle}] }
//!   GET /v1/kisskh/info?id=<slug>       → { success, data: { title, episodes: [{number, title, sub, date, link}] } }
//!   GET /v1/kisskh/stream?id=&episode_id= → { success, data: { title, embed_url, episode_url } }
//!
//! The API doesn't expose a numeric `id` field. The show's id is the **last
//! path segment of the search `link`**, e.g.
//!   "https://kisskh.dk/series/flower-boy-2026/" → "flower-boy-2026"
//! The episode id is the **last path segment of the episode `link`**, e.g.
//!   "https://kisskh.dk/flower-boy-the-series-episode-8/" → "flower-boy-the-series-episode-8"
//!
//! Episodes have a `number` field (string, can be "8", "7", etc.) but the
//! numbers aren't always sequential. We match by link slug against a derived
//! `{series_slug}-episode-{N}` first, then fall back to index.
//!
//! Stream returns `embed_url` (a dramavideo.se iframe), not a direct m3u8.
//! The provider emits a StreamResult with `format: "iframe"` so the Flutter
//! player can deprioritize it (see `HueStream.isIframe` in `stream_remote.dart`).

use crate::models::StreamResult;
use anyhow::Result;
use reqwest_middleware::ClientWithMiddleware;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{info, warn};

const XYRA_BASE: &str = "https://api.xyra.stream/v1/kisskh";
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
pub struct KkXyraSearchItem {
    pub title: String,
    pub link: String,
    #[serde(default)]
    #[allow(dead_code)]
    pub image: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    pub status: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    pub episode: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    pub subtitle: Option<String>,
}

impl KkXyraSearchItem {
    /// Extract the show slug (last non-empty path segment of `link`).
    pub fn slug(&self) -> Option<String> {
        slug_from_link(&self.link)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KkXyraEpisode {
    pub number: String,
    pub title: String,
    pub link: String,
    #[serde(default)]
    #[allow(dead_code)]
    pub sub: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    pub date: Option<String>,
}

impl KkXyraEpisode {
    pub fn slug(&self) -> Option<String> {
        slug_from_link(&self.link)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct KkXyraDramaInfo {
    pub title: String,
    pub episodes: Vec<KkXyraEpisode>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct KkXyraStreamData {
    pub title: String,
    pub embed_url: String,
    #[serde(default)]
    #[allow(dead_code)]
    pub episode_url: Option<String>,
}

fn slug_from_link(link: &str) -> Option<String> {
    link.trim_end_matches('/')
        .rsplit('/')
        .next()
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
}

pub struct KissKhXyraClient {
    client: ClientWithMiddleware,
}

impl KissKhXyraClient {
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

        info!("[kisskh_xyra] GET {}", url);
        let resp = self.client.get(&url).send().await?;
        let status = resp.status();
        if !status.is_success() {
            warn!("[kisskh_xyra] returned {}", status);
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
    /// directly (`{"latest_release": [...]}`), others wrap in
    /// `{"success": true, "data": {rails}}`. Normalize to always return
    /// the inner rails object.
    async fn browse_request(
        &self,
        endpoint: &str,
        params: Vec<(&str, &str)>,
    ) -> Result<serde_json::Value> {
        let mut url = format!("{}{}?api_key={}", XYRA_BASE, endpoint, XYRA_KEY);
        for (k, v) in params {
            url.push_str(&format!("&{}={}", k, urlencoding::encode(v)));
        }

        info!("[kisskh_xyra] GET {}", url);
        let resp = self.client.get(&url).send().await?;
        let status = resp.status();
        if !status.is_success() {
            warn!("[kisskh_xyra] browse {} returned {}", endpoint, status);
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

    pub async fn search(&self, query: &str) -> Result<Vec<KkXyraSearchItem>> {
        let res = self
            .make_request::<Vec<KkXyraSearchItem>>("/search", vec![("query", query)])
            .await?;
        Ok(res.data.unwrap_or_default())
    }

    pub async fn get_info(&self, id: &str) -> Result<Option<KkXyraDramaInfo>> {
        let res = self
            .make_request::<KkXyraDramaInfo>("/info", vec![("id", id)])
            .await?;
        Ok(res.data)
    }

    pub async fn get_stream(
        &self,
        id: &str,
        episode_id: &str,
    ) -> Result<Option<KkXyraStreamData>> {
        let res = self
            .make_request::<KkXyraStreamData>(
                "/stream",
                vec![("id", id), ("episode_id", episode_id)],
            )
            .await?;
        Ok(res.data)
    }

    /// Browse homepage — returns the raw xyra payload
    /// (latest_release / popular_drama / recommendation).
    /// Pass-through JSON since Rust does not consume the individual rails.
    pub async fn home(&self) -> Result<serde_json::Value> {
        self.browse_request("/home", vec![]).await
    }

    /// Direct lookup by show slug — used by the drama route handler when
    /// the caller already knows the exact slug.
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
            warn!("[kisskh_xyra] no episodes for {}", show_id);
            return Ok(vec![]);
        }

        // Try common slug patterns: {show}-episode-N, {show}-ep-N
        let needle_title = format!("episode {}", episode);
        let needle_slug = format!("-episode-{}", episode);
        let needle_slug_short = format!("-ep-{}", episode);
        let ep_id = info
            .episodes
            .iter()
            .find(|e| e.link.to_lowercase().ends_with(&needle_slug))
            .or_else(|| {
                info.episodes
                    .iter()
                    .find(|e| e.link.to_lowercase().ends_with(&needle_slug_short))
            })
            .or_else(|| {
                info.episodes
                    .iter()
                    .find(|e| e.title.to_lowercase().contains(&needle_title))
            })
            .or_else(|| {
                let idx = (episode as usize).saturating_sub(1);
                info.episodes.get(idx)
            })
            .and_then(|e| e.slug());

        let ep_id = match ep_id {
            Some(id) => id,
            None => return Ok(vec![]),
        };
        info!("[kisskh_xyra] resolved episode_id={} for E{}", ep_id, episode);

        let data = match self.get_stream(show_id, &ep_id).await? {
            Some(d) => d,
            None => return Ok(vec![]),
        };

        let mut headers = HashMap::new();
        headers.insert("Referer".to_string(), "https://kisskh.dk/".to_string());
        headers.insert("User-Agent".to_string(), USER_AGENT.to_string());

        let mut extra = HashMap::new();
        extra.insert("iframe".to_string(), "true".to_string());
        extra.insert("episode_id".to_string(), ep_id.clone());
        extra.insert("series_id".to_string(), show_id.to_string());

        let result = StreamResult {
            url: data.embed_url,
            title: format!("Aurora - iframe - E{}", episode),
            quality: "Auto".to_string(),
            provider: "Aurora".to_string(),
            format: "iframe".to_string(),
            subtitles: None,
            headers: Some(headers),
            extra_metadata: Some(extra),
        };
        info!("[kisskh_xyra] returning 1 iframe stream");
        Ok(vec![result])
    }

    /// Search + info + stream fetch.
    /// `_season` is ignored — KissKH shows are flat.
    /// Episode matching tries the slug pattern `{series}-episode-{N}` first
    /// (most common case), then falls back to numeric match in the title,
    /// then to the (N-1)th index in the list.
    pub async fn get_streams_normalized(
        &self,
        title: &str,
        _season: u32,
        episode: u32,
    ) -> Result<Vec<StreamResult>> {
        info!("[kisskh_xyra] Searching for: \"{}\" E{}", title, episode);

        let search_results = self.search(title).await?;
        if search_results.is_empty() {
            return Ok(vec![]);
        }

        let best_match = &search_results[0];
        let show_id = match best_match.slug() {
            Some(s) => s,
            None => {
                warn!("[kisskh_xyra] no slug in best match link: {}", best_match.link);
                return Ok(vec![]);
            }
        };
        info!("[kisskh_xyra] best match: {} ({})", best_match.title, show_id);

        let info = match self.get_info(&show_id).await? {
            Some(i) => i,
            None => return Ok(vec![]),
        };

        if info.episodes.is_empty() {
            warn!("[kisskh_xyra] no episodes for {}", show_id);
            return Ok(vec![]);
        }

        // Try common slug patterns: {show}-episode-N, {show}-ep-N
        let needle_title = format!("episode {}", episode);
        let needle_slug = format!("-episode-{}", episode);
        let needle_slug_short = format!("-ep-{}", episode);
        let ep_id = info
            .episodes
            .iter()
            .find(|e| e.link.to_lowercase().ends_with(&needle_slug))
            .or_else(|| {
                info.episodes
                    .iter()
                    .find(|e| e.link.to_lowercase().ends_with(&needle_slug_short))
            })
            .or_else(|| {
                info.episodes
                    .iter()
                    .find(|e| e.title.to_lowercase().contains(&needle_title))
            })
            .or_else(|| {
                let idx = (episode as usize).saturating_sub(1);
                info.episodes.get(idx)
            })
            .and_then(|e| e.slug());

        let ep_id = match ep_id {
            Some(id) => id,
            None => return Ok(vec![]),
        };
        info!("[kisskh_xyra] resolved episode_id={} for E{}", ep_id, episode);

        let data = match self.get_stream(&show_id, &ep_id).await? {
            Some(d) => d,
            None => return Ok(vec![]),
        };

        // iframe stream — set headers and extra_metadata so the player
        // recognizes it. No direct m3u8 is available from this provider.
        let mut headers = HashMap::new();
        headers.insert("Referer".to_string(), "https://kisskh.dk/".to_string());
        headers.insert("User-Agent".to_string(), USER_AGENT.to_string());

        let mut extra = HashMap::new();
        extra.insert("iframe".to_string(), "true".to_string());
        extra.insert("episode_id".to_string(), ep_id.clone());
        extra.insert("series_id".to_string(), show_id.clone());

        let result = StreamResult {
            url: data.embed_url,
            title: format!("Aurora - iframe - E{}", episode),
            quality: "Auto".to_string(),
            provider: "Aurora".to_string(),
            format: "iframe".to_string(),
            subtitles: None,
            headers: Some(headers),
            extra_metadata: Some(extra),
        };
        info!("[kisskh_xyra] returning 1 iframe stream");
        Ok(vec![result])
    }
}
