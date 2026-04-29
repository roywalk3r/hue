//! Hue Stream API
//! Professional Streaming Aggregator
//!
//! @author roywalk3r <roywalker440@gmail.com>
//! @repository https://github.com/roywalk3r/hue.git
//! @license MIT

use std::env;
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct AppConfig {
    pub secret: String,
    pub host: String,
    pub port: String,
    pub proxy_base_url: String,
    pub tmdb_api_keys: Vec<String>,
    #[allow(dead_code)]
    pub api_key: Option<String>,
    #[allow(dead_code)]
    pub microservice_url: String,
    pub cache_ttl_secs: u64,
    #[allow(dead_code)]
    pub cache_max_capacity: u64,
    pub fast_provider_deadline_secs: u64,
    pub slow_provider_deadline_secs: u64,
    #[allow(dead_code)]
    pub fast_provider_timeout_secs: u64,
    #[allow(dead_code)]
    pub slow_provider_timeout_secs: u64,
    pub head_check_timeout_secs: u64,
    #[allow(dead_code)]
    pub enable_proxy: bool,
    pub cache_backend: String,
    pub redis_url: String,
    #[allow(dead_code)]
    pub redis_pool_size: u32,
}

impl AppConfig {
    pub fn from_env() -> Self {
        dotenv::dotenv().ok();

        let host = env::var("RUST_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
        let port = env::var("RUST_PORT").unwrap_or_else(|_| "24609".to_string());
        let proxy_base_url = env::var("RUST_PROXY_URL")
            .unwrap_or_else(|_| format!("http://{}:{}", host, port));

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
            api_key: env::var("API_KEY").ok(),
            microservice_url: env::var("MICROSERVICE_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:24610".to_string()),
            cache_ttl_secs: env::var("CACHE_TTL_SECS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(14400),
            cache_max_capacity: env::var("CACHE_MAX_CAPACITY")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(1000),
            fast_provider_deadline_secs: env::var("FAST_PROVIDER_DEADLINE_SECS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(5),
            slow_provider_deadline_secs: env::var("SLOW_PROVIDER_DEADLINE_SECS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(25),
            fast_provider_timeout_secs: env::var("FAST_PROVIDER_TIMEOUT_SECS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(15),
            slow_provider_timeout_secs: env::var("SLOW_PROVIDER_TIMEOUT_SECS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(60),
            head_check_timeout_secs: env::var("HEAD_CHECK_TIMEOUT_SECS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(2),
            enable_proxy: env::var("ENABLE_PROXY")
                .ok()
                .map(|s| s == "true" || s == "1")
                .unwrap_or(true),
            cache_backend: env::var("CACHE_BACKEND")
                .unwrap_or_else(|_| "memory".to_string()),
            redis_url: env::var("REDIS_URL")
                .unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string()),
            redis_pool_size: env::var("REDIS_POOL_SIZE")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(8),
        }
    }

    #[allow(dead_code)]
    pub fn addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }

    #[allow(dead_code)]
    pub fn cache_ttl(&self) -> Duration {
        Duration::from_secs(self.cache_ttl_secs)
    }

    pub fn fast_deadline(&self) -> Duration {
        Duration::from_secs(self.fast_provider_deadline_secs)
    }

    pub fn slow_deadline(&self) -> Duration {
        Duration::from_secs(self.slow_provider_deadline_secs)
    }
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
            api_key: None,
            microservice_url: "http://localhost:24610".to_string(),
            cache_ttl_secs: 3600,
            cache_max_capacity: 500,
            fast_provider_deadline_secs: 3,
            slow_provider_deadline_secs: 10,
            fast_provider_timeout_secs: 10,
            slow_provider_timeout_secs: 30,
            head_check_timeout_secs: 2,
            enable_proxy: false,
            cache_backend: "memory".to_string(),
            redis_url: "redis://127.0.0.1:6379".to_string(),
            redis_pool_size: 8,
        };

        assert_eq!(cfg.addr(), "0.0.0.0:3000");
        assert_eq!(cfg.cache_ttl(), Duration::from_secs(3600));
        assert_eq!(cfg.fast_deadline(), Duration::from_secs(3));
        assert_eq!(cfg.slow_deadline(), Duration::from_secs(10));
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
}
