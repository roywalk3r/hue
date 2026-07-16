//! Sticky provider preference cache.
//!
//! Remembers per-show which provider served successfully so the next
//! episode can try that provider first instead of fanning out to all
//! providers. Keyed on `(media_type, tmdb_id)` — the Rust backend has
//! no JWT verification today, so the show's winning provider is a
//! property of the upstream mix, not the user.
//!
//! Storage mirrors `StreamCache`: moka in-process by default, redis when
//! `CACHE_BACKEND=redis`. Same `hue:sticky:*` key namespace as other caches.

use std::time::Duration;

/// Sentinel user_id for the show-only key shape. Reserved so it can never
/// collide with a real user id (CUIDs/Hex/UUIDs never start with `_`).
const SHOW_ONLY_USER: &str = "_";

#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub struct StickyPrefKey {
    pub user_id: String,
    pub media_type: String,
    pub tmdb_id: String,
}

impl StickyPrefKey {
    /// Key scoped to the show only — shared across all viewers of the title.
    pub fn for_show(media_type: &str, tmdb_id: &str) -> Self {
        Self {
            user_id: SHOW_ONLY_USER.to_string(),
            media_type: media_type.to_string(),
            tmdb_id: tmdb_id.to_string(),
        }
    }

    pub fn new(user_id: &str, media_type: &str, tmdb_id: &str) -> Self {
        Self {
            user_id: user_id.to_string(),
            media_type: media_type.to_string(),
            tmdb_id: tmdb_id.to_string(),
        }
    }

    pub fn redis_key(&self) -> String {
        format!(
            "hue:sticky:{}:{}:{}",
            self.user_id, self.media_type, self.tmdb_id
        )
    }

    fn moka_key(&self) -> String {
        format!("{}:{}:{}", self.user_id, self.media_type, self.tmdb_id)
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct StickyPrefEntry {
    pub provider_real_name: String,
    pub last_success_at_unix: u64,
    pub consecutive_failures: u32,
}

impl StickyPrefEntry {
    pub fn fresh(provider_real_name: &str) -> Self {
        Self {
            provider_real_name: provider_real_name.to_string(),
            last_success_at_unix: now_unix(),
            consecutive_failures: 0,
        }
    }
}

#[derive(Debug)]
enum CacheBackend {
    Memory(moka::future::Cache<String, StickyPrefEntry>),
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

pub struct StickyPrefCache {
    backend: CacheBackend,
    ttl_secs: u64,
    failure_threshold: u32,
    enabled: bool,
}

impl StickyPrefCache {
    pub async fn new_from_config(cfg: &crate::config::AppConfig) -> Self {
        if !cfg.sticky_pref_enabled {
            tracing::info!("[StickyPref] Disabled by config — using no-op memory backend");
            return Self::disabled(cfg.sticky_pref_ttl_secs, cfg.sticky_pref_failure_threshold);
        }
        if cfg.cache_backend == "redis" {
            match Self::new_redis(
                &cfg.redis_url,
                cfg.sticky_pref_ttl_secs,
                cfg.sticky_pref_failure_threshold,
            )
            .await
            {
                Ok(cache) => {
                    tracing::info!("[StickyPref] Using Redis backend");
                    cache
                }
                Err(e) => {
                    tracing::warn!(
                        "[StickyPref] Redis connection failed ({}), falling back to memory",
                        e
                    );
                    Self::new_memory(cfg.sticky_pref_ttl_secs, cfg.sticky_pref_failure_threshold)
                }
            }
        } else {
            tracing::info!("[StickyPref] Using in-memory backend");
            Self::new_memory(cfg.sticky_pref_ttl_secs, cfg.sticky_pref_failure_threshold)
        }
    }

    fn new_memory(ttl_secs: u64, failure_threshold: u32) -> Self {
        Self {
            backend: CacheBackend::Memory(
                moka::future::Cache::builder()
                    .max_capacity(50_000)
                    .time_to_live(Duration::from_secs(ttl_secs))
                    .build(),
            ),
            ttl_secs,
            failure_threshold,
            enabled: true,
        }
    }

    fn disabled(ttl_secs: u64, failure_threshold: u32) -> Self {
        Self {
            backend: CacheBackend::Memory(
                moka::future::Cache::builder()
                    .max_capacity(1)
                    .time_to_live(Duration::from_secs(1))
                    .build(),
            ),
            ttl_secs,
            failure_threshold,
            enabled: false,
        }
    }

    async fn new_redis(
        redis_url: &str,
        ttl_secs: u64,
        failure_threshold: u32,
    ) -> anyhow::Result<Self> {
        let client = redis::Client::open(redis_url)?;
        let conn = client.get_multiplexed_tokio_connection().await?;
        Ok(Self {
            backend: CacheBackend::Redis(conn),
            ttl_secs,
            failure_threshold,
            enabled: true,
        })
    }

    pub async fn get(&self, key: &StickyPrefKey) -> Option<StickyPrefEntry> {
        if !self.enabled {
            return None;
        }
        match &self.backend {
            CacheBackend::Memory(cache) => cache.get(&key.moka_key()).await,
            CacheBackend::Redis(conn) => {
                let result: redis::RedisResult<Option<String>> = redis::cmd("GET")
                    .arg(key.redis_key())
                    .query_async(&mut conn.clone())
                    .await;
                match result {
                    Ok(Some(json_str)) => serde_json::from_str::<StickyPrefEntry>(&json_str).ok(),
                    _ => None,
                }
            }
        }
    }

    pub async fn record_success(&self, key: &StickyPrefKey, provider_real_name: &str) {
        if !self.enabled {
            return;
        }
        let entry = StickyPrefEntry::fresh(provider_real_name);
        self.write(key, Some(entry)).await;
    }

    pub async fn record_failure(&self, key: &StickyPrefKey) {
        if !self.enabled {
            return;
        }
        let existing = self.get(key).await;
        let updated = match existing {
            Some(mut e) => {
                e.consecutive_failures += 1;
                if e.consecutive_failures >= self.failure_threshold {
                    self.write(key, None).await;
                    return;
                }
                e
            }
            None => return,
        };
        self.write(key, Some(updated)).await;
    }

    async fn write(&self, key: &StickyPrefKey, entry: Option<StickyPrefEntry>) {
        match &self.backend {
            CacheBackend::Memory(cache) => match entry {
                Some(e) => cache.insert(key.moka_key(), e).await,
                None => cache.invalidate(&key.moka_key()).await,
            },
            CacheBackend::Redis(conn) => {
                let mut conn = conn.clone();
                match entry {
                    Some(e) => {
                        if let Ok(json) = serde_json::to_string(&e) {
                            let _: redis::RedisResult<()> = redis::cmd("SETEX")
                                .arg(key.redis_key())
                                .arg(self.ttl_secs)
                                .arg(json)
                                .query_async(&mut conn)
                                .await;
                        }
                    }
                    None => {
                        let _: redis::RedisResult<()> = redis::cmd("DEL")
                            .arg(key.redis_key())
                            .query_async(&mut conn)
                            .await;
                    }
                }
            }
        }
    }
}

impl Clone for StickyPrefCache {
    fn clone(&self) -> Self {
        Self {
            backend: self.backend.clone(),
            ttl_secs: self.ttl_secs,
            failure_threshold: self.failure_threshold,
            enabled: self.enabled,
        }
    }
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_cache(ttl_secs: u64, threshold: u32) -> StickyPrefCache {
        StickyPrefCache {
            backend: CacheBackend::Memory(
                moka::future::Cache::builder()
                    .max_capacity(100)
                    .time_to_live(Duration::from_secs(ttl_secs))
                    .build(),
            ),
            ttl_secs,
            failure_threshold: threshold,
            enabled: true,
        }
    }

    fn test_key() -> StickyPrefKey {
        // Rust backend has no JWT verify today — production keys are
        // show-scoped via `for_show`. Tests use the same shape.
        StickyPrefKey::for_show("tv", "12345")
    }

    #[tokio::test]
    async fn record_success_then_get() {
        let cache = test_cache(60, 2);
        let key = StickyPrefKey::for_show("tv", "12345");

        assert!(cache.get(&key).await.is_none());
        cache.record_success(&key, "sflix").await;

        let entry = cache.get(&key).await.expect("entry should exist");
        assert_eq!(entry.provider_real_name, "sflix");
        assert_eq!(entry.consecutive_failures, 0);
    }

    #[tokio::test]
    async fn record_failure_increments_and_clears_at_threshold() {
        let cache = test_cache(60, 2);
        let key = StickyPrefKey::for_show("movie", "999");

        cache.record_success(&key, "moviebox").await;
        cache.record_failure(&key).await;

        let after_one = cache.get(&key).await.expect("still cached after 1 failure");
        assert_eq!(after_one.consecutive_failures, 1);
        assert_eq!(after_one.provider_real_name, "moviebox");

        cache.record_failure(&key).await;
        assert!(
            cache.get(&key).await.is_none(),
            "should be cleared at threshold"
        );
    }

    #[tokio::test]
    async fn record_success_resets_failure_counter() {
        let cache = test_cache(60, 3);
        let key = StickyPrefKey::for_show("tv", "777");

        cache.record_success(&key, "vidrock").await;
        cache.record_failure(&key).await;
        cache.record_failure(&key).await;
        cache.record_success(&key, "vidrock").await;

        let entry = cache.get(&key).await.expect("entry should exist");
        assert_eq!(entry.consecutive_failures, 0);
        assert_eq!(entry.provider_real_name, "vidrock");
    }

    #[tokio::test]
    async fn failure_on_nonexistent_entry_is_noop() {
        let cache = test_cache(60, 2);
        let key = StickyPrefKey::for_show("tv", "555");
        cache.record_failure(&key).await;
        assert!(cache.get(&key).await.is_none());
    }

    #[tokio::test]
    async fn ttl_expiry() {
        let cache = test_cache(1, 2);
        let key = StickyPrefKey::for_show("tv", "111");
        cache.record_success(&key, "sflix").await;
        assert!(cache.get(&key).await.is_some());
        tokio::time::sleep(Duration::from_millis(1500)).await;
        assert!(cache.get(&key).await.is_none());
    }

    #[tokio::test]
    async fn disabled_cache_returns_none() {
        let mut cache = test_cache(60, 2);
        cache.enabled = false;
        let key = StickyPrefKey::for_show("tv", "222");
        cache.record_success(&key, "sflix").await;
        assert!(cache.get(&key).await.is_none());
    }

    #[test]
    fn key_redis_format() {
        let key = StickyPrefKey::for_show("tv", "12345");
        assert_eq!(key.redis_key(), "hue:sticky:_:tv:12345");
    }
}
