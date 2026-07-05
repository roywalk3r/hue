//! Hue Stream API
//! Professional Streaming Aggregator
//!
//! @author roywalk3r <roywalker440@gmail.com>
//! @repository https://github.com/roywalk3r/hue.git
//! @license MIT

use std::collections::HashMap;
use std::env;
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct AppConfig {
    pub secret: String,
    pub host: String,
    pub port: String,
    pub proxy_base_url: String,
    pub tmdb_api_keys: Vec<String>,
    pub cache_ttl_secs: u64,
    pub cache_max_capacity: u64,
    #[allow(dead_code)]
    pub fast_provider_deadline_secs: u64,
    pub slow_provider_deadline_secs: u64,
    pub head_check_timeout_secs: u64,
    pub enable_proxy: bool,
    pub cache_backend: String,
    pub redis_url: String,
    pub kkey: String,
    pub cf_worker_url: Option<String>,
    /// See [`crate::clients::aniwave::AniwaveClient`]. Defaults to the
    /// `HIANIME_MIRROR_URL` env var or empty.
    pub hianime_mirror_url: Option<String>,
    pub worker_upstream_hosts: Vec<String>,
    pub sticky_pref_enabled: bool,
    pub sticky_pref_ttl_secs: u64,
    pub sticky_pref_failure_threshold: u32,
    pub stale_cache_ttl_secs: u64,
    pub api_key: String,
    /// Per-provider season split mappings:
    /// provider_real_name → { tmdb_id → [episodes_in_season_1, episodes_in_season_2, ...] }
    /// For shows where TMDB lists as one season but some providers split into multiple.
    pub season_splits: HashMap<String, HashMap<String, Vec<u32>>>,
}

impl AppConfig {
    pub fn from_env() -> Self {
        dotenv::dotenv().ok();

        let host = env::var("RUST_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
        let port = env::var("RUST_PORT").unwrap_or_else(|_| "24609".to_string());
        let proxy_base_url =
            env::var("RUST_PROXY_URL").unwrap_or_else(|_| format!("http://{}:{}", host, port));

        let tmdb_api_keys = env::var("TMDB_API_KEYS")
            .unwrap_or_else(|_| env::var("TMDB_API_KEY").unwrap_or_default())
            .split(|c| c == ',' || c == ';' || c == '\n')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        Self {
            secret: env::var("APP_SECRET")
                .unwrap_or_else(|_| "hue-streaming-stable-secret-2026".to_string()),
            host,
            port,
            proxy_base_url,
            tmdb_api_keys,
            cache_ttl_secs: env::var("CACHE_TTL_SECS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(1800), // 30 min — CDN URLs expire in ~1h
            cache_max_capacity: env::var("CACHE_MAX_CAPACITY")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(1000),
            fast_provider_deadline_secs: env::var("FAST_PROVIDER_DEADLINE_SECS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(4),
            slow_provider_deadline_secs: env::var("SLOW_PROVIDER_DEADLINE_SECS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(8),
            head_check_timeout_secs: env::var("HEAD_CHECK_TIMEOUT_SECS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(3),
            enable_proxy: env::var("ENABLE_PROXY")
                .ok()
                .map(|s| s == "true" || s == "1")
                .unwrap_or(true),
            cache_backend: env::var("CACHE_BACKEND").unwrap_or_else(|_| "memory".to_string()),
            redis_url: env::var("REDIS_URL")
                .unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string()),
            kkey: env::var("KKEY").unwrap_or_else(|_| "hue-kkey-2026-default".to_string()),
            api_key: env::var("API_KEY").unwrap_or_else(|_| "dev".to_string()),
            cf_worker_url: env::var("CF_WORKER_URL")
                .ok()
                .filter(|s| !s.trim().is_empty()),
            hianime_mirror_url: env::var("HIANIME_MIRROR_URL")
                .ok()
                .filter(|s| !s.trim().is_empty()),
            worker_upstream_hosts: env::var("WORKER_UPSTREAM_HOSTS")
                .unwrap_or_default()
                .split(|c| c == ',' || c == ';' || c == '\n')
                .map(|s| s.trim().to_lowercase().to_string())
                .filter(|s| !s.is_empty())
                .collect(),
            sticky_pref_enabled: env::var("STICKY_PREF_ENABLED")
                .ok()
                .map(|s| s == "true" || s == "1")
                .unwrap_or(true),
            sticky_pref_ttl_secs: env::var("STICKY_PREF_TTL_SECS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(259200),
            sticky_pref_failure_threshold: env::var("STICKY_PREF_FAILURE_THRESHOLD")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(2),
            stale_cache_ttl_secs: env::var("STALE_CACHE_TTL_SECS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(86400),
            season_splits: parse_season_splits(&env::var("SEASON_SPLITS").unwrap_or_default()),
        }
    }

    pub fn validate_runtime(&self) -> anyhow::Result<()> {
        let env_name = env::var("APP_ENV")
            .or_else(|_| env::var("RUST_ENV"))
            .or_else(|_| env::var("NODE_ENV"))
            .unwrap_or_default()
            .to_lowercase();
        let is_production = matches!(env_name.as_str(), "prod" | "production");

        if is_production {
            if self.secret.trim().is_empty() || self.secret == "hue-streaming-stable-secret-2026" {
                anyhow::bail!("APP_SECRET must be set to a non-default value in production");
            }

            let proxy_url = url::Url::parse(&self.proxy_base_url).map_err(|_| {
                anyhow::anyhow!("RUST_PROXY_URL must be an absolute URL in production")
            })?;
            let proxy_host = proxy_url
                .host_str()
                .unwrap_or_default()
                .to_ascii_lowercase();
            if proxy_host.is_empty()
                || proxy_host == "localhost"
                || proxy_host == "127.0.0.1"
                || proxy_host == "0.0.0.0"
                || proxy_host == "::1"
            {
                anyhow::bail!(
                    "RUST_PROXY_URL must point to the public Rust backend origin in production, e.g. https://api.vexli.online"
                );
            }

            let api_key = env::var("API_KEY").unwrap_or_default();
            if api_key.trim().is_empty() {
                anyhow::bail!("API_KEY must be set in production");
            }
        }

        Ok(())
    }

    #[allow(dead_code)]
    pub fn fast_deadline(&self) -> Duration {
        Duration::from_secs(self.fast_provider_deadline_secs)
    }

    pub fn slow_deadline(&self) -> Duration {
        Duration::from_secs(self.slow_provider_deadline_secs)
    }

    /// Map TMDB season/episode to provider season/episode for shows where
    /// TMDB lists as one season but a specific provider splits into multiple seasons.
    /// Returns (provider_season, provider_episode).
    pub fn map_season_episode(
        &self,
        provider: &str,
        tmdb_id: &str,
        season: u32,
        episode: u32,
    ) -> (u32, u32) {
        if season != 1 {
            return (season, episode);
        }
        let Some(provider_map) = self.season_splits.get(provider) else {
            return (season, episode);
        };
        let Some(splits) = provider_map.get(tmdb_id) else {
            return (season, episode);
        };
        if splits.is_empty() {
            return (season, episode);
        }
        let mut cumulative = 0u32;
        for (idx, count) in splits.iter().enumerate() {
            if episode <= cumulative + *count {
                return (idx as u32 + 1, episode - cumulative);
            }
            cumulative += *count;
        }
        // Episode exceeds all split points — return original
        (season, episode)
    }
}

/// Parse SEASON_SPLITS env var format:
/// "moviebox:127532:12,13;sflix:127532:12,13" → {"moviebox": {"127532": [12,13]}, "sflix": {...}}
fn parse_season_splits(raw: &str) -> HashMap<String, HashMap<String, Vec<u32>>> {
    let mut map: HashMap<String, HashMap<String, Vec<u32>>> = HashMap::new();
    if raw.trim().is_empty() {
        return map;
    }
    for entry in raw.split(';') {
        let entry = entry.trim();
        if entry.is_empty() {
            continue;
        }
        let parts: Vec<&str> = entry.splitn(3, ':').collect();
        if parts.len() != 3 {
            continue;
        }
        let provider = parts[0].trim().to_string();
        let tmdb_id = parts[1].trim().to_string();
        let counts: Vec<u32> = parts[2]
            .split(',')
            .map(|s| s.trim().parse().unwrap_or(0))
            .filter(|n| *n > 0)
            .collect();
        if !counts.is_empty() {
            map.entry(provider).or_default().insert(tmdb_id, counts);
        }
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_helpers() {
        let cfg = AppConfig {
            secret: "s".to_string(),
            host: "0.0.0.0".to_string(),
            port: "3000".to_string(),
            proxy_base_url: "http://localhost:3000".to_string(),
            tmdb_api_keys: vec!["k1".to_string()],
            cache_ttl_secs: 3600,
            cache_max_capacity: 500,
            fast_provider_deadline_secs: 3,
            slow_provider_deadline_secs: 10,
            head_check_timeout_secs: 2,
            enable_proxy: false,
            cache_backend: "memory".to_string(),
            redis_url: "redis://127.0.0.1:6379".to_string(),
            kkey: "test-kkey".to_string(),
            cf_worker_url: None,
            hianime_mirror_url: None,
            worker_upstream_hosts: vec![],
            sticky_pref_enabled: true,
            sticky_pref_ttl_secs: 3600,
            sticky_pref_failure_threshold: 2,
            stale_cache_ttl_secs: 86400,
            api_key: "dev".to_string(),
            season_splits: HashMap::new(),
        };

        assert_eq!(cfg.fast_deadline(), Duration::from_secs(3));
        assert_eq!(cfg.slow_deadline(), Duration::from_secs(10));

        // Test parsing of the new per-provider format
        let parsed = parse_season_splits("moviebox:127532:12,13;sflix:127532:12,13");
        assert_eq!(
            parsed.get("moviebox").unwrap().get("127532").unwrap(),
            &vec![12, 13]
        );
        assert_eq!(
            parsed.get("sflix").unwrap().get("127532").unwrap(),
            &vec![12, 13]
        );
        assert!(parsed.get("vidrock").is_none());
    }

    #[test]
    fn test_tmdb_keys_parsing() {
        let keys: Vec<String> = "key1,key2;key3\nkey4"
            .split(|c: char| c == ',' || c == ';' || c == '\n')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        assert_eq!(keys, vec!["key1", "key2", "key3", "key4"]);
    }

    #[test]
    fn test_map_season_episode_split() {
        let mut splits: HashMap<String, HashMap<String, Vec<u32>>> = HashMap::new();
        let mut inner = HashMap::new();
        inner.insert("127532".to_string(), vec![12, 13]);
        splits.insert("moviebox".to_string(), inner.clone());
        splits.insert("sflix".to_string(), inner);
        let cfg = AppConfig {
            secret: "s".to_string(),
            host: "0.0.0.0".to_string(),
            port: "3000".to_string(),
            proxy_base_url: "http://localhost:3000".to_string(),
            tmdb_api_keys: vec!["k1".to_string()],
            cache_ttl_secs: 3600,
            cache_max_capacity: 500,
            fast_provider_deadline_secs: 3,
            slow_provider_deadline_secs: 10,
            head_check_timeout_secs: 2,
            enable_proxy: false,
            cache_backend: "memory".to_string(),
            redis_url: "redis://127.0.0.1:6379".to_string(),
            kkey: "test-kkey".to_string(),
            cf_worker_url: None,
            hianime_mirror_url: None,
            worker_upstream_hosts: vec![],
            sticky_pref_enabled: true,
            sticky_pref_ttl_secs: 3600,
            sticky_pref_failure_threshold: 2,
            stale_cache_ttl_secs: 86400,
            api_key: "dev".to_string(),
            season_splits: splits,
        };

        // Solo Leveling for moviebox: TMDB S1E1-12 map to provider S1E1-12
        assert_eq!(cfg.map_season_episode("moviebox", "127532", 1, 1), (1, 1));
        assert_eq!(cfg.map_season_episode("moviebox", "127532", 1, 12), (1, 12));

        // TMDB S1E13-25 map to provider S2E1-13
        assert_eq!(cfg.map_season_episode("moviebox", "127532", 1, 13), (2, 1));
        assert_eq!(cfg.map_season_episode("moviebox", "127532", 1, 18), (2, 6));
        assert_eq!(cfg.map_season_episode("moviebox", "127532", 1, 25), (2, 13));

        // vidrock has no split — passes through unchanged
        assert_eq!(cfg.map_season_episode("vidrock", "127532", 1, 18), (1, 18));

        // Unknown show / season > 1 pass through unchanged
        assert_eq!(cfg.map_season_episode("moviebox", "99999", 1, 5), (1, 5));
        assert_eq!(cfg.map_season_episode("moviebox", "127532", 2, 1), (2, 1));
    }
}
