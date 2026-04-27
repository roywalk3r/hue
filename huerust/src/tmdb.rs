//! Hue Stream API
//! Professional Streaming Aggregator
//!
//! @author roywalk3r <roywalker440@gmail.com>
//! @repository https://github.com/roywalk3r/hue.git
//! @license MIT

use serde::{Deserialize, Serialize};
use reqwest::Client;
use anyhow::{Result, anyhow};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};

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
    pub year: Option<u32>,
    pub rating: Option<f64>,
    pub imdb_id: Option<String>,
}

pub struct TmdbClient {
    client: Client,
    api_keys: Vec<String>,
    key_index: AtomicUsize,
}

impl TmdbClient {
    pub fn new(api_keys: Vec<String>) -> Self {
        Self {
            client: Client::new(),
            api_keys,
            key_index: AtomicUsize::new(0),
        }
    }

    fn get_api_key(&self) -> String {
        if self.api_keys.is_empty() {
            return String::new();
        }
        let idx = self.key_index.fetch_add(1, Ordering::SeqCst) % self.api_keys.len();
        self.api_keys[idx].clone()
    }

    async fn tmdb_fetch<T: for<'de> Deserialize<'de>>(&self, endpoint: &str, params: HashMap<&str, String>) -> Result<T> {
        let key = self.get_api_key();
        if key.is_empty() {
            return Err(anyhow!("No TMDB API keys configured"));
        }

        let mut url = format!("https://api.themoviedb.org/3{}?api_key={}", endpoint, key);
        for (k, v) in params {
            url.push_str(&format!("&{}={}", k, urlencoding::encode(&v)));
        }

        let resp = self.client.get(&url).send().await?;
        if !resp.status().is_success() {
            return Err(anyhow!("TMDB API error: {}", resp.status()));
        }

        let data = resp.json::<T>().await?;
        Ok(data)
    }

    pub async fn search(&self, query: &str, m_type: &str, year: Option<u32>) -> Result<Vec<TmdbNormalizedInfo>> {
        let endpoint = format!("/search/{}", if m_type == "multi" { "multi" } else if m_type == "tv" { "tv" } else { "movie" });
        let mut params = HashMap::new();
        params.insert("query", query.to_string());
        
        if let Some(y) = year {
            let year_key = if m_type == "tv" { "first_air_date_year" } else { "primary_release_year" };
            params.insert(year_key, y.to_string());
        }

        #[derive(Deserialize)]
        struct SearchResponse {
            results: Vec<serde_json::Value>,
        }

        let resp: SearchResponse = self.tmdb_fetch(&endpoint, params).await?;
        
        let normalized = resp.results.into_iter().filter_map(|v| {
            let media_type = v.get("media_type").and_then(|m| m.as_str()).unwrap_or(m_type);
            
            if media_type == "tv" {
                let info: TmdbTvInfo = serde_json::from_value(v).ok()?;
                let year = info.first_air_date.as_ref()
                    .and_then(|d| d.split('-').next())
                    .and_then(|y| y.parse::<u32>().ok());
                Some(TmdbNormalizedInfo {
                    id: info.id,
                    title: info.name,
                    overview: info.overview,
                    poster_path: info.poster_path,
                    backdrop_path: info.backdrop_path,
                    year,
                    rating: info.vote_average,
                    imdb_id: None,
                })
            } else {
                let info: TmdbMovieInfo = serde_json::from_value(v).ok()?;
                let year = info.release_date.as_ref()
                    .and_then(|d| d.split('-').next())
                    .and_then(|y| y.parse::<u32>().ok());
                Some(TmdbNormalizedInfo {
                    id: info.id,
                    title: info.title,
                    overview: info.overview,
                    poster_path: info.poster_path,
                    backdrop_path: info.backdrop_path,
                    year,
                    rating: info.vote_average,
                    imdb_id: None,
                })
            }
        }).collect();

        Ok(normalized)
    }

    pub async fn get_details(&self, m_type: &str, tmdb_id: u64) -> Result<TmdbNormalizedInfo> {
        let endpoint = format!("/{}/{}", if m_type == "tv" { "tv" } else { "movie" }, tmdb_id);
        let mut params = HashMap::new();

        if m_type == "tv" {
            params.insert("append_to_response", "external_ids".to_string());
            let info: TmdbTvInfo = self.tmdb_fetch(&endpoint, params).await?;
            let year = info.first_air_date.as_ref()
                .and_then(|d| d.split('-').next())
                .and_then(|y| y.parse::<u32>().ok());
            let imdb_id = info.external_ids.and_then(|e| e.imdb_id);
            Ok(TmdbNormalizedInfo {
                id: info.id,
                title: info.name,
                overview: info.overview,
                poster_path: info.poster_path,
                backdrop_path: info.backdrop_path,
                year,
                rating: info.vote_average,
                imdb_id,
            })
        } else {
            let info: TmdbMovieInfo = self.tmdb_fetch(&endpoint, params).await?;
            let year = info.release_date.as_ref()
                .and_then(|d| d.split('-').next())
                .and_then(|y| y.parse::<u32>().ok());
            Ok(TmdbNormalizedInfo {
                id: info.id,
                title: info.title,
                overview: info.overview,
                poster_path: info.poster_path,
                backdrop_path: info.backdrop_path,
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
        if api_key.is_empty() { return; }
        
        let client = TmdbClient::new(vec![api_key]);
        let results = client.search("Solo Leveling", "tv", Some(2024)).await.unwrap();
        
        assert!(!results.is_empty());
        assert!(results[0].title.contains("Solo Leveling"));
    }
}
