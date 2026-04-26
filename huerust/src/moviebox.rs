use serde::{Deserialize, Serialize};
use reqwest::{Client, header::{HeaderMap, HeaderValue, REFERER, USER_AGENT, ORIGIN, COOKIE}};
use anyhow::{Result, anyhow};
use std::collections::HashMap;
use tracing::{info, warn, error};
use crate::models::{StreamResult, Subtitle};

const BASE_URL: &str = "https://h5-api.aoneroom.com/wefeed-h5api-bff";
const DEFAULT_X_USER: &str = "{\"token\":\"eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJ1aWQiOjc1NjEyMzU2Nzc1NjA4NTMyODAsImF0cCI6MywiZXh0IjoiMTc3MTE0NDQ4MiIsImV4cCI6MTc3ODkyMDQ4MiwiaWF0IjoxNzcxMTQxNDk1fQ.BF7r1K_npeKWjkjuIxqZMTvUDxJ8oBjvSBc3glWcqtU\",\"userId\":\"7561235677560853280\",\"userType\":0,\"appType\":2}";
const DEFAULT_COOKIE: &str = "uuid=202d061a-fbe4-443c-aa09-be0232791ef7";

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
        Self {
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(45))
                .build()
                .unwrap(),
            host: host.unwrap_or_else(|| "moviebox.ph".to_string()),
        }
    }

    fn get_base_headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert("Accept", HeaderValue::from_static("application/json"));
        headers.insert(USER_AGENT, HeaderValue::from_static("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36"));
        headers.insert(ORIGIN, HeaderValue::from_str(&format!("https://{}", self.host)).unwrap());
        headers.insert("x-user", HeaderValue::from_str(DEFAULT_X_USER).unwrap());
        headers.insert("x-client-info", HeaderValue::from_static("{\"timezone\":\"Europe/Berlin\"}"));
        headers.insert(COOKIE, HeaderValue::from_static(DEFAULT_COOKIE));
        headers
    }

    pub async fn search(&self, keyword: &str, page: u32, m_type: u32) -> Result<Vec<MovieBoxItem>> {
        let url = format!("{}/subject/search", BASE_URL);
        let payload = serde_json::json!({
            "keyword": keyword,
            "page": page,
            "perPage": 28,
            "subjectType": m_type
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
        
        let hosts = vec!["moviebox.ph", "movibox.net", "123movienow.cc", "moviebox.to"];
        let timezones = vec!["Pacific/Pago_Pago", "Asia/Hong_Kong", "Europe/London", "America/New_York"];

        for (h_idx, host) in hosts.iter().enumerate() {
            let max_tz = if h_idx == 0 { 1 } else { 2 };
            for tz in timezones.iter().take(max_tz) {
                let play_url = format!("https://{}/wefeed-h5api-bff/subject/play", host);
                
                let mut referer_path = detail_path.to_string();
                if referer_path.starts_with('/') { referer_path.remove(0); }
                referer_path = referer_path.replace("movies/detail/", "movies/").replace("series/detail/", "series/");

                let referer = format!("https://{}/spa/videoPlayPage/{}?id={}&type=/{}/detail&detailSe=&detailEp=&lang=en", 
                    host, referer_path, subject_id, type_label);

                let url = format!("{}?subjectId={}&se={}&ep={}&detailPath={}", 
                    play_url, subject_id, season, episode, urlencoding::encode(detail_path));

                if h_idx > 0 {
                    info!("[MovieBox Rust] Retrying with host={} tz={}", host, tz);
                }

                let mut headers = self.get_base_headers();
                headers.insert(REFERER, HeaderValue::from_str(&referer).unwrap());
                headers.insert("x-client-info", HeaderValue::from_str(&format!("{{\"timezone\":\"{}\"}}", tz)).unwrap());

                match self.client.get(&url).headers(headers).send().await {
                    Ok(resp) => {
                        let text = resp.text().await?;
                        if text.contains("<!DOCTYPE") { continue; }
                        
                        let data: serde_json::Value = serde_json::from_str(&text)?;
                        if data["code"] == 0 {
                            return Ok(serde_json::from_value(data["data"].clone()).ok());
                        }
                        
                        if data["message"] == "invalid region" {
                            warn!("[MovieBox Rust] Region Blocked on {}: id={}", host, subject_id);
                            continue;
                        }
                    }
                    Err(_) => continue,
                }
            }
        }

        Ok(None)
    }

    pub async fn get_streams_normalized(&self, subject_id: &str, detail_path: &str, season: u32, episode: u32) -> Result<Vec<StreamResult>> {
        match self.get_streams(subject_id, detail_path, season, episode).await? {
            Some(data) => {
                let mut results = Vec::new();

                let mut mb_headers = HashMap::new();
                mb_headers.insert("Origin".to_string(), format!("https://{}", self.host));
                mb_headers.insert("x-user".to_string(), DEFAULT_X_USER.to_string());

                if let Some(streams) = data.streams {
                    for s in streams {
                        results.push(StreamResult {
                            url: s.url,
                            title: format!("Cinema - {}p (MP4)", s.resolutions.unwrap_or_else(|| "Auto".to_string())),
                            quality: s.quality.unwrap_or_else(|| "Auto".to_string()),
                            provider: "Cinema".to_string(),
                            format: "mp4".to_string(),
                            subtitles: None,
                            headers: Some(mb_headers.clone()),
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
                        });
                    }
                }
                Ok(results)
            }
            None => Ok(vec![])
        }
    }
}
