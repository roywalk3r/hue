//! Hue Stream API
//! Professional Streaming Aggregator
//!
//! @author roywalk3r <roywalker440@gmail.com>
//! @repository https://github.com/roywalk3r/hue.git
//! @license MIT

use serde::{Deserialize, Serialize};
use reqwest::{Client, header::{HeaderMap, HeaderValue, REFERER, USER_AGENT, ORIGIN, COOKIE, AUTHORIZATION}};
use anyhow::{Result, anyhow};
use std::collections::HashMap;
use tracing::{info, warn};
use crate::models::StreamResult;

const BASE_URL: &str = "https://h5-api.aoneroom.com/wefeed-h5api-bff";
const DEFAULT_X_USER: &str = r#"{"token":"eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJ1aWQiOjc1NjEyMzU2Nzc1NjA4NTMyODAsImF0cCI6MywiZXh0IjoiMTc3MTE0NDQ4MiIsImV4cCI6MTc3ODkyMDQ4MiwiaWF0IjoxNzcxMTQxNDk1fQ.BF7r1K_npeKWjkjuIxqZMTvUDxJ8oBjvSBc3glWcqtU","userId":"7561235677560853280","userType":0,"appType":2}"#;
const DEFAULT_COOKIE: &str = "uuid=202d061a-fbe4-443c-aa09-be0232791ef7";
const DEFAULT_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MovieBoxItem {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub cover: Option<String>,
    pub rating: Option<f64>,
    #[serde(rename = "releaseDate")]
    pub release_date: Option<String>,
    #[serde(rename = "detailPath")]
    pub detail_path: Option<String>,
    pub genre: Option<String>,
    #[serde(rename = "type")]
    pub media_type: String,
    #[serde(rename = "subjectType")]
    pub subject_type: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StreamData {
    pub streams: Option<Vec<MovieBoxStream>>,
    pub hls: Option<Vec<MovieBoxStream>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MovieBoxStream {
    pub url: String,
    pub resolutions: Option<String>,
    pub quality: Option<String>,
    pub format: Option<String>,
    pub id: Option<String>,
}

pub struct MovieBoxClient {
    client: Client,
    host: String,
}

impl MovieBoxClient {
    pub fn new(host: Option<String>) -> Self {
        let default_host = host.unwrap_or_else(|| "moviebox.ph".to_string());
        let uc = crate::user_config::HueUserConfig::get_live();
        // Warn once at startup if the fallback credential is expired
        let x_user = uc.moviebox_x_user_or_env(DEFAULT_X_USER);
        if x_user == DEFAULT_X_USER && crate::security::jwt_is_expired(DEFAULT_X_USER) {
            warn!("[MovieBox] Default JWT is expired. Set moviebox_x_user in user-config.json to restore functionality.");
        }
        let host = uc.moviebox_host_or_env(&default_host);
        Self {
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(45))
                .connect_timeout(std::time::Duration::from_secs(10))
                .tcp_keepalive(std::time::Duration::from_secs(60))
                .pool_max_idle_per_host(4)
                .pool_idle_timeout(std::time::Duration::from_secs(90))
                .build()
                .unwrap(),
            host,
        }
    }

    fn get_base_headers(&self) -> HeaderMap {
        let uc = crate::user_config::HueUserConfig::get_live();
        let x_user = uc.moviebox_x_user_or_env(DEFAULT_X_USER);
        let cookie = uc.moviebox_cookie_or_env(DEFAULT_COOKIE);
        let ua = uc.moviebox_user_agent.as_deref().unwrap_or(DEFAULT_UA);

        let mut headers = HeaderMap::new();
        headers.insert("Accept", HeaderValue::from_static("application/json"));
        headers.insert(USER_AGENT, HeaderValue::from_str(ua).unwrap_or_else(|_| HeaderValue::from_static(DEFAULT_UA)));
        headers.insert(ORIGIN, HeaderValue::from_str(&format!("https://{}", self.host)).unwrap());
        if let Ok(v) = HeaderValue::from_str(&x_user) {
            headers.insert("x-user", v);
        }
        headers.insert("x-client-info", HeaderValue::from_static(r#"{"timezone":"Europe/Berlin"}"#));
        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&x_user) {
            if let Some(token) = parsed.get("token").and_then(|t| t.as_str()) {
                if let Ok(v) = HeaderValue::from_str(&format!("Bearer {}", token)) {
                    headers.insert(AUTHORIZATION, v);
                }
            }
        }
        if let Ok(v) = HeaderValue::from_str(&cookie) {
            headers.insert(COOKIE, v);
        }
        headers
    }

    pub async fn search(&self, keyword: &str, page: u32, subject_type: u32) -> Result<Vec<MovieBoxItem>> {
        let url = format!("{}/subject/search", BASE_URL);
        let payload = serde_json::json!({
            "keyword": keyword,
            "page": page,
            "perPage": 28,
            "subjectType": subject_type
        });

        let resp = self.client.post(&url)
            .headers(self.get_base_headers())
            .json(&payload)
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(anyhow!("MovieBox API error: {}", resp.status()));
        }

        let data: serde_json::Value = resp.json().await?;
        if data["code"] != 0 {
            return Ok(vec![]);
        }

        let items = data["data"]["items"].as_array()
            .or_else(|| data["data"]["subjectList"].as_array())
            .map(|arr| arr.iter().filter_map(|v| self.map_item(v)).collect())
            .unwrap_or_default();

        Ok(items)
    }

    fn map_item(&self, v: &serde_json::Value) -> Option<MovieBoxItem> {
        let subject_type = v["subjectType"].as_u64()? as u32;
        let media_type = if subject_type == 1 { "movie" } else { "tv" };

        Some(MovieBoxItem {
            id: v["subjectId"].as_str()?.to_string(),
            title: v["title"].as_str()?.to_string(),
            description: v["description"].as_str().map(String::from),
            cover: v["cover"]["url"].as_str().map(String::from),
            rating: v["imdbRatingValue"].as_f64(),
            release_date: v["releaseDate"].as_str().map(String::from),
            detail_path: v["detailPath"].as_str().map(String::from),
            genre: v["genre"].as_str().map(String::from),
            media_type: media_type.to_string(),
            subject_type,
        })
    }

    pub async fn get_streams(&self, subject_id: &str, detail_path: &str, season: u32, episode: u32) -> Result<Option<StreamData>> {
        let is_movie = season == 0 && episode == 0;
        let type_label = if is_movie { "movie" } else { "series" };

        // Race all host×timezone combos simultaneously — return the first successful response.
        let hosts: &[&str] = &["moviebox.ph", "movibox.net", "123movienow.cc", "moviebox.to"];
        let timezones: &[&str] = &["Pacific/Pago_Pago", "Asia/Hong_Kong", "Europe/London", "America/New_York"];

        let mut set = tokio::task::JoinSet::new();

        for (h_idx, &host) in hosts.iter().enumerate() {
            let max_tz = if h_idx == 0 { 1 } else { 2 };
            for &tz in timezones.iter().take(max_tz) {
                let client = self.client.clone();
                let base_headers = self.get_base_headers();
                let subject_id = subject_id.to_string();
                let detail_path = detail_path.to_string();
                let type_label = type_label.to_string();
                let host = host.to_string();
                let tz = tz.to_string();
                let season = season;
                let episode = episode;

                set.spawn(async move {
                    let play_url = format!("https://{}/wefeed-h5api-bff/subject/play", host);

                    let mut referer_path = detail_path.clone();
                    if referer_path.starts_with('/') { referer_path.remove(0); }
                    referer_path = referer_path.replace("movies/detail/", "movies/").replace("series/detail/", "series/");

                    let referer = format!(
                        "https://{}/spa/videoPlayPage/{}?id={}&type=/{}/detail&detailSe=&detailEp=&lang=en",
                        host, referer_path, subject_id, type_label
                    );
                    let url = format!("{}?subjectId={}&se={}&ep={}&detailPath={}",
                        play_url, subject_id, season, episode, urlencoding::encode(&detail_path));

                    let mut headers = base_headers;
                    if let Ok(v) = HeaderValue::from_str(&referer) { headers.insert(REFERER, v); }
                    if let Ok(v) = HeaderValue::from_str(&format!(r#"{{"timezone":"{}"}}"#, tz)) {
                        headers.insert("x-client-info", v);
                    }

                    match client.get(&url).headers(headers).send().await {
                        Ok(resp) => {
                            let text = resp.text().await.unwrap_or_default();
                            if text.contains("<!DOCTYPE") { return None; }
                            let data: serde_json::Value = serde_json::from_str(&text).ok()?;
                            if data["code"] == 0 {
                                return serde_json::from_value(data["data"].clone()).ok();
                            }
                            if data["message"] == "invalid region" {
                                warn!("[MovieBox Rust] Region blocked on {}", host);
                            }
                            None
                        }
                        Err(_) => None,
                    }
                });
            }
        }

        while let Some(result) = set.join_next().await {
            if let Ok(Some(data)) = result {
                set.abort_all();
                return Ok(Some(data));
            }
        }

        Ok(None)
    }

    pub async fn get_streams_normalized(&self, title: &str, media_type: &str, year: Option<u32>, season: u32, episode: u32) -> Result<Vec<StreamResult>> {
        info!("[MovieBox Rust] Searching for: \"{}\" ({:?}) S{} E{}", title, year, season, episode);

        let subject_type = if media_type == "movie" { 1 } else { 2 };
        let items = self.search(title, 1, subject_type).await?;
        if items.is_empty() {
            info!("[MovieBox Rust] No search results for: \"{}\"", title);
            return Ok(vec![]);
        }

        // Find best match by title + year + subjectType (same logic as Node.js)
        let target_type = if media_type == "movie" { 1 } else { 2 };
        let normalized_title = title.to_lowercase().replace(|c: char| !c.is_alphanumeric(), "");

        let best_match = items.iter().find(|item| {
            let item_title = item.title.to_lowercase().replace(|c: char| !c.is_alphanumeric(), "");
            let item_year = item.release_date.as_ref().and_then(|d| d.split('-').next()).and_then(|y| y.parse::<u32>().ok());
            let title_match = item_title == normalized_title;
            let year_match = year.map_or(true, |y| item_year == Some(y));
            let type_match = item.subject_type == target_type;
            title_match && year_match && type_match
        }).or_else(|| {
            items.iter().find(|item| {
                let item_title = item.title.to_lowercase().replace(|c: char| !c.is_alphanumeric(), "");
                let type_match = item.subject_type == target_type;
                item_title == normalized_title && type_match
            })
        }).or_else(|| {
            items.first()
        });

        let best_match = match best_match {
            Some(m) => m,
            None => return Ok(vec![]),
        };

        info!(
            "[MovieBox Rust] Selected match: \"{}\" ({}) path: {:?}",
            best_match.title, best_match.id, best_match.detail_path
        );

        let se = if media_type == "movie" { 0 } else { season };
        let ep = if media_type == "movie" { 0 } else { episode };
        let detail_path = best_match.detail_path.clone().unwrap_or_default();

        match self.get_streams(&best_match.id, &detail_path, se, ep).await? {
            Some(data) => {
                let mut results = Vec::new();
                let mut mb_headers = HashMap::new();
                mb_headers.insert("Origin".to_string(), format!("https://{}", self.host));
                mb_headers.insert("x-user".to_string(), crate::user_config::HueUserConfig::get_live().moviebox_x_user_or_env(DEFAULT_X_USER));
                mb_headers.insert("Referer".to_string(), format!("https://{}/", self.host));

                if let Some(streams) = data.streams {
                    for s in streams {
                        results.push(StreamResult {
                            url: s.url,
                            title: format!("Cinema - {}p (MP4)", s.resolutions.as_deref().unwrap_or("Auto")),
                            quality: s.quality.as_deref().unwrap_or("Auto").to_string(),
                            provider: "Cinema".to_string(),
                            format: "mp4".to_string(),
                            subtitles: None,
                            headers: Some(mb_headers.clone()),
                            extra_metadata: None,
                        });
                    }
                }

                if let Some(hls) = data.hls {
                    for s in hls {
                        results.push(StreamResult {
                            url: s.url,
                            title: "Cinema - Auto (HLS)".to_string(),
                            quality: "Auto".to_string(),
                            provider: "Cinema".to_string(),
                            format: "hls".to_string(),
                            subtitles: None,
                            headers: Some(mb_headers.clone()),
                            extra_metadata: None,
                        });
                    }
                }
                Ok(results)
            }
            None => Ok(vec![]),
        }
    }

    pub async fn filter(&self, page: u32, per_page: u32, country: Option<&str>, sort: Option<&str>) -> Result<Vec<MovieBoxItem>> {
        let url = format!("{}/subject/filter", BASE_URL);
        let payload = serde_json::json!({
            "page": page,
            "perPage": per_page,
            "channelId": 2,
            "genre": "All",
            "country": country.unwrap_or("All"),
            "year": "All",
            "sort": sort.unwrap_or("ForYou"),
            "tabId": 0,
            "classify": "All"
        });

        let resp = self.client.post(&url)
            .headers(self.get_base_headers())
            .json(&payload)
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(anyhow!("MovieBox filter error: {}", resp.status()));
        }

        let data: serde_json::Value = resp.json().await?;
        if data["code"] != 0 {
            return Ok(vec![]);
        }

        let items = data["data"]["items"].as_array()
            .or_else(|| data["data"]["subjectList"].as_array())
            .map(|arr| arr.iter().filter_map(|v| self.map_item(v)).collect())
            .unwrap_or_default();

        Ok(items)
    }

    pub async fn browse_rails(&self, page: u32, per_page: u32, country: Option<&str>, sort: Option<&str>) -> Result<Vec<serde_json::Value>> {
        if let Some(c) = country {
            let items = self.filter(page, per_page, Some(c), sort).await?;
            let rail = serde_json::json!({
                "country": c,
                "items": items
            });
            return Ok(vec![rail]);
        }

        let countries = vec!["Korea", "China", "Japan", "Thailand"];
        let mut rails = Vec::new();

        for c in countries {
            match self.filter(page, per_page, Some(c), sort).await {
                Ok(items) if !items.is_empty() => {
                    rails.push(serde_json::json!({
                        "country": c,
                        "items": items
                    }));
                }
                _ => {}
            }
        }

        Ok(rails)
    }

    pub async fn get_detail(&self, detail_path: &str) -> Result<serde_json::Value> {
        let url = format!("{}/detail?detailPath={}", BASE_URL, urlencoding::encode(detail_path));
        let resp = self.client.get(&url)
            .headers(self.get_base_headers())
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(anyhow!("MovieBox detail error: {}", resp.status()));
        }

        let data: serde_json::Value = resp.json().await?;
        Ok(data)
    }

    // ── Browse / Discovery ───────────────────────────────────────────────────

    pub async fn get_home(&self) -> Result<serde_json::Value> {
        let url = format!("{}/home?host={}", BASE_URL, self.host);
        let resp = self.client.get(&url)
            .headers(self.get_base_headers())
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(anyhow!("MovieBox home error: {}", resp.status()));
        }

        let data: serde_json::Value = resp.json().await?;
        if data["code"] != 0 {
            return Ok(serde_json::json!({ "platforms": [], "sections": [] }));
        }

        let inner = &data["data"];
        let sections = inner["operatingList"].as_array()
            .map(|arr| arr.iter().filter_map(|s| {
                let title = s["title"].as_str()?;
                let items = s["subjects"].as_array()
                    .or_else(|| s["subjectList"].as_array())
                    .map(|list| list.iter().filter_map(|v| self.map_item(v)).collect::<Vec<_>>())
                    .unwrap_or_default();
                if items.is_empty() { return None; }
                Some(serde_json::json!({ "title": title, "items": items }))
            }).collect::<Vec<_>>())
            .unwrap_or_default();

        let platforms = inner["platformList"].as_array()
            .map(|arr| arr.iter().cloned().collect::<Vec<_>>())
            .unwrap_or_default();

        Ok(serde_json::json!({ "platforms": platforms, "sections": sections }))
    }

    pub async fn get_trending(&self, page: u32) -> Result<Vec<MovieBoxItem>> {
        let url = format!("{}/subject/trending?page={}&perPage=18", BASE_URL, page);
        let resp = self.client.get(&url)
            .headers(self.get_base_headers())
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(anyhow!("MovieBox trending error: {}", resp.status()));
        }

        let data: serde_json::Value = resp.json().await?;
        if data["code"] != 0 {
            return Ok(vec![]);
        }

        let items = data["data"]["items"].as_array()
            .or_else(|| data["data"]["subjectList"].as_array())
            .map(|arr| arr.iter().filter_map(|v| self.map_item(v)).collect())
            .unwrap_or_default();

        Ok(items)
    }

    pub async fn get_section(&self, title_query: &str) -> Result<Vec<MovieBoxItem>> {
        let home = self.get_home().await?;
        let sections = home["sections"].as_array().unwrap_or(&Vec::new()).clone();
        let q = title_query.to_lowercase();
        for s in sections {
            if let Some(title) = s["title"].as_str() {
                if title.to_lowercase().contains(&q) {
                    return Ok(s["items"].as_array()
                        .map(|arr| arr.iter().filter_map(|v| serde_json::from_value(v.clone()).ok()).collect())
                        .unwrap_or_default());
                }
            }
        }
        Ok(vec![])
    }

    pub async fn get_discover(&self) -> Result<Vec<serde_json::Value>> {
        let home = self.get_home().await?;
        let sections = home["sections"].as_array().cloned().unwrap_or_default();

        let targets: &[&str] = &[
            "Popular Series", "Popular Movie", "Hot Short TV", "SA Drama",
            "Thai-Drama", "Teen Romance", "Turkish Drama", "C-Drama",
            "K-Drama", "Western TV", "Movie/TV Series", "Must-watch Black Shows",
        ];

        let matched: Vec<serde_json::Value> = sections.into_iter()
            .filter(|s| {
                if let Some(t) = s["title"].as_str() {
                    let tl = t.to_lowercase();
                    targets.iter().any(|target| tl.contains(&target.to_lowercase()))
                } else { false }
            })
            .collect();

        if matched.len() > 1 {
            Ok(matched)
        } else {
            Ok(home["sections"].as_array().cloned().unwrap_or_default())
        }
    }

    pub async fn get_indian_content(&self, media_type: &str, page: u32) -> Result<Vec<MovieBoxItem>> {
        let channel_id = if media_type == "movie" { 1 } else { 2 };
        self.filter_india(page, 28, channel_id).await
    }

    async fn filter_india(&self, page: u32, per_page: u32, channel_id: u32) -> Result<Vec<MovieBoxItem>> {
        let url = format!("{}/subject/filter", BASE_URL);
        let payload = serde_json::json!({
            "page": page,
            "perPage": per_page,
            "channelId": channel_id,
            "genre": "All",
            "country": "India",
            "year": "All",
            "sort": "ForYou",
            "tabId": 0,
            "classify": "All"
        });

        let resp = self.client.post(&url)
            .headers(self.get_base_headers())
            .json(&payload)
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(anyhow!("MovieBox India filter error: {}", resp.status()));
        }

        let data: serde_json::Value = resp.json().await?;
        if data["code"] != 0 {
            return Ok(vec![]);
        }

        let items = data["data"]["items"].as_array()
            .or_else(|| data["data"]["subjectList"].as_array())
            .map(|arr| arr.iter().filter_map(|v| self.map_item(v)).collect())
            .unwrap_or_default();

        Ok(items)
    }

    pub async fn get_shorts(&self, page: u32) -> Result<Vec<MovieBoxItem>> {
        let url = format!("{}/subject/filter", BASE_URL);
        let payload = serde_json::json!({
            "page": page,
            "perPage": 28,
            "channelId": 7,
            "genre": "All",
            "country": "All",
            "year": "All",
            "sort": "ForYou",
            "tabId": 0,
            "classify": "All"
        });

        let resp = self.client.post(&url)
            .headers(self.get_base_headers())
            .json(&payload)
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(anyhow!("MovieBox shorts error: {}", resp.status()));
        }

        let data: serde_json::Value = resp.json().await?;
        if data["code"] != 0 {
            return Ok(vec![]);
        }

        let items = data["data"]["items"].as_array()
            .or_else(|| data["data"]["subjectList"].as_array())
            .map(|arr| arr.iter().filter_map(|v| self.map_item(v)).collect())
            .unwrap_or_default();

        Ok(items)
    }
}
