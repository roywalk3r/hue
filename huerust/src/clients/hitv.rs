//! Hue Stream API
//! Professional Streaming Aggregator
//!
//! @author roywalk3r <roywalker440@gmail.com>
//! @repository https://github.com/roywalk3r/hue.git
//! @license MIT

use serde::{Deserialize, Serialize};
use reqwest_middleware::ClientWithMiddleware;
use anyhow::{Result, anyhow};
use std::sync::Arc;
use crate::clients::tmdb::{TmdbClient, TmdbNormalizedInfo};
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
    client: ClientWithMiddleware,
    #[allow(dead_code)]
    tmdb: Arc<TmdbClient>,
    pub library_cache: Arc<RwLock<Vec<HiTvItem>>>,
    #[allow(dead_code)]
    pub mapping_cache: Arc<RwLock<HashMap<String, TmdbNormalizedInfo>>>,
}

impl Clone for HiTvClient {
    fn clone(&self) -> Self {
        Self {
            client: self.client.clone(),
            tmdb: self.tmdb.clone(),
            library_cache: self.library_cache.clone(),
            mapping_cache: self.mapping_cache.clone(),
        }
    }
}

const BASE_URL: &str = "https://h5-api.aoneroom.com/wefeed-h5api-bff";

impl HiTvClient {
    pub fn new(tmdb: Arc<TmdbClient>) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .connect_timeout(std::time::Duration::from_secs(8))
            .tcp_keepalive(std::time::Duration::from_secs(60))
            .pool_max_idle_per_host(4)
            .pool_idle_timeout(std::time::Duration::from_secs(90))
            .build()
            .unwrap();
        Self {
            client: crate::http_client::wrap_with_retry(client),
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

        let countries = ["Korea", "China", "Thailand", "Japan"];
        let categories = ["Drama", "Movie", "Variety"];

        // Fire all 60 page requests concurrently (4 countries × 3 categories × 5 pages).
        let mut set = tokio::task::JoinSet::new();
        for country in countries {
            for category in categories {
                for page in 1u32..=5 {
                    let slf = self.clone();
                    set.spawn(async move {
                        slf.browse_hitv(page, 60, country, category, "All", "Hottest", None).await
                    });
                }
            }
        }

        let mut all_items = Vec::new();
        let mut seen_ids = HashSet::new();
        while let Some(result) = set.join_next().await {
            if let Ok(Ok(res)) = result {
                for item in res.items {
                    if seen_ids.insert(item.id.clone()) {
                        all_items.push(item);
                    }
                }
            }
        }

        let count = all_items.len();
        *self.library_cache.write().await = all_items;
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
