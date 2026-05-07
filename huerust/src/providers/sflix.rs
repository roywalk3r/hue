//! Hue Stream API
//! Professional Streaming Aggregator
//!
//! @author roywalk3r <roywalker440@gmail.com>
//! @repository https://github.com/roywalk3r/hue.git
//! @license MIT

use serde::Deserialize;
use reqwest::{Client, header::{HeaderMap, HeaderValue, REFERER, CONTENT_TYPE}};
use anyhow::Result;
use std::collections::HashMap;
use tracing::{info, warn};
use crate::moviebox::StreamData;
use crate::models::StreamResult;

const BASE_URL: &str = "https://sflix.film";
const API_BASE: &str = "https://h5-api.aoneroom.com/wefeed-h5api-bff";
const DEFAULT_USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";
const DEFAULT_COOKIE: &str = "sflix_token=%22eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJ1aWQiOjMxMTYwODUxODIwODQ5MjQ1MTIsImF0cCI6MywiZXh0IjoiMTc2ODA4NTkyNCIsImV4cCI6MTc3NTg2MTkyNCwiaWF0IjoxNzY4MDg1NjI0fQ.K6fuHWpTcLP0WjQx9ZFe6LSo_oNVNKiuC3MOA2HRw1s%22; account=2904345630854337384|0|H5|1768086238|; uuid=181f2237-fb79-4245-9a8a-2d5f0a60c4aa; sflix_i18n_lang=en";

pub struct SflixClient {
    client: Client,
    scraper_url: String,
}

impl SflixClient {
    pub fn new() -> Self {
        let scraper_url = std::env::var("MICROSERVICE_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:24610".to_string());
        // Warn once at startup if the fallback credential is expired
        let cookie = crate::user_config::HueUserConfig::get_live()
            .sflix_cookie_or_env(DEFAULT_COOKIE);
        if cookie == DEFAULT_COOKIE && crate::security::jwt_is_expired(DEFAULT_COOKIE) {
            warn!("[SFlix] Default JWT is expired. Set sflix_cookie in user-config.json to restore functionality.");
        }
        Self {
            client: Client::builder()
                .user_agent(DEFAULT_USER_AGENT)
                .timeout(std::time::Duration::from_secs(30))
                .connect_timeout(std::time::Duration::from_secs(10))
                .tcp_keepalive(std::time::Duration::from_secs(60))
                .pool_max_idle_per_host(2)
                .pool_idle_timeout(std::time::Duration::from_secs(90))
                .build()
                .unwrap(),
            scraper_url,
        }
    }

    fn get_base_headers(&self) -> HeaderMap {
        let cookie = crate::user_config::HueUserConfig::get_live()
            .sflix_cookie_or_env(DEFAULT_COOKIE);
        let mut headers = HeaderMap::new();
        headers.insert("Accept", HeaderValue::from_static("application/json"));
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(REFERER, HeaderValue::from_static("https://sflix.film/"));
        if let Ok(v) = HeaderValue::from_str(&cookie) {
            headers.insert("Cookie", v);
        }
        headers
    }

    pub async fn search(&self, keyword: &str, media_type: &str) -> Result<Vec<serde_json::Value>> {
        let url = format!("{}/subject/search", API_BASE);
        let subject_type = if media_type == "tv" { 2 } else { 1 };
        
        let payload = serde_json::json!({
            "keyword": keyword,
            "page": "1",
            "perPage": 24,
            "subjectType": subject_type
        });

        let resp = self.client.post(&url)
            .headers(self.get_base_headers())
            .json(&payload)
            .send()
            .await?;

        let data: serde_json::Value = resp.json().await?;
        let items = data["data"]["items"].as_array().cloned().unwrap_or_default();
        Ok(items)
    }

    pub async fn get_streams(&self, slug: &str, subject_id: &str, media_type: &str, season: u32, episode: u32) -> Result<Option<StreamData>> {
        let se = season;
        let ep = episode;

        let (type_path, type_query) = if media_type == "tv" {
            ("series", "/series/detail")
        } else {
            ("movies", "/movie/detail")
        };

        let referer_url = format!("{}/spa/videoPlayPage/{}/{}?id={}&type={}&lang=en",
            BASE_URL, type_path, slug, subject_id, urlencoding::encode(type_query));

        let play_url = format!("{}/wefeed-h5api-bff/subject/play?subjectId={}&se={}&ep={}&detailPath={}", 
            BASE_URL, subject_id, se, ep, slug);

        let mut headers = self.get_base_headers();
        headers.insert(REFERER, HeaderValue::from_str(&referer_url).unwrap());

        let resp = self.client.get(&play_url)
            .headers(headers)
            .send()
            .await?;

        let data: serde_json::Value = resp.json().await?;
        if data["code"] == 0 {
            let parsed: Option<StreamData> = serde_json::from_value(data["data"].clone()).ok();
            // Fallback to scraper if API returns empty streams
            if let Some(ref sd) = parsed {
                let has_streams = sd.streams.as_ref().map(|v| !v.is_empty()).unwrap_or(false);
                let has_hls = sd.hls.as_ref().map(|v| !v.is_empty()).unwrap_or(false);
                if !has_streams && !has_hls {
                    return Ok(None);
                }
            }
            return Ok(parsed);
        }

        Ok(None)
    }

    pub async fn get_streams_normalized(&self, title: &str, media_type: &str, _year: Option<u32>, season: u32, episode: u32) -> Result<Vec<StreamResult>> {
        info!("[SFlix Rust] Searching for: \"{}\" ({}) S{} E{}", title, media_type, season, episode);
        
        let items = self.search(title, media_type).await?;
        if items.is_empty() { return Ok(vec![]); }

        let best_match = &items[0];
        let slug = best_match["detailPath"].as_str().unwrap_or_default();
        let subject_id = best_match["subjectId"].as_str().unwrap_or_default();

        match self.get_streams(slug, subject_id, media_type, season, episode).await? {
            Some(data) => {
                let mut results = Vec::new();
                let mut headers = HashMap::new();
                headers.insert("Referer".to_string(), format!("{}/", BASE_URL));
                headers.insert("User-Agent".to_string(), DEFAULT_USER_AGENT.to_string());

                if let Some(streams) = data.streams {
                    for s in streams {
                        results.push(StreamResult {
                            url: s.url,
                            title: format!("Nova - {}p (MP4)", s.resolutions.unwrap_or_else(|| "Auto".to_string())),
                            quality: s.quality.unwrap_or_else(|| "Auto".to_string()),
                            provider: "Nova".to_string(),
                            format: "mp4".to_string(),
                            subtitles: None,
                            headers: Some(headers.clone()),
                            extra_metadata: None,
                        });
                    }
                }

                if let Some(hls) = data.hls {
                    for s in hls {
                        results.push(StreamResult {
                            url: s.url,
                            title: "Nova - Auto (HLS)".to_string(),
                            quality: "Auto".to_string(),
                            provider: "Nova".to_string(),
                            format: "hls".to_string(),
                            subtitles: None,
                            headers: Some(headers.clone()),
                            extra_metadata: None,
                        });
                    }
                }
                Ok(results)
            }
            None => {
                info!("[SFlix Rust] API fast-path failed, delegating to Node.js Scraper Microservice...");
                let (type_path, type_query) = if media_type == "tv" {
                    ("series", "/series/detail")
                } else {
                    ("movies", "/movie/detail")
                };
                let detail_url = format!("{}/spa/videoPlayPage/{}/{}?id={}&type={}&lang=en",
                    BASE_URL, type_path, slug, subject_id, urlencoding::encode(type_query));
                
                let is_movie = media_type == "movie";
                
                let payload = serde_json::json!({
                    "url": detail_url,
                    "isMovie": is_movie,
                    "season": season,
                    "episode": episode
                });

                match self.client.post(format!("{}/scrape/sflix", self.scraper_url))
                    .json(&payload)
                    .send()
                    .await {
                        Ok(scraper_resp) => {
                            if scraper_resp.status().is_success() {
                                #[derive(Deserialize)]
                                struct ScraperResponse {
                                    success: bool,
                                    streams: Vec<StreamResult>,
                                }
                                if let Ok(data) = scraper_resp.json::<ScraperResponse>().await {
                                    if data.success {
                                        return Ok(data.streams);
                                    }
                                }
                            }
                        },
                        Err(e) => warn!("[SFlix Rust] Scraper microservice call failed: {}", e),
                    }
                
                Ok(vec![])
            }
        }
    }
}
