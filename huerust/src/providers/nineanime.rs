use serde::{Deserialize, Serialize};
use reqwest::Client;
use anyhow::{Result, anyhow};
use std::collections::HashMap;
use tracing::info;

const BASE_URL: &str = "https://9animetv.to";
const RAPID_CLOUD: &str = "https://rapid-cloud.co";
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

#[derive(Debug, Serialize, Deserialize)]
pub struct NineAnimeResponse<T> {
    pub status: bool,
    pub html: Option<String>,
    pub link: Option<String>,
    #[serde(flatten)]
    pub other: T,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RapidCloudResponse {
    pub sources: Vec<RapidCloudSource>,
    pub tracks: Option<Vec<RapidCloudTrack>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RapidCloudSource {
    pub file: String,
    #[serde(rename = "type")]
    pub s_type: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RapidCloudTrack {
    pub file: String,
    pub label: String,
    pub kind: String,
}

pub struct NineAnimeClient {
    client: Client,
}

impl NineAnimeClient {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .user_agent(USER_AGENT)
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap(),
        }
    }

    fn get_headers(&self, referer: &str) -> reqwest::header::HeaderMap {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("X-Requested-With", reqwest::header::HeaderValue::from_static("XMLHttpRequest"));
        headers.insert(reqwest::header::REFERER, reqwest::header::HeaderValue::from_str(referer).unwrap());
        headers
    }

    pub async fn search(&self, keyword: &str) -> Result<String> {
        let url = format!("{}/search?keyword={}", BASE_URL, urlencoding::encode(keyword));
        let resp = self.client.get(&url).send().await?;
        Ok(resp.text().await?)
    }

    pub async fn get_episode_list(&self, id: &str) -> Result<serde_json::Value> {
        let url = format!("{}/ajax/episode/list/{}", BASE_URL, id);
        let resp = self.client.get(&url).headers(self.get_headers(BASE_URL)).send().await?;
        Ok(resp.json().await?)
    }

    pub async fn get_servers(&self, ep_id: &str) -> Result<serde_json::Value> {
        let url = format!("{}/ajax/episode/servers?episodeId={}", BASE_URL, ep_id);
        let resp = self.client.get(&url).headers(self.get_headers(BASE_URL)).send().await?;
        Ok(resp.json().await?)
    }

    pub async fn get_sources(&self, data_id: &str) -> Result<serde_json::Value> {
        let url = format!("{}/ajax/episode/sources?id={}", BASE_URL, data_id);
        let resp = self.client.get(&url).headers(self.get_headers(BASE_URL)).send().await?;
        Ok(resp.json().await?)
    }

    pub async fn get_rapid_sources(&self, embed_id: &str, embed_url: &str) -> Result<RapidCloudResponse> {
        let url = format!("{}/embed-2/v2/e-1/getSources?id={}", RAPID_CLOUD, embed_id);
        let resp = self.client.get(&url).headers(self.get_headers(embed_url)).send().await?;
        Ok(resp.json().await?)
    }
}
