//! Global provider popularity score per title.
//!
//! Tracks which provider delivers streams most consistently for a given title
//! across all users. When a title has a clear winner (e.g. MovieBox for
//! Deadpool), that provider gets priority treatment for all future requests.

use std::collections::HashMap;
use std::time::Duration;

enum ScoreBackend {
    Memory(moka::future::Cache<String, HashMap<String, u32>>),
    Redis(redis::aio::MultiplexedConnection),
}

impl Clone for ScoreBackend {
    fn clone(&self) -> Self {
        match self {
            ScoreBackend::Memory(c) => ScoreBackend::Memory(c.clone()),
            ScoreBackend::Redis(conn) => ScoreBackend::Redis(conn.clone()),
        }
    }
}

#[derive(Clone)]
pub struct ProviderScoreCache {
    backend: ScoreBackend,
    ttl_secs: u64,
}

impl ProviderScoreCache {
    pub async fn new_from_config(cfg: &crate::config::AppConfig) -> Self {
        if cfg.cache_backend == "redis" {
            match Self::new_redis(&cfg.redis_url, cfg.stale_cache_ttl_secs).await {
                Ok(cache) => {
                    tracing::info!("[ProviderScore] Using Redis backend");
                    cache
                }
                Err(e) => {
                    tracing::warn!(
                        "[ProviderScore] Redis connection failed ({}), falling back to memory",
                        e
                    );
                    Self::new_memory(cfg.stale_cache_ttl_secs)
                }
            }
        } else {
            tracing::info!("[ProviderScore] Using in-memory backend");
            Self::new_memory(cfg.stale_cache_ttl_secs)
        }
    }

    fn new_memory(ttl_secs: u64) -> Self {
        Self {
            backend: ScoreBackend::Memory(
                moka::future::Cache::builder()
                    .max_capacity(50_000)
                    .time_to_live(Duration::from_secs(ttl_secs))
                    .build(),
            ),
            ttl_secs,
        }
    }

    async fn new_redis(redis_url: &str, ttl_secs: u64) -> anyhow::Result<Self> {
        let client = redis::Client::open(redis_url)?;
        let conn = client.get_multiplexed_tokio_connection().await?;
        Ok(Self {
            backend: ScoreBackend::Redis(conn),
            ttl_secs,
        })
    }

    fn title_key(media_type: &str, tmdb_id: &str) -> String {
        format!("{}:{}", media_type, tmdb_id)
    }

    fn redis_title_key(media_type: &str, tmdb_id: &str) -> String {
        format!("hue:score:{}:{}", media_type, tmdb_id)
    }

    pub async fn record_success(&self, media_type: &str, tmdb_id: &str, provider_real_name: &str) {
        let title = Self::title_key(media_type, tmdb_id);
        match &self.backend {
            ScoreBackend::Memory(cache) => {
                let mut scores = cache.get(&title).await.unwrap_or_default();
                *scores.entry(provider_real_name.to_string()).or_insert(0) += 1;
                cache.insert(title, scores).await;
            }
            ScoreBackend::Redis(conn) => {
                let key = Self::redis_title_key(media_type, tmdb_id);
                let _: redis::RedisResult<()> = redis::cmd("ZINCRBY")
                    .arg(&key)
                    .arg(1)
                    .arg(provider_real_name)
                    .query_async(&mut conn.clone())
                    .await;
                let _: redis::RedisResult<()> = redis::cmd("EXPIRE")
                    .arg(&key)
                    .arg(self.ttl_secs)
                    .query_async(&mut conn.clone())
                    .await;
            }
        }
    }

    /// Return the provider with the highest success count for this title.
    pub async fn get_top_provider(&self, media_type: &str, tmdb_id: &str) -> Option<String> {
        let title = Self::title_key(media_type, tmdb_id);
        match &self.backend {
            ScoreBackend::Memory(cache) => cache.get(&title).await.and_then(|scores| {
                scores
                    .into_iter()
                    .max_by_key(|(_, count)| *count)
                    .map(|(p, _)| p)
            }),
            ScoreBackend::Redis(conn) => {
                let key = Self::redis_title_key(media_type, tmdb_id);
                let result: redis::RedisResult<Option<Vec<String>>> = redis::cmd("ZRANGE")
                    .arg(&key)
                    .arg(0)
                    .arg(0)
                    .arg("REV")
                    .query_async(&mut conn.clone())
                    .await;
                match result {
                    Ok(Some(entries)) if !entries.is_empty() => Some(entries[0].clone()),
                    _ => None,
                }
            }
        }
    }
}
