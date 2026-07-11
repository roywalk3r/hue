//! Hue Stream API
//! Professional Streaming Aggregator
//!
//! @author roywalk3r <roywalker440@gmail.com>
//! @repository https://github.com/roywalk3r/hue.git
//! @license MIT

use crate::models::{StreamResult, Subtitle};
use anyhow::{anyhow, Result};
use base64::{engine::general_purpose, Engine as _};
use reqwest_middleware::ClientWithMiddleware;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::process::Command;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tracing::{info, warn};

const API_BASE: &str = "https://kisskh.ovh/api";
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";
/// Path to the cookie file written by the Node.js Playwright refresh script.
/// Defaults to `data/kisskh-cookies.json` (relative to CWD); override with `KISSKH_COOKIE_FILE` env var.
fn kisskh_cookie_file() -> String {
    std::env::var("KISSKH_COOKIE_FILE")
        .unwrap_or_else(|_| "../data/kisskh-cookies.json".to_string())
}

/// Playwright-extracted cookie payload from Node.js
#[derive(Debug, Deserialize)]
struct CookieFile {
    cookie_header: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct KissKhSearchItem {
    pub id: u64,
    pub title: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct KissKhEpisode {
    pub id: u64,
    pub number: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct KissKhDramaDetail {
    pub episodes: Vec<KissKhEpisode>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct KissKhStreamPayload {
    #[serde(rename = "Video")]
    pub video: Option<String>,
    #[serde(rename = "ThirdParty")]
    pub third_party: Option<String>,
    #[serde(rename = "Subtitles")]
    pub subtitles: Option<Vec<KissKhSubtitle>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct KissKhSubtitle {
    pub src: String,
    pub label: Option<String>,
    pub lang: Option<String>,
}

const KISSKH_APP_VER: &str = "2.8.10";
const KISSKH_PLATFORM_VER: u64 = 4830201;
const KISSKH_APP_NAME: &str = "kisskh";
const KISSKH_STREAM_VIGUID: &str = "62f176f3bb1b5b8e70e39932ad34a0c7";
const KISSKH_SUB_VIGUID: &str = "VgV52sWhwvBSf8BsM3BRY9weWiiCbtGp";

pub struct KissKhClient {
    client: ClientWithMiddleware,
    common_js_cache: Mutex<Option<(String, Instant)>>,
    cookie_header: Mutex<Option<String>>,
    /// Catalog responses (search/browse/show/upcoming/top-rating) keyed by
    /// endpoint+params → serialized JSON. Collapses the constant catalog traffic
    /// into one upstream fetch per TTL, so browsing doesn't hammer the CF proxy
    /// pool on every user click.
    catalog_cache: moka::future::Cache<String, String>,
}

/// TTL for cached kisskh catalog responses. Catalog data changes slowly (new
/// episodes hourly at most), so a generous default keeps proxy/upstream load
/// negligible. Override with `KISSKH_CATALOG_TTL_SECS`.
/// Whether a kisskh Video/ThirdParty URL is an actual playable stream rather
/// than a placeholder. Unreleased episodes return a countdown-widget URL
/// (tickcounter.com) instead of a video; those must not reach the player.
fn is_playable_video_url(url: &str) -> bool {
    let u = url.to_ascii_lowercase();
    if u.contains("tickcounter.com") || u.contains("/widget/") {
        return false;
    }
    // Real kisskh streams are HLS/MP4 on its video CDNs.
    u.contains(".m3u8") || u.contains(".mp4") || u.contains("cdnvideo") || u.contains("hakunay")
}

fn catalog_ttl_secs() -> u64 {
    std::env::var("KISSKH_CATALOG_TTL_SECS")
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(1800)
}

impl KissKhClient {
    pub fn new() -> Self {
        let builder = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .tcp_keepalive(std::time::Duration::from_secs(60))
            .connect_timeout(std::time::Duration::from_secs(10))
            .pool_max_idle_per_host(2)
            .pool_idle_timeout(std::time::Duration::from_secs(90))
            .timeout(std::time::Duration::from_secs(15));
        // Route kisskh.ovh (Cloudflare-challenged on the VPS) through the clean-IP pool.
        let builder = crate::http_client::with_cf_proxy_pool(builder);
        let client = crate::http_client::build_client_with_retry(builder, "KISSKH_PROXY_URL");

        let cookie_header = Self::load_cookies_from_file();
        if cookie_header.is_some() {
            info!("[KissKH] Loaded cookies from {}", kisskh_cookie_file());
        }

        Self {
            client,
            common_js_cache: Mutex::new(None),
            cookie_header: Mutex::new(cookie_header),
            catalog_cache: moka::future::Cache::builder()
                .max_capacity(512)
                .time_to_live(Duration::from_secs(catalog_ttl_secs()))
                .build(),
        }
    }

    /// Try to load cookies written by the Node.js Playwright refresh script.
    fn load_cookies_from_file() -> Option<String> {
        let raw = std::fs::read_to_string(kisskh_cookie_file()).ok()?;
        let parsed: CookieFile = serde_json::from_str(&raw).ok()?;
        if parsed.cookie_header.is_empty() {
            return None;
        }
        Some(parsed.cookie_header)
    }

    /// Build a GET request, injecting the stored Cookie header if available.
    fn get(&self, url: &str) -> reqwest_middleware::RequestBuilder {
        let mut req = self.client.get(url);
        if let Ok(lock) = self.cookie_header.lock() {
            if let Some(ref cookie) = *lock {
                req = req.header("Cookie", cookie.clone());
            }
        }
        req
    }

    /// Generate a kkey token for the given episode ID and viGuid.
    /// Uses Node.js to execute the obfuscated _0x54b991 AES function from common.js.
    async fn generate_kkey(&self, episode_id: u64, vi_guid: &str) -> Result<String> {
        // Check cache (valid for 1 hour)
        {
            let cache = self.common_js_cache.lock().unwrap();
            if let Some((ref js, ref time)) = *cache {
                if time.elapsed() < Duration::from_secs(3600) {
                    return Self::exec_kkey_js(js, episode_id, vi_guid);
                }
            }
        }

        // Fetch common.js (with cookies to bypass CF)
        let js_code = self
            .get("https://kisskh.ovh/common.js")
            .send()
            .await
            .map_err(|e| anyhow!("Failed to fetch common.js: {}", e))?
            .text()
            .await
            .map_err(|e| anyhow!("Failed to read common.js: {}", e))?;

        // Cache it
        {
            let mut cache = self.common_js_cache.lock().unwrap();
            *cache = Some((js_code.clone(), Instant::now()));
        }

        Self::exec_kkey_js(&js_code, episode_id, vi_guid)
    }

    fn exec_kkey_js(common_js: &str, episode_id: u64, vi_guid: &str) -> Result<String> {
        let script = format!(
            r#"{common_js}
const episodeId = {episode_id};
const appVer = "{app_ver}";
const viGuid = "{vi_guid}";
const platformVer = {platform_ver};
const appName = "{app_name}";
const token = _0x54b991(episodeId, null, appVer, viGuid, platformVer, appName, appName, appName, appName, appName, appName);
console.log(token);
"#,
            common_js = common_js,
            episode_id = episode_id,
            app_ver = KISSKH_APP_VER,
            vi_guid = vi_guid,
            platform_ver = KISSKH_PLATFORM_VER,
            app_name = KISSKH_APP_NAME,
        );

        let child = Command::new("node")
            .arg("-e")
            .arg(&script)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| anyhow!("Failed to spawn node for kkey generation: {}", e))?;

        let output = child
            .wait_with_output()
            .map_err(|e| anyhow!("Node kkey process failed: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("Node kkey generation error: {}", stderr));
        }

        let token = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if token.is_empty() {
            return Err(anyhow!("Node returned empty kkey"));
        }
        Ok(token)
    }

    pub async fn search(&self, title: &str) -> Result<Vec<KissKhSearchItem>> {
        let ck = format!("search:{}", title.to_lowercase());
        if let Some(s) = self.catalog_cache.get(&ck).await {
            if let Ok(v) = serde_json::from_str(&s) {
                return Ok(v);
            }
        }
        let url = format!(
            "{}/DramaList/Search?q={}",
            API_BASE,
            urlencoding::encode(title)
        );
        let resp = self.get(&url).send().await?;
        let data = resp.json::<Vec<KissKhSearchItem>>().await?;
        if let Ok(s) = serde_json::to_string(&data) {
            self.catalog_cache.insert(ck, s).await;
        }
        Ok(data)
    }

    pub async fn get_episode_id(&self, show_id: u64, ep_num: u32) -> Result<Option<u64>> {
        let url = format!("{}/DramaList/Drama/{}?is_series=1", API_BASE, show_id);
        let resp = self.get(&url).send().await?;
        let data = resp.json::<KissKhDramaDetail>().await?;

        let ep = data
            .episodes
            .iter()
            .find(|e| e.number.floor() as u32 == ep_num);
        Ok(ep.map(|e| e.id))
    }

    pub async fn get_stream(&self, ep_id: u64) -> Result<Option<KissKhStreamPayload>> {
        // Generate a real kkey using the cracked algorithm
        let kkey = match self.generate_kkey(ep_id, KISSKH_STREAM_VIGUID).await {
            Ok(k) => k,
            Err(e) => {
                warn!(
                    "[KissKH] kkey generation failed: {}, falling back to kkey=0",
                    e
                );
                "0".to_string()
            }
        };

        let url = format!(
            "{}/DramaList/Episode/{}.png?err=false&ts=0&time=0&kkey={}",
            API_BASE, ep_id, kkey
        );
        let resp = self.get(&url).send().await?;

        let text = resp.text().await?;
        if text.contains("<!DOCTYPE") {
            return Ok(None);
        }

        if let Ok(data) = serde_json::from_str::<KissKhStreamPayload>(&text) {
            return Ok(Some(data));
        }

        if let Ok(decoded_bytes) = general_purpose::STANDARD.decode(&text) {
            if let Ok(decoded_text) = String::from_utf8(decoded_bytes) {
                if let Ok(data) = serde_json::from_str::<KissKhStreamPayload>(&decoded_text) {
                    return Ok(Some(data));
                }
            }
        }

        Ok(None)
    }

    /// Fetch subtitle list for an episode using the subtitle-specific kkey.
    pub async fn get_subtitles(&self, ep_id: u64) -> Result<Vec<KissKhSubtitle>> {
        let kkey = match self.generate_kkey(ep_id, KISSKH_SUB_VIGUID).await {
            Ok(k) => k,
            Err(e) => {
                warn!(
                    "[KissKH] subtitle kkey generation failed: {}, falling back to kkey=0",
                    e
                );
                "0".to_string()
            }
        };

        let url = format!("{}/Sub/{}?kkey={}", API_BASE, ep_id, kkey);
        let resp = self.get(&url).send().await?;

        let text = resp.text().await?;
        if text.contains("<!DOCTYPE") {
            return Ok(vec![]);
        }

        // Try direct JSON first
        if let Ok(data) = serde_json::from_str::<Vec<KissKhSubtitle>>(&text) {
            return Ok(data);
        }

        // Try base64-encoded JSON
        if let Ok(decoded_bytes) = general_purpose::STANDARD.decode(&text) {
            if let Ok(decoded_text) = String::from_utf8(decoded_bytes) {
                if let Ok(data) = serde_json::from_str::<Vec<KissKhSubtitle>>(&decoded_text) {
                    return Ok(data);
                }
            }
        }

        Ok(vec![])
    }

    pub async fn get_streams_normalized(
        &self,
        title: &str,
        _season: u32,
        ep_num: u32,
    ) -> Result<Vec<StreamResult>> {
        info!("[KissKH Rust] Searching for: \"{}\" E{}", title, ep_num);

        let results = self.search(title).await?;
        if results.is_empty() {
            return Ok(vec![]);
        }

        let mut sorted = results;
        sorted.sort_by_key(|i| i.title.len());
        let best_match = &sorted[0];

        self.resolve_episode_streams(best_match.id, ep_num, title)
            .await
    }

    pub async fn get_streams_direct(
        &self,
        id_or_title: &str,
        ep_num: u32,
    ) -> Result<Vec<StreamResult>> {
        info!(
            "[KissKH Rust] Smart Fetch for: \"{}\" E{}",
            id_or_title, ep_num
        );

        let show_id = if let Ok(id) = id_or_title.parse::<u64>() {
            id
        } else {
            let results = self.search(id_or_title).await?;
            if results.is_empty() {
                info!("[KissKH Rust] Title not found: {}", id_or_title);
                return Ok(vec![]);
            }
            let mut sorted = results;
            sorted.sort_by_key(|i| i.title.len());
            sorted[0].id
        };

        self.resolve_episode_streams(show_id, ep_num, id_or_title)
            .await
    }

    async fn resolve_episode_streams(
        &self,
        show_id: u64,
        ep_num: u32,
        title: &str,
    ) -> Result<Vec<StreamResult>> {
        let ep_id = match self.get_episode_id(show_id, ep_num).await? {
            Some(id) => id,
            None => {
                info!(
                    "[KissKH Rust] Episode {} not found for \"{}\"",
                    ep_num, title
                );
                return Ok(vec![]);
            }
        };

        match self.get_stream(ep_id).await? {
            Some(data) => {
                let mut results = Vec::new();
                let mut headers = HashMap::new();
                headers.insert("Referer".to_string(), "https://kisskh.ovh".to_string());
                headers.insert("User-Agent".to_string(), USER_AGENT.to_string());

                // Subtitles may come from the stream payload or a separate endpoint
                let subs = if let Some(s_list) = data.subtitles {
                    Some(
                        s_list
                            .into_iter()
                            .map(|s| Subtitle {
                                url: s.src,
                                lang: s.lang.unwrap_or_else(|| "en".to_string()),
                                label: s.label.unwrap_or_else(|| "English".to_string()),
                            })
                            .collect(),
                    )
                } else {
                    match self.get_subtitles(ep_id).await {
                        Ok(s_list) if !s_list.is_empty() => Some(
                            s_list
                                .into_iter()
                                .map(|s| Subtitle {
                                    url: s.src,
                                    lang: s.lang.unwrap_or_else(|| "en".to_string()),
                                    label: s.label.unwrap_or_else(|| "English".to_string()),
                                })
                                .collect(),
                        ),
                        _ => None,
                    }
                };

                // ── Video CDN first ──────────────────────────────────────────────
                // ThirdParty URLs (e.g. dwish.pro) now require browser fingerprinting
                // and return HTML instead of HLS. Video CDN works reliably.
                //
                // Unreleased/upcoming episodes have no video yet: kisskh returns a
                // countdown-widget URL (tickcounter.com) in the Video field. Only
                // accept URLs that actually look like a playable stream, so those
                // placeholders don't reach the player (where they 502 in a loop).
                if let Some(video_url) = data.video {
                    if is_playable_video_url(&video_url) {
                        results.push(StreamResult {
                            url: video_url,
                            title: "Zenith - Auto".to_string(),
                            quality: "Auto".to_string(),
                            provider: "Zenith".to_string(),
                            format: "hls".to_string(),
                            subtitles: subs.clone(),
                            headers: Some(headers.clone()),
                            extra_metadata: None,
                        });
                    }
                }
                // Only include ThirdParty if it's from a known-good CDN (not dwish.pro)
                if let Some(third_party_url) = data.third_party {
                    if is_playable_video_url(&third_party_url) && !third_party_url.contains("dwish.pro") {
                        results.push(StreamResult {
                            url: third_party_url,
                            title: "Zenith - Auto (ThirdParty)".to_string(),
                            quality: "Auto (ThirdParty)".to_string(),
                            provider: "Zenith".to_string(),
                            format: "hls".to_string(),
                            subtitles: subs,
                            headers: Some(headers),
                            extra_metadata: None,
                        });
                    }
                }
                Ok(results)
            }
            None => Ok(vec![]),
        }
    }

    pub async fn get_show_list(&self) -> Result<Vec<serde_json::Value>> {
        let ck = "show_list";
        if let Some(s) = self.catalog_cache.get(ck).await {
            if let Ok(v) = serde_json::from_str(&s) {
                return Ok(v);
            }
        }
        let url = format!("{}/DramaList/Show", API_BASE);
        let resp = self.get(&url).send().await?;
        let data = resp.json::<Vec<serde_json::Value>>().await?;
        if let Ok(s) = serde_json::to_string(&data) {
            self.catalog_cache.insert(ck.to_string(), s).await;
        }
        Ok(data)
    }

    pub async fn get_list(
        &self,
        page: u32,
        page_size: u32,
        type_: u32,
        sub: u32,
        country: u32,
        status: u32,
        order: u32,
    ) -> Result<serde_json::Value> {
        let ck = format!(
            "list:{}:{}:{}:{}:{}:{}:{}",
            page, page_size, type_, sub, country, status, order
        );
        if let Some(s) = self.catalog_cache.get(&ck).await {
            if let Ok(v) = serde_json::from_str(&s) {
                return Ok(v);
            }
        }
        // kisskh honors &pageSize (unlike limit/size/perPage). Forwarding it lets
        // the grid pull a full page in one request instead of 10 at a time.
        let url = format!(
            "{}/DramaList/List?page={}&pageSize={}&type={}&sub={}&country={}&status={}&order={}",
            API_BASE, page, page_size, type_, sub, country, status, order
        );
        let resp = self.get(&url).send().await?;
        let data = resp.json::<serde_json::Value>().await?;
        if let Ok(s) = serde_json::to_string(&data) {
            self.catalog_cache.insert(ck, s).await;
        }
        Ok(data)
    }

    pub async fn get_upcoming(&self) -> Result<Vec<serde_json::Value>> {
        let ck = "upcoming";
        if let Some(s) = self.catalog_cache.get(ck).await {
            if let Ok(v) = serde_json::from_str(&s) {
                return Ok(v);
            }
        }
        let url = format!("{}/DramaList/upcoming", API_BASE);
        let resp = self.get(&url).send().await?;
        let data = resp.json::<Vec<serde_json::Value>>().await?;
        if let Ok(s) = serde_json::to_string(&data) {
            self.catalog_cache.insert(ck.to_string(), s).await;
        }
        Ok(data)
    }

    pub async fn get_top_rating(&self) -> Result<Vec<serde_json::Value>> {
        let ck = "top_rating";
        if let Some(s) = self.catalog_cache.get(ck).await {
            if let Ok(v) = serde_json::from_str(&s) {
                return Ok(v);
            }
        }
        let url = format!("{}/DramaList/TopRating?ispc=false", API_BASE);
        let resp = self.get(&url).send().await?;
        let data = resp.json::<Vec<serde_json::Value>>().await?;
        if let Ok(s) = serde_json::to_string(&data) {
            self.catalog_cache.insert(ck.to_string(), s).await;
        }
        Ok(data)
    }

    /// Most-viewed titles for a country code (c=1 China, c=2 Korea, …). Powers
    /// the "Top C-Drama" / "Top K-Drama" home shelves.
    pub async fn get_most_view(&self, country: u32) -> Result<Vec<serde_json::Value>> {
        let ck = format!("most_view:{}", country);
        if let Some(s) = self.catalog_cache.get(&ck).await {
            if let Ok(v) = serde_json::from_str(&s) {
                return Ok(v);
            }
        }
        let url = format!("{}/DramaList/MostView?ispc=true&c={}", API_BASE, country);
        let resp = self.get(&url).send().await?;
        let data = resp.json::<Vec<serde_json::Value>>().await?;
        if let Ok(s) = serde_json::to_string(&data) {
            self.catalog_cache.insert(ck, s).await;
        }
        Ok(data)
    }

    /// Recently-updated titles (the "Last Update" home shelf).
    pub async fn get_last_update(&self) -> Result<Vec<serde_json::Value>> {
        let ck = "last_update";
        if let Some(s) = self.catalog_cache.get(ck).await {
            if let Ok(v) = serde_json::from_str(&s) {
                return Ok(v);
            }
        }
        let url = format!("{}/DramaList/LastUpdate?ispc=true", API_BASE);
        let resp = self.get(&url).send().await?;
        let data = resp.json::<Vec<serde_json::Value>>().await?;
        if let Ok(s) = serde_json::to_string(&data) {
            self.catalog_cache.insert(ck.to_string(), s).await;
        }
        Ok(data)
    }

    pub async fn get_detail(&self, id: u64) -> Result<serde_json::Value> {
        let ck = format!("detail:{}", id);
        if let Some(s) = self.catalog_cache.get(&ck).await {
            if let Ok(v) = serde_json::from_str(&s) {
                return Ok(v);
            }
        }
        let url = format!("{}/DramaList/Drama/{}?isq=false", API_BASE, id);
        let resp = self.get(&url).send().await?;
        let data = resp.json::<serde_json::Value>().await?;
        if let Ok(s) = serde_json::to_string(&data) {
            self.catalog_cache.insert(ck, s).await;
        }
        Ok(data)
    }
}
