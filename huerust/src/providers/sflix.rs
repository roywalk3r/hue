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
use crate::models::{StreamResult, Subtitle};

const BASE_URL: &str = "https://sflix.film";
const API_BASE: &str = "https://h5-api.aoneroom.com/wefeed-h5api-bff";
const DEFAULT_USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/147.0.0.0 Safari/537.36";
const DEFAULT_COOKIE: &str = "sflix_token=%22eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJ1aWQiOjMxMTYwODUxODIwODQ5MjQ1MTIsImF0cCI6MywiZXh0IjoiMTc2ODA4NTkyNCIsImV4cCI6MTc3NTg2MTkyNCwiaWF0IjoxNzY4MDg1NjI0fQ.K6fuHWpTcLP0WjQx9ZFe6LSo_oNVNKiuC3MOA2HRw1s%22; account=2904345630854337384|0|H5|1768086238|; uuid=181f2237-fb79-4245-9a8a-2d5f0a60c4aa; sflix_i18n_lang=en";

pub struct SflixClient {
    client: Client,
    scraper_url: String,
}

impl SflixClient {
    pub fn new() -> Self {
        let scraper_url = std::env::var("MICROSERVICE_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:24610".to_string());
        Self {
            client: Client::builder()
                .user_agent(DEFAULT_USER_AGENT)
                .timeout(std::time::Duration::from_secs(30))
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
        let se = if media_type == "movie" { 0 } else { season };
        let ep = if media_type == "movie" { 0 } else { episode };

        let (type_path, type_query) = if media_type == "tv" {
            ("series", "/series/detail")
        } else {
            ("movies", "/movie/detail")
        };

        let referer_url = format!("{}/spa/videoPlayPage/{}/{}?id={}&type={}&lang=en",
            BASE_URL, type_path, slug, subject_id, urlencoding::encode(type_query));

        let play_url = format!("{}/wefeed-h5api-bff/subject/play?subjectId={}&se={}&ep={}&detailPath={}",
            BASE_URL, subject_id, se, ep, urlencoding::encode(slug));

        let mut headers = self.get_base_headers();
        headers.remove(CONTENT_TYPE); // GET should not send Content-Type
        headers.insert(REFERER, HeaderValue::from_str(&referer_url).unwrap());

        let resp = self.client.get(&play_url)
            .headers(headers)
            .send()
            .await?;

        let status = resp.status();
        let text = resp.text().await?;
        info!("[SFlix Rust] Play response status={} body={}", status, &text[..text.len().min(200)]);
        let data: serde_json::Value = serde_json::from_str(&text).unwrap_or_else(|e| {
            warn!("[SFlix Rust] Failed to parse play response: {}", e);
            serde_json::json!({})
        });
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

    pub async fn get_captions(&self, subject_id: &str, detail_path: &str, stream_id: &str) -> Result<Vec<Subtitle>> {
        let url = format!("{}/subject/caption?format=MP4&id={}&subjectId={}&detailPath={}",
            API_BASE, stream_id, subject_id, urlencoding::encode(detail_path));

        let resp = self.client.get(&url)
            .headers(self.get_base_headers())
            .send()
            .await?;

        let data: serde_json::Value = resp.json().await?;
        let captions = data["data"]["captions"].as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|c| {
                        let url = c["url"].as_str()?.to_string();
                        let lang = c["lan"].as_str().unwrap_or("en").to_string();
                        let label = c["lanName"].as_str().unwrap_or("English").to_string();
                        Some(Subtitle { url, lang, label })
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(captions)
    }

    pub async fn get_streams_normalized(&self, title: &str, media_type: &str, _year: Option<u32>, season: u32, episode: u32) -> Result<Vec<StreamResult>> {
        let year = _year;

        info!("[SFlix Rust] Searching for: \"{}\" ({}) S{} E{} (Year: {:?})", title, media_type, season, episode, year);
        
        let items = self.search(title, media_type).await?;
        if items.is_empty() { return Ok(vec![]); }

        // Fuzzy Matching Scoring
        let normalize = |s: &str| -> String {
            s.to_lowercase().chars().filter(|c| c.is_alphanumeric()).collect()
        };
        let target = normalize(title);
        let mut best_match = None;
        let mut best_score = -10000;

        for item in items {
            let item_title = item["title"].as_str().unwrap_or_default();
            let item_release = item["releaseDate"].as_str().unwrap_or_default();
            let item_year = item_release.split('-').next().and_then(|y| y.parse::<u32>().ok());
            
            // Clean title (remove S1, Season 1, (2024) etc)
            let re_season = regex::Regex::new(r"(?i)S\d+(-S\d+)?|Season \d+|\(\d{4}\)").unwrap();
            let clean_title = re_season.replace_all(item_title, "").trim().to_string();
            let norm_clean = normalize(&clean_title);
            
            let mut current_score = 0;
            
            // Year matching
            if let Some(y) = year {
                if let Some(iy) = item_year {
                    let diff = (y as i32 - iy as i32).abs();
                    if diff == 0 { current_score += 500; }
                    else if diff <= 1 { current_score += 200; }
                    else { current_score -= 5000; }
                } else {
                    current_score -= 50;
                }
            }

            // Title matching
            if norm_clean == target {
                current_score += 1000;
            }

            // Season matching for TV
            if media_type == "tv" && season > 0 {
                let season_str = format!("S{}", season);
                let season_word = format!("Season {}", season);
                if item_title.contains(&season_str) || item_title.contains(&season_word) {
                    current_score += 100;
                }
            }

            if current_score > best_score {
                best_score = current_score;
                best_match = Some(item);
            }
        }

        let best_match = match best_match {
            Some(m) if best_score > 0 => m,
            _ => return Ok(vec![]),
        };

        let slug = best_match["detailPath"].as_str().unwrap_or_default();
        let subject_id = best_match["subjectId"].as_str().unwrap_or_default();

        match self.get_streams(slug, subject_id, media_type, season, episode).await? {
            Some(data) => {
                let mut results = Vec::new();
                let mut headers = HashMap::new();
                headers.insert("Referer".to_string(), format!("{}/", BASE_URL));
                headers.insert("User-Agent".to_string(), DEFAULT_USER_AGENT.to_string());

                // Fetch captions using the first available stream id
                let first_stream_id = data.streams.as_ref()
                    .and_then(|s| s.first())
                    .and_then(|s| s.id.as_ref())
                    .or_else(|| data.hls.as_ref().and_then(|h| h.first()).and_then(|s| s.id.as_ref()))
                    .cloned();

                let subtitles = if let Some(stream_id) = first_stream_id {
                    match self.get_captions(subject_id, slug, &stream_id).await {
                        Ok(subs) if !subs.is_empty() => {
                            info!("[SFlix Rust] Fetched {} captions", subs.len());
                            Some(subs)
                        }
                        Ok(_) => None,
                        Err(e) => {
                            warn!("[SFlix Rust] Caption fetch failed: {}", e);
                            None
                        }
                    }
                } else {
                    None
                };

                if let Some(streams) = data.streams {
                    for s in streams {
                        results.push(StreamResult {
                            url: s.url,
                            title: format!("Nova - {}p (MP4)", s.resolutions.unwrap_or_else(|| "Auto".to_string())),
                            quality: s.quality.unwrap_or_else(|| "Auto".to_string()),
                            provider: "Nova".to_string(),
                            format: "mp4".to_string(),
                            subtitles: subtitles.clone(),
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
                            subtitles: subtitles.clone(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_sflix_the_boys() {
        let client = SflixClient::new();
        let results = client.get_streams_normalized("The Boys", "tv", Some(2019), 1, 1).await.unwrap();
        println!("SFlix Results: {:?}", results);
        assert!(!results.is_empty());
    }
}
