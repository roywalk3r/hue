//! Aurex (xyra.stream web-app "aurex-alpha" provider).
//!
//! The xyra.stream web app exposes a different code path from the official
//! `api.xyra.stream/v1/streamhub/streams` API. When the official API is
//! down (502s) this endpoint is usually still up and returns MP4 sources
//! through xyra's `proxy.xyra.stream/segment?url=...` proxy with the
//! upstream Referer/Origin headers already baked in.
//!
//! Endpoint:
//!   GET https://xyra.stream/api/stream/sources
//!     ?provider=aurex-alpha
//!     &tmdb_id={id}&title=...&year=...&imdb_id=...
//!     &kind=series|movie[&season=&episode=]
//!
//! Response:
//!   { success, provider, count, sources: [{quality, url}], subtitles: [{url, language, lang}] }
//!
//! Notes:
//!   * Requires browser-style headers (Referer, Origin, sec-fetch-*) —
//!     the endpoint gates on same-origin + browser nav, not just User-Agent.
//!   * Sources are already proxied by xyra; do NOT re-wrap them through
//!     `/ts-proxy` (the segment URL's `?url=...` query contains the
//!     inner URL + headers + sign token that our re-wrap would strip).
//!   * Subtitles are direct CloudFront-signed URLs that survive in the
//!     browser; xyra's CDN sets CORS correctly.

use crate::models::{StreamResult, Subtitle};
use anyhow::Result;
use reqwest_middleware::ClientWithMiddleware;
use serde::Deserialize;
use std::time::Duration;
use tracing::{info, warn};

const XYRA_WEB_BASE: &str = "https://xyra.stream/api/stream/sources";
const XYRA_AUREX_PROVIDER: &str = "aurex-alpha";
const USER_AGENT: &str =
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

#[derive(Debug, Deserialize)]
struct XyraAurexSource {
    url: String,
    #[serde(default)]
    quality: Option<String>,
}

#[derive(Debug, Deserialize)]
struct XyraAurexSubtitle {
    url: String,
    #[serde(default)]
    language: Option<String>,
    #[serde(default)]
    lang: Option<String>,
}

#[derive(Debug, Deserialize)]
struct XyraAurexResp {
    #[serde(default)]
    success: bool,
    #[serde(default)]
    sources: Vec<XyraAurexSource>,
    #[serde(default)]
    subtitles: Vec<XyraAurexSubtitle>,
}

pub struct XyraAurexClient {
    client: ClientWithMiddleware,
}

impl XyraAurexClient {
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

    pub async fn get_streams(
        &self,
        tmdb_id: u64,
        title: &str,
        year: Option<&str>,
        imdb_id: Option<&str>,
        media_type: &str,
        season: u32,
        episode: u32,
    ) -> Result<Vec<StreamResult>> {
        let kind = if media_type == "movie" { "movie" } else { "series" };

        let mut url = format!(
            "{XYRA_WEB_BASE}?provider={XYRA_AUREX_PROVIDER}&tmdb_id={tmdb_id}&title={}&kind={kind}",
            urlencoding::encode(title),
        );
        if let Some(y) = year {
            url.push_str(&format!("&year={}", urlencoding::encode(y)));
        }
        if let Some(imdb) = imdb_id {
            url.push_str(&format!("&imdb_id={}", urlencoding::encode(imdb)));
        }
        if kind == "series" {
            url.push_str(&format!("&season={season}&episode={episode}"));
        }

        // Build the page URL for the Referer header. Must point to a real
        // xyra.stream/p/... page on the same host, otherwise the request
        // is gated as cross-origin and returns 403.
        let referer = if kind == "movie" {
            format!("https://xyra.stream/p/movie/{tmdb_id}")
        } else {
            format!("https://xyra.stream/p/series/{tmdb_id}/{season}/{episode}")
        };

        info!("[xyra_aurex] GET {url}");

        let resp = self
            .client
            .get(&url)
            .header("Accept", "application/json, text/plain, */*")
            .header("Accept-Language", "en-US,en;q=0.9")
            .header("Referer", &referer)
            .header("Origin", "https://xyra.stream")
            .header("Sec-Fetch-Dest", "empty")
            .header("Sec-Fetch-Mode", "cors")
            .header("Sec-Fetch-Site", "same-origin")
            .send()
            .await?;

        let status = resp.status();
        if !status.is_success() {
            warn!("[xyra_aurex] returned {status}");
            return Ok(vec![]);
        }
        let body: XyraAurexResp = resp.json().await?;
        if !body.success {
            return Ok(vec![]);
        }

        let subtitles = if body.subtitles.is_empty() {
            None
        } else {
            Some(
                body.subtitles
                    .into_iter()
                    .map(|sub| Subtitle {
                        url: sub.url,
                        label: sub.language.unwrap_or_default(),
                        lang: sub.lang.unwrap_or_default(),
                    })
                    .collect(),
            )
        };

        Ok(body
            .sources
            .into_iter()
            .map(|s| StreamResult {
                // The proxy.xyra.stream/segment URL is already proxied; it
                // encodes the inner URL + headers + sign token in the query.
                // Mark as format=mp4 so wrap_with_proxy routes through the
                // whitelisted-raw path (added in is_whitelisted_raw_url).
                url: s.url,
                title: title.to_string(),
                quality: s.quality.unwrap_or_else(|| "Auto".to_string()),
                provider: "Aurex".to_string(),
                format: "mp4".to_string(),
                subtitles: subtitles.clone(),
                headers: None,
                extra_metadata: None,
            })
            .collect())
    }
}
