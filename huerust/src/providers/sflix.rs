use serde::{Deserialize, Serialize};
use reqwest::{Client, header::{HeaderMap, HeaderValue, REFERER, CONTENT_TYPE}};
use anyhow::{Result, anyhow};
use std::collections::HashMap;
use tracing::{info, warn};
use crate::moviebox::{StreamData, MovieBoxStream};
use crate::models::{StreamResult, Subtitle};

const BASE_URL: &str = "https://sflix.film";
const API_BASE: &str = "https://h5-api.aoneroom.com/wefeed-h5api-bff";
const DEFAULT_USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";
const DEFAULT_COOKIE: &str = "sflix_token=%22eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJ1aWQiOjMxMTYwODUxODIwODQ5MjQ1MTIsImF0cCI6MywiZXh0IjoiMTc2ODA4NTkyNCIsImV4cCI6MTc3NTg2MTkyNCwiaWF0IjoxNzY4MDg1NjI0fQ.K6fuHWpTcLP0WjQx9ZFe6LSo_oNVNKiuC3MOA2HRw1s%22; account=2904345630854337384|0|H5|1768086238|; uuid=181f2237-fb79-4245-9a8a-2d5f0a60c4aa; sflix_i18n_lang=en";

pub struct SflixClient {
    client: Client,
}

impl SflixClient {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .user_agent(DEFAULT_USER_AGENT)
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap(),
        }
    }

    fn get_base_headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert("Accept", HeaderValue::from_static("application/json"));
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(REFERER, HeaderValue::from_static("https://sflix.film/"));
        headers.insert("Cookie", HeaderValue::from_static(DEFAULT_COOKIE));
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

    pub async fn get_streams(&self, slug: &str, subject_id: &str, season: u32, episode: u32) -> Result<Option<StreamData>> {
        let se = season;
        let ep = episode;
        
        let type_path = "movies"; 
        let type_query = "/movie/detail";
        
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
            return Ok(serde_json::from_value(data["data"].clone()).ok());
        }

        Ok(None)
    }

    pub async fn get_streams_normalized(&self, title: &str, media_type: &str, season: u32, episode: u32) -> Result<Vec<StreamResult>> {
        info!("[SFlix Rust] Searching for: \"{}\" ({}) S{} E{}", title, media_type, season, episode);
        
        let items = self.search(title, media_type).await?;
        if items.is_empty() { return Ok(vec![]); }

        let best_match = &items[0];
        let slug = best_match["detailPath"].as_str().unwrap_or_default();
        let subject_id = best_match["subjectId"].as_str().unwrap_or_default();

        match self.get_streams(slug, subject_id, season, episode).await? {
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
                        });
                    }
                }
                Ok(results)
            }
            None => Ok(vec![])
        }
    }
}
