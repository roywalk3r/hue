//! Hue Stream API
//! Professional Streaming Aggregator
//!
//! @author roywalk3r <roywalker440@gmail.com>
//! @repository https://github.com/roywalk3r/hue.git
//! @license MIT

use serde::{Deserialize, Serialize};
use reqwest::Client;
use anyhow::{Result, anyhow};
use std::sync::Arc;
use crate::tmdb::{TmdbClient, TmdbNormalizedInfo};
use tokio::sync::RwLock;
use std::collections::{HashMap, HashSet};
use tracing::info;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HiTvItem {
    pub id: String,
    pub title: String,
    pub cover: Option<String>,
    pub backdrop: Option<String>,
    pub rating: Option<String>,
    pub year: Option<String>,
    pub genre: Option<String>,
    #[serde(rename = "tmdbId")]
    pub tmdb_id: Option<u64>,
    pub overview: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HiTvBrowseResponse {
    pub items: Vec<HiTvItem>,
    pub total: u32,
}

pub struct HiTvClient {
    client: Client,
    #[allow(dead_code)]
    tmdb: Arc<TmdbClient>,
    pub library_cache: Arc<RwLock<Vec<HiTvItem>>>,
    #[allow(dead_code)]
    pub mapping_cache: Arc<RwLock<HashMap<String, TmdbNormalizedInfo>>>,
}

const BASE_URL: &str = "https://h5-api.aoneroom.com/wefeed-h5api-bff";

impl HiTvClient {
    pub fn new(tmdb: Arc<TmdbClient>) -> Self {
        Self {
            client: Client::new(),
            tmdb,
            library_cache: Arc::new(RwLock::new(Vec::new())),
            mapping_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn get_library(&self) -> Vec<HiTvItem> {
        self.library_cache.read().await.clone()
    }

    pub async fn build_library(&self) -> Result<()> {
        info!("[HiTV Rust] Starting Library Builder...");
        let mut all_items = Vec::new();
        let mut seen_ids = HashSet::new();

        let countries = vec!["Korea", "China", "Thailand", "Japan"];
        let categories = vec!["Drama", "Movie", "Variety"];

        for country in countries {
            for category in categories.iter() {
                info!("[HiTV Rust] Scanning {} {}...", country, category);
                // Scan first 5 pages of each category
                for page in 1..=5 {
                    if let Ok(res) = self.browse_hitv(page, 60, country, category, "All", "Hottest", None).await {
                        for item in res.items {
                            if !seen_ids.contains(&item.id) {
                                seen_ids.insert(item.id.clone());
                                all_items.push(item);
                            }
                        }
                    }
                }
            }
        }

        let count = all_items.len();
        let mut cache = self.library_cache.write().await;
        *cache = all_items;
        info!("[HiTV Rust] Library built successfully with {} items", count);
        Ok(())
    }

    pub async fn get_detail(&self, id: &str) -> Result<Option<HiTvItem>> {
        let url = format!("{}/detail?subjectId={}&lang=en", BASE_URL, id);

        let resp = self.client.get(&url)
            .send()
            .await?;

        let data: serde_json::Value = resp.json().await?;
        if data["code"] != 0 {
            return Ok(None);
        }

        let v = &data["data"]["subject"];
        Ok(Some(HiTvItem {
            id: v["subjectId"].as_str().unwrap_or_default().to_string(),
            title: v["title"].as_str().unwrap_or_default().to_string(),
            cover: v["cover"]["url"].as_str().map(String::from),
            backdrop: v["backdrop"]["url"].as_str().map(String::from),
            rating: v["imdbRatingValue"].as_f64().map(|s| s.to_string()),
            year: v["releaseDate"].as_str().map(String::from),
            genre: v["genre"].as_str().map(String::from),
            tmdb_id: v["tmdbId"].as_u64(),
            overview: v["description"].as_str().map(String::from),
        }))
    }

    pub async fn search(&self, query: &str, page: u32, page_size: u32) -> Result<HiTvBrowseResponse> {
        let url = format!("{}/subject/search", BASE_URL);
        let payload = serde_json::json!({
            "keyword": query,
            "page": page,
            "perPage": page_size
        });

        let resp = self.client.post(&url)
            .json(&payload)
            .send()
            .await?;

        let data: serde_json::Value = resp.json().await?;
        if data["code"] != 0 {
            return Ok(HiTvBrowseResponse { items: vec![], total: 0 });
        }

        let items = data["data"]["items"].as_array().map(|arr| {
            arr.iter().map(|v| HiTvItem {
                id: v["subjectId"].as_str().unwrap_or_default().to_string(),
                title: v["title"].as_str().unwrap_or_default().to_string(),
                cover: v["cover"]["url"].as_str().map(String::from),
                backdrop: None,
                rating: v["imdbRatingValue"].as_f64().map(|s| s.to_string()),
                year: v["releaseDate"].as_str().map(String::from),
                genre: v["genre"].as_str().map(String::from),
                tmdb_id: v["tmdbId"].as_u64(),
                overview: None,
            }).collect()
        }).unwrap_or_default();

        let total = data["data"]["pager"]["totalCount"].as_u64().unwrap_or(0) as u32;

        Ok(HiTvBrowseResponse {
            items,
            total,
        })
    }

    pub async fn browse_hitv(&self, page: u32, page_size: u32, country: &str, category: &str, genre: &str, sort: &str, year: Option<&str>) -> Result<HiTvBrowseResponse> {
        let url = format!("{}/subject/filter", BASE_URL);

        let channel_id = match category.to_lowercase().as_str() {
            "movie" => 1,
            "variety" => 3,
            "anime" => 4,
            _ => 2, // Drama
        };

        let mut payload = serde_json::json!({
            "page": page,
            "perPage": page_size,
            "channelId": channel_id,
            "country": if country.eq_ignore_ascii_case("all") { serde_json::Value::Null } else { serde_json::Value::String(country.to_string()) },
            "sort": sort
        });

        if !genre.eq_ignore_ascii_case("all") {
            payload["genre"] = serde_json::Value::String(genre.to_string());
        }
        if let Some(y) = year {
            payload["year"] = serde_json::Value::String(y.to_string());
        }

        let resp = self.client.post(&url)
            .json(&payload)
            .send()
            .await?;

        let data: serde_json::Value = resp.json().await?;
        if data["code"] != 0 {
            return Err(anyhow!("HiTV API error: {}", data["message"]));
        }

        let items = data["data"]["items"].as_array().map(|arr| {
            arr.iter().map(|v| HiTvItem {
                id: v["subjectId"].as_str().unwrap_or_default().to_string(),
                title: v["title"].as_str().unwrap_or_default().to_string(),
                cover: v["cover"]["url"].as_str().map(String::from),
                backdrop: None,
                rating: v["imdbRatingValue"].as_f64().map(|s| s.to_string()),
                year: v["releaseDate"].as_str().map(String::from),
                genre: v["genre"].as_str().map(String::from),
                tmdb_id: None,
                overview: None,
            }).collect()
        }).unwrap_or_default();

        let total = data["data"]["pager"]["totalCount"].as_u64().unwrap_or(0) as u32;

        Ok(HiTvBrowseResponse {
            items,
            total,
        })
    }
}
