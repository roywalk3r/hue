//! Hue Stream API
//! Professional Streaming Aggregator
//!
//! @author roywalk3r <roywalker440@gmail.com>
//! @repository https://github.com/roywalk3r/hue.git
//! @license MIT

use std::time::Duration;
use crate::models::StreamResult;

#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub struct CacheKey {
    pub media_type: String,
    pub tmdb_id: String,
    pub season: u32,
    pub episode: u32,
}

impl CacheKey {
    pub fn new(media_type: &str, tmdb_id: &str, season: u32, episode: u32) -> Self {
        Self {
            media_type: media_type.to_string(),
            tmdb_id: tmdb_id.to_string(),
            season,
            episode,
        }
    }

    pub fn redis_key(&self) -> String {
        format!("huerust:streams:{}:{}:{}:{}", self.media_type, self.tmdb_id, self.season, self.episode)
    }

    fn moka_key(&self) -> String {
        format!("{}:{}:{}:{}", self.media_type, self.tmdb_id, self.season, self.episode)
    }
}

// ── Cache Backend Enum ─────────────────────────────────────────────────────────

enum CacheBackend {
    Memory(moka::future::Cache<String, Vec<StreamResult>>),
    Redis(redis::aio::MultiplexedConnection),
}

impl Clone for CacheBackend {
    fn clone(&self) -> Self {
        match self {
            CacheBackend::Memory(c) => CacheBackend::Memory(c.clone()),
            CacheBackend::Redis(conn) => CacheBackend::Redis(conn.clone()),
        }
    }
}

// ── Stream Cache ───────────────────────────────────────────────────────────────

pub struct StreamCache {
    backend: CacheBackend,
    ttl_secs: u64,
}

impl StreamCache {
    /// Create from config: uses Redis if `CACHE_BACKEND=redis`, otherwise in-memory
    pub async fn new_from_config(cfg: &crate::config::AppConfig) -> Self {
        if cfg.cache_backend == "redis" {
            match Self::new_redis(&cfg.redis_url, cfg.cache_ttl_secs).await {
                Ok(cache) => {
                    tracing::info!("[Cache] Using Redis backend at {}", cfg.redis_url);
                    cache
                }
                Err(e) => {
                    tracing::warn!("[Cache] Redis connection failed ({}), falling back to memory", e);
                    Self::new_memory(cfg.cache_max_capacity, cfg.cache_ttl_secs)
                }
            }
        } else {
            tracing::info!("[Cache] Using in-memory backend");
            Self::new_memory(cfg.cache_max_capacity, cfg.cache_ttl_secs)
        }
    }

    pub fn new() -> Self {
        Self::new_memory(1000, Duration::from_secs(4 * 60 * 60).as_secs())
    }

    #[allow(dead_code)]
    pub fn new_with_capacity_ttl(max_capacity: u64, ttl: Duration) -> Self {
        Self::new_memory(max_capacity, ttl.as_secs())
    }

    fn new_memory(max_capacity: u64, ttl_secs: u64) -> Self {
        Self {
            backend: CacheBackend::Memory(
                moka::future::Cache::builder()
                    .max_capacity(max_capacity)
                    .time_to_live(Duration::from_secs(ttl_secs))
                    .build()
            ),
            ttl_secs,
        }
    }

    async fn new_redis(redis_url: &str, ttl_secs: u64) -> anyhow::Result<Self> {
        let client = redis::Client::open(redis_url)?;
        let conn = client.get_multiplexed_tokio_connection().await?;
        Ok(Self {
            backend: CacheBackend::Redis(conn),
            ttl_secs,
        })
    }

    pub async fn get(&self, key: &CacheKey) -> Option<Vec<StreamResult>> {
        let streams = match &self.backend {
            CacheBackend::Memory(cache) => cache.get(&key.moka_key()).await,
            CacheBackend::Redis(conn) => {
                let result: redis::RedisResult<Option<String>> = redis::cmd("GET")
                    .arg(key.redis_key())
                    .query_async(&mut conn.clone())
                    .await;
                match result {
                    Ok(Some(json_str)) => serde_json::from_str::<Vec<StreamResult>>(&json_str).ok(),
                    _ => None,
                }
            }
        }?;

        let streams = filter_cacheable_streams(streams);
        if streams.is_empty() { None } else { Some(streams) }
    }

    pub async fn set(&self, key: &CacheKey, streams: Vec<StreamResult>) {
        let streams = filter_cacheable_streams(streams);
        if streams.is_empty() {
            return;
        }

        match &self.backend {
            CacheBackend::Memory(cache) => {
                cache.insert(key.moka_key(), streams).await;
            }
            CacheBackend::Redis(conn) => {
                if let Ok(json) = serde_json::to_string(&streams) {
                    let _: redis::RedisResult<()> = redis::cmd("SETEX")
                        .arg(key.redis_key())
                        .arg(self.ttl_secs)
                        .arg(json)
                        .query_async(&mut conn.clone())
                        .await;
                }
            }
        }
    }

    /// Merge new streams into existing cached entry, deduplicating by URL
    #[allow(dead_code)]
    pub async fn add(&self, key: &CacheKey, new_streams: Vec<StreamResult>) {
        let existing = self.get(key).await.unwrap_or_default();
        let mut merged = existing;
        let existing_urls: std::collections::HashSet<String> = merged.iter().map(|s| s.url.clone()).collect();

        for stream in new_streams {
            if !existing_urls.contains(&stream.url) {
                merged.push(stream);
            }
        }

        self.set(key, merged).await;
    }

    #[allow(dead_code)]
    pub async fn invalidate(&self, key: &CacheKey) {
        match &self.backend {
            CacheBackend::Memory(cache) => cache.invalidate(&key.moka_key()).await,
            CacheBackend::Redis(conn) => {
                let _: redis::RedisResult<()> = redis::cmd("DEL")
                    .arg(key.redis_key())
                    .query_async(&mut conn.clone())
                    .await;
            }
        }
    }

    #[allow(dead_code)]
    pub async fn clear(&self) {
        match &self.backend {
            CacheBackend::Memory(cache) => cache.invalidate_all(),
            CacheBackend::Redis(conn) => {
                let mut cursor: u64 = 0;
                loop {
                    let scan_result: redis::RedisResult<(u64, Vec<String>)> = redis::cmd("SCAN")
                        .arg(cursor)
                        .arg("MATCH")
                        .arg("huerust:streams:*")
                        .arg("COUNT")
                        .arg(100)
                        .query_async(&mut conn.clone())
                        .await;

                    match scan_result {
                        Ok((next, keys)) if !keys.is_empty() => {
                            let _: redis::RedisResult<()> = redis::cmd("DEL")
                                .arg(&keys)
                                .query_async(&mut conn.clone())
                                .await;
                            if next == 0 { break; }
                            cursor = next;
                        }
                        Ok((next, _)) if next == 0 => { break; }
                        Ok((next, _)) => { cursor = next; }
                        Err(e) => {
                            tracing::warn!("[Cache] Redis SCAN failed during clear: {}", e);
                            break;
                        }
                    }
                }
            }
        }
    }
}

fn filter_cacheable_streams(streams: Vec<StreamResult>) -> Vec<StreamResult> {
    streams
        .into_iter()
        .filter(|stream| {
            let provider = stream.provider.to_lowercase();
            let url = stream.url.to_lowercase();
            provider != "aura" && !url.contains("vixsrc.to")
        })
        .collect()
}

impl Default for StreamCache {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for StreamCache {
    fn clone(&self) -> Self {
        Self {
            backend: self.backend.clone(),
            ttl_secs: self.ttl_secs,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_stream(url: &str) -> StreamResult {
        StreamResult {
            url: url.to_string(),
            title: "Test".to_string(),
            quality: "1080p".to_string(),
            provider: "TestProvider".to_string(),
            format: "hls".to_string(),
            subtitles: None,
            headers: None,
            extra_metadata: None,
        }
    }

    #[tokio::test]
    async fn test_cache_get_set() {
        let cache = StreamCache::new();
        let key = CacheKey::new("movie", "123", 1, 1);
        let streams = vec![dummy_stream("http://example.com/1.m3u8")];

        cache.set(&key, streams.clone()).await;
        let got = cache.get(&key).await.unwrap();

        assert_eq!(got.len(), 1);
        assert_eq!(got[0].url, "http://example.com/1.m3u8");
    }

    #[tokio::test]
    async fn test_cache_merge_dedup() {
        let cache = StreamCache::new();
        let key = CacheKey::new("tv", "456", 2, 3);

        cache.set(&key, vec![dummy_stream("http://a.com")]).await;
        cache.add(&key, vec![
            dummy_stream("http://a.com"), // duplicate
            dummy_stream("http://b.com"), // new
        ]).await;

        let got = cache.get(&key).await.unwrap();
        assert_eq!(got.len(), 2);
    }

    #[tokio::test]
    async fn test_cache_invalidate() {
        let cache = StreamCache::new();
        let key = CacheKey::new("movie", "789", 1, 1);

        cache.set(&key, vec![dummy_stream("http://x.com")]).await;
        cache.invalidate(&key).await;

        assert!(cache.get(&key).await.is_none());
    }

    #[test]
    fn test_cache_key_redis() {
        let key = CacheKey::new("tv", "12345", 2, 5);
        assert_eq!(key.redis_key(), "huerust:streams:tv:12345:2:5");
    }
}
