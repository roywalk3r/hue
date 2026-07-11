//! Hue Stream API
//! Professional Streaming Aggregator
//!
//! @author roywalk3r <roywalker440@gmail.com>
//! @repository https://github.com/roywalk3r/hue.git
//! @license MIT

use anyhow::{anyhow, Result};
use moka::future::Cache;
use reqwest_middleware::ClientWithMiddleware;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TmdbMovieInfo {
    pub id: u64,
    pub title: String,
    pub overview: Option<String>,
    pub poster_path: Option<String>,
    pub backdrop_path: Option<String>,
    pub release_date: Option<String>,
    pub vote_average: Option<f64>,
    pub imdb_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ExternalIds {
    pub imdb_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TmdbTvInfo {
    pub id: u64,
    pub name: String,
    pub overview: Option<String>,
    pub poster_path: Option<String>,
    pub backdrop_path: Option<String>,
    pub first_air_date: Option<String>,
    pub vote_average: Option<f64>,
    pub external_ids: Option<ExternalIds>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TmdbNormalizedInfo {
    pub id: u64,
    pub title: String,
    pub overview: Option<String>,
    pub poster_path: Option<String>,
    pub backdrop_path: Option<String>,
    pub release_date: Option<String>,
    pub year: Option<u32>,
    pub rating: Option<f64>,
    pub imdb_id: Option<String>,
}

pub struct TmdbClient {
    client: ClientWithMiddleware,
    api_keys: Vec<String>,
    key_index: AtomicUsize,
    details_cache: Cache<(String, u64), TmdbNormalizedInfo>,
}

impl TmdbClient {
    pub fn new(api_keys: Vec<String>) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .tcp_keepalive(Duration::from_secs(60))
            .build()
            .unwrap();
        Self {
            client: crate::http_client::wrap_with_retry(client),
            api_keys,
            key_index: AtomicUsize::new(0),
            details_cache: Cache::builder()
                .max_capacity(500)
                .time_to_live(Duration::from_secs(3600))
                .build(),
        }
    }

    fn get_api_key(&self) -> String {
        if self.api_keys.is_empty() {
            return String::new();
        }
        let idx = self.key_index.fetch_add(1, Ordering::SeqCst) % self.api_keys.len();
        self.api_keys[idx].clone()
    }

    async fn tmdb_fetch<T: for<'de> Deserialize<'de>>(
        &self,
        endpoint: &str,
        params: HashMap<&str, String>,
    ) -> Result<T> {
        let key = self.get_api_key();
        if key.is_empty() {
            return Err(anyhow!("No TMDB API keys configured"));
        }

        let mut url = format!("https://api.themoviedb.org/3{}?api_key={}", endpoint, key);
        for (k, v) in &params {
            url.push_str(&format!("&{}={}", k, urlencoding::encode(v)));
        }

        let resp = self
            .client
            .get(&url)
            .header(
                "User-Agent",
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
            )
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(anyhow!("TMDB API returned {}", resp.status()));
        }

        let content_type = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();

        if !content_type.contains("application/json") {
            let body_preview = resp.text().await.unwrap_or_default();
            let preview = body_preview.chars().take(200).collect::<String>();
            tracing::warn!(
                "[TMDB] non-JSON response: content-type={}, body={}",
                content_type,
                preview
            );
            return Err(anyhow!("non-JSON response: {}", preview));
        }

        resp.json::<T>()
            .await
            .map_err(|e| anyhow!("JSON parse error: {}", e))
    }

    fn normalize_value(
        v: serde_json::Value,
        default_media_type: &str,
    ) -> Option<TmdbNormalizedInfo> {
        let media_type = v
            .get("media_type")
            .and_then(|m| m.as_str())
            .unwrap_or(default_media_type);
        if media_type == "tv" {
            let info: TmdbTvInfo = serde_json::from_value(v).ok()?;
            let year = info
                .first_air_date
                .as_ref()
                .and_then(|d| d.split('-').next())
                .and_then(|y| y.parse::<u32>().ok());
            Some(TmdbNormalizedInfo {
                id: info.id,
                title: info.name,
                overview: info.overview,
                poster_path: info.poster_path,
                backdrop_path: info.backdrop_path,
                release_date: info.first_air_date.clone(),
                year,
                rating: info.vote_average,
                imdb_id: None,
            })
        } else {
            let info: TmdbMovieInfo = serde_json::from_value(v).ok()?;
            let year = info
                .release_date
                .as_ref()
                .and_then(|d| d.split('-').next())
                .and_then(|y| y.parse::<u32>().ok());
            Some(TmdbNormalizedInfo {
                id: info.id,
                title: info.title,
                overview: info.overview,
                poster_path: info.poster_path,
                backdrop_path: info.backdrop_path,
                release_date: info.release_date.clone(),
                year,
                rating: info.vote_average,
                imdb_id: None,
            })
        }
    }

    pub async fn search(
        &self,
        query: &str,
        m_type: &str,
        year: Option<u32>,
    ) -> Result<Vec<TmdbNormalizedInfo>> {
        let endpoint = format!(
            "/search/{}",
            if m_type == "multi" {
                "multi"
            } else if m_type == "tv" {
                "tv"
            } else {
                "movie"
            }
        );
        let mut params = HashMap::new();
        params.insert("query", query.to_string());

        if let Some(y) = year {
            let year_key = if m_type == "tv" {
                "first_air_date_year"
            } else {
                "primary_release_year"
            };
            params.insert(year_key, y.to_string());
        }

        #[derive(Deserialize)]
        struct SearchResponse {
            results: Vec<serde_json::Value>,
        }

        let resp: SearchResponse = self.tmdb_fetch(&endpoint, params).await?;

        let normalized = resp
            .results
            .into_iter()
            .filter_map(|v| Self::normalize_value(v, m_type))
            .collect();

        Ok(normalized)
    }

    pub async fn movie_popular(&self, page: u32) -> Result<Vec<TmdbNormalizedInfo>> {
        let mut params = HashMap::new();
        params.insert("page", page.to_string());
        #[derive(Deserialize)]
        struct PaginatedResponse {
            results: Vec<serde_json::Value>,
        }
        let resp: PaginatedResponse = self.tmdb_fetch("/movie/popular", params).await?;
        Ok(resp
            .results
            .into_iter()
            .filter_map(|v| Self::normalize_value(v, "movie"))
            .collect())
    }

    pub async fn movie_top_rated(&self, page: u32) -> Result<Vec<TmdbNormalizedInfo>> {
        let mut params = HashMap::new();
        params.insert("page", page.to_string());
        #[derive(Deserialize)]
        struct PaginatedResponse {
            results: Vec<serde_json::Value>,
        }
        let resp: PaginatedResponse = self.tmdb_fetch("/movie/top_rated", params).await?;
        Ok(resp
            .results
            .into_iter()
            .filter_map(|v| Self::normalize_value(v, "movie"))
            .collect())
    }

    pub async fn movie_now_playing(&self, page: u32) -> Result<Vec<TmdbNormalizedInfo>> {
        let mut params = HashMap::new();
        params.insert("page", page.to_string());
        #[derive(Deserialize)]
        struct PaginatedResponse {
            results: Vec<serde_json::Value>,
        }
        let resp: PaginatedResponse = self.tmdb_fetch("/movie/now_playing", params).await?;
        Ok(resp
            .results
            .into_iter()
            .filter_map(|v| Self::normalize_value(v, "movie"))
            .collect())
    }

    pub async fn tv_popular(&self, page: u32) -> Result<Vec<TmdbNormalizedInfo>> {
        let mut params = HashMap::new();
        params.insert("page", page.to_string());
        #[derive(Deserialize)]
        struct PaginatedResponse {
            results: Vec<serde_json::Value>,
        }
        let resp: PaginatedResponse = self.tmdb_fetch("/tv/popular", params).await?;
        Ok(resp
            .results
            .into_iter()
            .filter_map(|v| Self::normalize_value(v, "tv"))
            .collect())
    }

    pub async fn tv_top_rated(&self, page: u32) -> Result<Vec<TmdbNormalizedInfo>> {
        let mut params = HashMap::new();
        params.insert("page", page.to_string());
        #[derive(Deserialize)]
        struct PaginatedResponse {
            results: Vec<serde_json::Value>,
        }
        let resp: PaginatedResponse = self.tmdb_fetch("/tv/top_rated", params).await?;
        Ok(resp
            .results
            .into_iter()
            .filter_map(|v| Self::normalize_value(v, "tv"))
            .collect())
    }

    pub async fn tv_on_the_air(&self, page: u32) -> Result<Vec<TmdbNormalizedInfo>> {
        let mut params = HashMap::new();
        params.insert("page", page.to_string());
        #[derive(Deserialize)]
        struct PaginatedResponse {
            results: Vec<serde_json::Value>,
        }
        let resp: PaginatedResponse = self.tmdb_fetch("/tv/on_the_air", params).await?;
        Ok(resp
            .results
            .into_iter()
            .filter_map(|v| Self::normalize_value(v, "tv"))
            .collect())
    }

    pub async fn trending(&self, media_type: &str, page: u32) -> Result<Vec<TmdbNormalizedInfo>> {
        let endpoint = format!(
            "/trending/{}/day",
            if media_type == "tv" { "tv" } else { "movie" }
        );
        let mut params = HashMap::new();
        params.insert("page", page.to_string());
        #[derive(Deserialize)]
        struct PaginatedResponse {
            results: Vec<serde_json::Value>,
        }
        let resp: PaginatedResponse = self.tmdb_fetch(&endpoint, params).await?;
        Ok(resp
            .results
            .into_iter()
            .filter_map(|v| Self::normalize_value(v, media_type))
            .collect())
    }

    pub async fn discover(
        &self,
        media_type: &str,
        genre_id: &str,
        page: u32,
    ) -> Result<Vec<TmdbNormalizedInfo>> {
        let endpoint = format!(
            "/discover/{}",
            if media_type == "tv" { "tv" } else { "movie" }
        );
        let mut params = HashMap::new();
        params.insert("page", page.to_string());
        params.insert("with_genres", genre_id.to_string());
        #[derive(Deserialize)]
        struct PaginatedResponse {
            results: Vec<serde_json::Value>,
        }
        let resp: PaginatedResponse = self.tmdb_fetch(&endpoint, params).await?;
        Ok(resp
            .results
            .into_iter()
            .filter_map(|v| Self::normalize_value(v, media_type))
            .collect())
    }

    pub async fn get_details(&self, m_type: &str, tmdb_id: u64) -> Result<TmdbNormalizedInfo> {
        let cache_key = (m_type.to_string(), tmdb_id);
        if let Some(cached) = self.details_cache.get(&cache_key).await {
            return Ok(cached);
        }
        let result = self.fetch_details(m_type, tmdb_id).await?;
        self.details_cache.insert(cache_key, result.clone()).await;
        Ok(result)
    }

    async fn fetch_details(&self, m_type: &str, tmdb_id: u64) -> Result<TmdbNormalizedInfo> {
        let endpoint = format!(
            "/{}/{}",
            if m_type == "tv" { "tv" } else { "movie" },
            tmdb_id
        );
        let mut params = HashMap::new();

        if m_type == "tv" {
            params.insert("append_to_response", "external_ids".to_string());
            let info: TmdbTvInfo = self.tmdb_fetch(&endpoint, params).await?;
            let year = info
                .first_air_date
                .as_ref()
                .and_then(|d| d.split('-').next())
                .and_then(|y| y.parse::<u32>().ok());
            let imdb_id = info.external_ids.and_then(|e| e.imdb_id);
            Ok(TmdbNormalizedInfo {
                id: info.id,
                title: info.name,
                overview: info.overview,
                poster_path: info.poster_path,
                backdrop_path: info.backdrop_path,
                release_date: info.first_air_date.clone(),
                year,
                rating: info.vote_average,
                imdb_id,
            })
        } else {
            let info: TmdbMovieInfo = self.tmdb_fetch(&endpoint, params).await?;
            let year = info
                .release_date
                .as_ref()
                .and_then(|d| d.split('-').next())
                .and_then(|y| y.parse::<u32>().ok());
            Ok(TmdbNormalizedInfo {
                id: info.id,
                title: info.title,
                overview: info.overview,
                poster_path: info.poster_path,
                backdrop_path: info.backdrop_path,
                release_date: info.release_date.clone(),
                year,
                rating: info.vote_average,
                imdb_id: info.imdb_id,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[tokio::test]
    async fn test_tmdb_search() {
        dotenv::dotenv().ok();
        let api_key = env::var("TMDB_API_KEY").unwrap_or_default();
        if api_key.is_empty() {
            return;
        }

        let client = TmdbClient::new(vec![api_key]);
        let results = client
            .search("Solo Leveling", "tv", Some(2024))
            .await
            .unwrap();

        assert!(!results.is_empty());
        assert!(results[0].title.contains("Solo Leveling"));
    }
}
