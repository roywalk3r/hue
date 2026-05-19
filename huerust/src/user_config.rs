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
        // 1. Direct config
        if let Some(xu) = self.moviebox_x_user.clone() {
            if !xu.is_empty() && xu != fallback {
                return xu;
            }
        }

        // 2. Env override
        if let Ok(env) = std::env::var("MOVIEBOX_X_USER") {
            if !env.is_empty() {
                return env;
            }
        }

        // 3. Extract JWT from movieboxCookie and synthesize x-user
        if let Some(cookie) = self.moviebox_cookie.as_ref() {
            if let Some(x_user) = Self::build_x_user_from_cookie(cookie) {
                return x_user;
            }
        }

        // 4. Fallback
        warn!("[HueConfig] moviebox_x_user not configured and could not extract from cookie; using fallback JWT. Set movieboxXUser in utils/user-config.json.");
        fallback.to_string()
    }

    /// Try to extract a JWT from the cookie string, decode its payload for uid/sub,
    /// and construct the x-user JSON that MovieBox expects.
    fn build_x_user_from_cookie(cookie: &str) -> Option<String> {
        let token = Self::extract_jwt_from_string(cookie)?;

        let parts: Vec<&str> = token.split('.').collect();
        if parts.len() != 3 {
            return None;
        }

        let payload = Self::base64url_decode(parts[1]).ok()?;
        let json: serde_json::Value = serde_json::from_slice(&payload).ok()?;

        let user_id = json.get("uid")
            .or_else(|| json.get("sub"))
            .or_else(|| json.get("userId"))
            .and_then(|v| v.as_str())?;

        let x_user = serde_json::json!({
            "token": token,
            "userId": user_id,
            "userType": 0,
            "appType": 2
        });

        Some(x_user.to_string())
    }

    fn extract_jwt_from_string(s: &str) -> Option<String> {
        // Strip URL-encoded quotes and literal quotes
        let cleaned = s.replace("%22", "").replace('"', "");

        for word in cleaned.split(|c: char| c == ' ' || c == ';' || c == '=' || c == ',') {
            let w = word.trim();
            // JWT signature: starts with eyJ, has exactly 2 dots, reasonable length
            if w.starts_with("eyJ") && w.matches('.').count() == 2 && w.len() > 50 {
                return Some(w.to_string());
            }
        }
        None
    }

    fn base64url_decode(input: &str) -> Result<Vec<u8>, ()> {
        use base64::{Engine as _, engine::general_purpose};

        let mut padded = input.replace('-', "+").replace('_', "/");
        let pad_len = (4 - padded.len() % 4) % 4;
        padded.extend(std::iter::repeat('=').take(pad_len));

        general_purpose::STANDARD.decode(&padded).map_err(|_| ())
    }
}
