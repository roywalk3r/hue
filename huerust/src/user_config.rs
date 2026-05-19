//! Hue Stream API
//! Professional Streaming Aggregator
//!
//! @author roywalk3r <roywalker440@gmail.com>
//! @repository https://github.com/roywalk3r/hue.git
//! @license MIT

use serde::Deserialize;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use std::sync::OnceLock;
use tracing::warn;

static CONFIG_CACHE: OnceLock<Mutex<(HueUserConfig, Instant)>> = OnceLock::new();
const CACHE_TTL: Duration = Duration::from_secs(300);

#[derive(Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct HueUserConfig {
    pub sflix_cookie: Option<String>,
    #[allow(dead_code)]
    pub sflix_user_agent: Option<String>,
    pub moviebox_cookie: Option<String>,
    pub moviebox_user_agent: Option<String>,
    pub moviebox_host: Option<String>,
    pub moviebox_x_user: Option<String>,
}

impl HueUserConfig {
    pub fn load() -> Self {
        let path = Self::resolve_path();
        match std::fs::File::open(&path) {
            Ok(f) => match serde_json::from_reader(f) {
                Ok(cfg) => return cfg,
                Err(e) => warn!("[HueConfig] Failed to parse {}: {}", path.display(), e),
            },
            Err(_) => warn!("[HueConfig] user-config.json not found at {}", path.display()),
        }
        HueUserConfig::default()
    }

    /// Returns a cached config, reloading from disk if older than 5 minutes.
    pub fn get_live() -> Self {
        let cache = CONFIG_CACHE.get_or_init(|| {
            Mutex::new((Self::load(), Instant::now()))
        });
        let mut guard = cache.lock().unwrap();
        if guard.1.elapsed() > CACHE_TTL {
            *guard = (Self::load(), Instant::now());
        }
        guard.0.clone()
    }

    /// Forces the next `get_live()` call to reload from disk immediately.
    pub fn invalidate() {
        if let Some(cache) = CONFIG_CACHE.get() {
            if let Ok(mut guard) = cache.lock() {
                guard.1 = Instant::now().checked_sub(CACHE_TTL + Duration::from_secs(1))
                    .unwrap_or(Instant::now());
            }
        }
    }

    fn resolve_path() -> PathBuf {
        // 1. Explicit env override — portable across any VPS
        if let Ok(p) = std::env::var("USER_CONFIG_PATH") {
            return PathBuf::from(p);
        }

        // 2. Relative to executable directory (primary — matches Node.js refresh script)
        //    e.g. huerust/target/release/huerust → ../../../utils/user-config.json
        if let Ok(exe) = std::env::current_exe() {
            if let Some(exe_dir) = exe.parent() {
                let from_exe = exe_dir.join("../../../utils/user-config.json");
                if from_exe.exists() {
                    return from_exe;
                }
                let from_exe = exe_dir.join("../utils/user-config.json");
                if from_exe.exists() {
                    return from_exe;
                }
            }
        }

        // 3. Relative to current working directory
        let candidates = [
            "utils/user-config.json",
            "../utils/user-config.json",
            "../../utils/user-config.json",
        ];
        for c in &candidates {
            let p = PathBuf::from(c);
            if p.exists() {
                return p;
            }
        }

        PathBuf::from("utils/user-config.json")
    }

    pub fn sflix_cookie_or_env(&self, fallback: &str) -> String {
        self.sflix_cookie
            .clone()
            .or_else(|| std::env::var("SFLIX_COOKIE").ok())
            .unwrap_or_else(|| fallback.to_string())
    }

    pub fn moviebox_cookie_or_env(&self, fallback: &str) -> String {
        self.moviebox_cookie
            .clone()
            .or_else(|| std::env::var("MOVIEBOX_COOKIE").ok())
            .unwrap_or_else(|| fallback.to_string())
    }

    pub fn moviebox_host_or_env(&self, fallback: &str) -> String {
        self.moviebox_host
            .clone()
            .or_else(|| std::env::var("MOVIEBOX_HOST").ok())
            .unwrap_or_else(|| fallback.to_string())
    }

    pub fn moviebox_x_user_or_env(&self, fallback: &str) -> String {
        self.moviebox_x_user
            .clone()
            .or_else(|| std::env::var("MOVIEBOX_X_USER").ok())
            .unwrap_or_else(|| {
                warn!("[HueConfig] moviebox_x_user not configured; using fallback JWT. Set it in utils/user-config.json or MOVIEBOX_X_USER env.");
                fallback.to_string()
            })
    }
}
