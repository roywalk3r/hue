//! kissasian.top drama provider — direct MP4 streams + VTT subtitles.
//!
//! Endpoints (origin-locked: server enforces `Origin: https://kissasian.top`):
//!   GET /api/v1/dramas?page=N&type=kdrama|cdrama&country=KR|CN&per_page=N
//!     → { status, data: [{ id, title, slug, type, year, poster_url,
//!                          backdrop_url, total_episodes, rating, status,
//!                          view_count, is_pinned, created_at, updated_at }],
//!         meta: { current_page, last_page, per_page, total } }
//!   GET /api/v1/search?q=...
//!     → { status, data: [{ id, title, slug, type, year, ... }] }
//!   GET /api/v1/dramas/{slug}
//!     → { status, data: { id, title, slug, synopsis, year, rating, type,
//!                         country, status, total_episodes, poster_url,
//!                         backdrop_url, subtitle_languages, genres, cast,
//!                         episodes: [{ episode_number, url, subtitle_languages }] } }
//!   GET /api/v1/dramas/{slug}/episodes/{n}
//!     → { status, data: { episode_number, view_count, url,
//!                         subtitles: [{ language, code, url }],
//!                         navigation: { prev_episode, next_episode } } }
//!
//! Stream URL is direct MP4 on `hugh.cdn.rumble.cloud`. We do NOT need headers
//! today but send them anyway in case rumble.cloud starts enforcing Referer.

use crate::models::{StreamResult, Subtitle};
use anyhow::Result;
use reqwest_middleware::ClientWithMiddleware;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{info, warn};

const BASE_URL: &str = "https://kissasian.top";
// NOTE: do NOT send a full browser fingerprint. Cloudflare bot management
// returns 403 to over-spoofed requests from datacenter egress IPs and 200 to
// default reqwest clients. Keep headers minimal.
const USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/137.0.0.0 Safari/537.36";

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct KADrama {
    pub id: u64,
    pub title: String,
    pub slug: String,
    #[serde(rename = "type")]
    pub kind: Option<String>,
    pub year: Option<u32>,
    pub poster_url: Option<String>,
    pub backdrop_url: Option<String>,
    pub total_episodes: Option<u32>,
    pub rating: Option<f32>,
    pub status: Option<String>,
    pub view_count: Option<u64>,
    #[serde(default)]
    pub is_pinned: Option<bool>,
    #[serde(default)]
    pub country: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KAEpisodeBrief {
    pub episode_number: u32,
    pub url: String,
    #[serde(default)]
    pub subtitle_languages: Option<Vec<String>>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct KADramaInfo {
    pub id: u64,
    pub title: String,
    pub slug: String,
    pub synopsis: Option<String>,
    pub year: Option<u32>,
    pub rating: Option<f32>,
    #[serde(rename = "type")]
    pub kind: Option<String>,
    pub country: Option<String>,
    pub status: Option<String>,
    pub total_episodes: Option<u32>,
    pub poster_url: Option<String>,
    pub backdrop_url: Option<String>,
    pub subtitle_languages: Option<Vec<String>>,
    pub episodes: Vec<KAEpisodeBrief>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KASubtitle {
    pub language: String,
    pub code: String,
    pub url: String,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct KAEpisodeStream {
    pub episode_number: u32,
    pub url: String,
    pub subtitles: Vec<KASubtitle>,
}

#[derive(Debug, Deserialize)]
struct Envelope<T> {
    #[serde(default)]
    #[allow(dead_code)]
    status: String,
    #[serde(default)]
    data: Option<T>,
    #[serde(default)]
    #[allow(dead_code)]
    message: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct BrowseMeta {
    #[allow(dead_code)]
    current_page: u32,
    #[allow(dead_code)]
    last_page: u32,
    #[allow(dead_code)]
    per_page: u32,
    #[allow(dead_code)]
    total: u32,
}

pub struct KissAsianTopClient {
    client: ClientWithMiddleware,
}

impl KissAsianTopClient {
    pub fn new() -> Self {
        Self {
            client: crate::http_client::wrap_with_retry(
                reqwest::Client::builder()
                    .connect_timeout(std::time::Duration::from_secs(5))
                    .timeout(std::time::Duration::from_secs(15))
                    .user_agent(USER_AGENT)
                    .build()
                    .unwrap(),
            ),
        }
    }

    fn request(&self, method: reqwest::Method, path: &str) -> reqwest_middleware::RequestBuilder {
        self.client
            .request(method, format!("{}{}", BASE_URL, path))
            .header("User-Agent", USER_AGENT)
            .header("Accept", "application/json")
    }

    pub async fn browse(
        &self,
        page: u32,
        per_page: u32,
        kind: Option<&str>,
        country: Option<&str>,
    ) -> Result<Vec<KADrama>> {
        let mut params: Vec<(String, String)> = vec![
            ("page".into(), page.to_string()),
            ("per_page".into(), per_page.to_string()),
        ];
        if let Some(k) = kind {
            params.push(("type".into(), k.to_string()));
        }
        if let Some(c) = country {
            params.push(("country".into(), c.to_string()));
        }

        let mut req = self.request(reqwest::Method::GET, "/api/v1/dramas");
        for (k, v) in &params {
            req = req.query(&[(k.as_str(), v.as_str())]);
        }
        let resp = req.send().await?;
        let env: Envelope<serde_json::Value> = resp.json().await?;
        let data = env.data.unwrap_or(serde_json::Value::Null);
        // The endpoint returns either an array (when no meta) or {data:[...], meta:{...}}
        let list = if data.is_array() {
            data
        } else {
            data.get("data")
                .cloned()
                .unwrap_or(serde_json::Value::Array(vec![]))
        };
        let items: Vec<KADrama> = serde_json::from_value(list).unwrap_or_default();
        Ok(items)
    }

    pub async fn search(&self, q: &str) -> Result<Vec<KADrama>> {
        let resp = self
            .request(reqwest::Method::GET, "/api/v1/search")
            .query(&[("q", q)])
            .send()
            .await?;
        let env: Envelope<Vec<KADrama>> = resp.json().await?;
        Ok(env.data.unwrap_or_default())
    }

    pub async fn info(&self, slug: &str) -> Result<Option<KADramaInfo>> {
        let resp = self
            .request(reqwest::Method::GET, &format!("/api/v1/dramas/{}", slug))
            .send()
            .await?;
        let env: Envelope<KADramaInfo> = resp.json().await?;
        Ok(env.data)
    }

    pub async fn stream(&self, slug: &str, episode: u32) -> Result<Option<KAEpisodeStream>> {
        let resp = self
            .request(
                reqwest::Method::GET,
                &format!("/api/v1/dramas/{}/episodes/{}", slug, episode),
            )
            .send()
            .await?;
        let env: Envelope<KAEpisodeStream> = resp.json().await?;
        Ok(env.data)
    }

    /// Direct MP4 stream resolution — emits a single StreamResult with defensive
    /// headers and the episode's subtitles.
    pub async fn get_streams_normalized(
        &self,
        slug: &str,
        episode: u32,
    ) -> Result<Vec<StreamResult>> {
        info!("[Rose] streams: slug={} ep={}", slug, episode);

        let data = match self.stream(slug, episode).await? {
            Some(d) => d,
            None => {
                warn!("[Rose] no stream data for {} ep {}", slug, episode);
                return Ok(vec![]);
            }
        };

        let mut headers = HashMap::new();
        headers.insert("User-Agent".to_string(), USER_AGENT.to_string());

        let subtitles: Vec<Subtitle> = data
            .subtitles
            .into_iter()
            .map(|s| Subtitle {
                url: s.url,
                lang: s.code,
                label: s.language,
            })
            .collect();

        Ok(vec![StreamResult {
            url: data.url,
            title: format!("Rose - Episode {} - Auto", episode),
            quality: "Auto".to_string(),
            provider: "Rose".to_string(),
            format: "mp4".to_string(),
            subtitles: if subtitles.is_empty() {
                None
            } else {
                Some(subtitles)
            },
            headers: Some(headers),
            extra_metadata: None,
        }])
    }
}
