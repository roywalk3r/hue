/*
 * HUE ENGINE ❄
 * Second-level iframe extraction for donghua embeds.
 *
 * Converts `iframe` StreamPayload URLs into direct HLS/MP4 URLs where the
 * embed host exposes them in page source or a known API.
 */

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Result};
use rand::distributions::{Alphanumeric, DistString};
use regex::Regex;
use reqwest::header::{HeaderMap, HeaderValue};
use reqwest_middleware::ClientWithMiddleware;
use serde_json::Value;
use tokio::process::Command;
use tracing::{debug, trace};

const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

/// A native stream discovered inside an iframe embed page.
#[derive(Debug, Clone)]
pub struct NativeStream {
    /// Direct m3u8/mp4 URL.
    pub url: String,
    /// `hls` or `mp4`.
    pub stream_type: String,
    /// Referer the player should send when requesting this stream.
    pub referer: Option<String>,
}

/// Attempt to turn an iframe URL into a native stream.
///
/// Dispatches to host-specific extractors when available, then falls back to a
/// generic regex scan of the embed page source. If both fail and a Node
/// scraper microservice URL is configured, asks it to extract the iframe as a
/// last resort (useful for packed/encoded JS hosts we haven't ported to Rust).
pub async fn try_extract(
    client: &ClientWithMiddleware,
    iframe_url: &str,
    source_referer: &str,
) -> Option<NativeStream> {
    if iframe_url.is_empty() {
        return None;
    }

    let lower = iframe_url.to_ascii_lowercase();

    if is_dood_host(&lower) {
        if let Some(stream) = dood_extract(client, iframe_url)
            .await
            .inspect_err(|e| debug!("[Dood] extraction failed: {}", e))
            .ok()
        {
            return Some(stream);
        }
    }

    if is_streamwish_host(&lower) {
        if let Some(stream) = streamwish_extract(client, iframe_url, source_referer)
            .await
            .inspect_err(|e| debug!("[Streamwish] extraction failed: {}", e))
            .ok()
        {
            return Some(stream);
        }
    }

    if is_okru_host(&lower) {
        if let Some(stream) = okru_extract(client, iframe_url, source_referer)
            .await
            .inspect_err(|e| debug!("[OK.ru] extraction failed: {}", e))
            .ok()
        {
            return Some(stream);
        }
    }

    if is_dailymotion_host(&lower) {
        if let Some(stream) = dailymotion_extract(client, iframe_url, source_referer)
            .await
            .inspect_err(|e| debug!("[Dailymotion] extraction failed: {}", e))
            .ok()
        {
            return Some(stream);
        }
    }

    if let Some(stream) = generic_extract(client, iframe_url, source_referer)
        .await
        .inspect_err(|e| debug!("[iframe] generic extraction failed: {}", e))
        .ok()
    {
        return Some(stream);
    }

    // Last resort: Node scraper microservice for hosts with packed/headless
    // extraction logic we haven't ported to Rust yet.
    node_scraper_extract(client, iframe_url, source_referer)
        .await
        .inspect_err(|e| debug!("[NodeScraper] fallback extraction failed: {}", e))
        .ok()
}

fn is_dood_host(url: &str) -> bool {
    url.contains("dood")
        || url.contains("d000d")
        || url.contains("playmogo.com")
        || url.contains("dooood.com")
        || url.contains("ds2play")
        || url.contains("doods")
        || url.contains("doodstream")
}

fn is_streamwish_host(url: &str) -> bool {
    url.contains("streamwish")
        || url.contains("seekplayer")
        || url.contains("wishplayer")
        || url.contains("embedwish")
        || url.contains("wishembed")
}

fn is_okru_host(url: &str) -> bool {
    url.contains("ok.ru") || url.contains("odnoklassniki.ru")
}

fn is_dailymotion_host(url: &str) -> bool {
    url.contains("dailymotion.com")
}

fn default_headers(referer: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert("User-Agent", HeaderValue::from_static(UA));
    headers.insert(
        "Accept",
        HeaderValue::from_static(
            "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,*/*;q=0.8",
        ),
    );
    if let Ok(value) = HeaderValue::from_str(referer) {
        headers.insert("Referer", value);
    }
    headers
}

fn default_headers_with_cookie(referer: &str, cookie: &str) -> HeaderMap {
    let mut headers = default_headers(referer);
    if let Ok(value) = HeaderValue::from_str(cookie) {
        headers.insert("Cookie", value);
    }
    headers
}

/// Decode a small set of HTML entities found in `data-options` attributes.
fn decode_html_entities(s: &str) -> String {
    s.replace("&quot;", "\"")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&#39;", "'")
}

// ═══════════════════════════════════════════════════════════════════════════
// Dood family
// ═══════════════════════════════════════════════════════════════════════════

/// Dood embeds expose `/pass_md5/<hash>/<token>` in their page source. The
/// endpoint returns the base CDN URL; the final MP4 needs a random 10-char
/// suffix plus `?token=<token>&expiry=<timestamp>`.
///
/// We shell out to `curl` here because Dood's CDN blocks the Rust reqwest
/// fingerprint (returns 403) while a real browser/curl handshake succeeds.
async fn dood_extract(_client: &ClientWithMiddleware, iframe_url: &str) -> Result<NativeStream> {
    let embed = reqwest::Url::parse(iframe_url)
        .map_err(|e| anyhow!("invalid dood embed URL: {e}"))?;
    let embed_origin = format!("{}://{}", embed.scheme(), embed.host_str().unwrap_or(""));

    let html = curl_get(iframe_url, &embed_origin).await?;
    debug!("[Dood] embed {} body_len={}", iframe_url, html.len());

    // The embed page performs `$.get('/pass_md5/<hash>/<token>', ...)`.
    let re = Regex::new(r#"/pass_md5/([^'"\s]+)"#)?;
    let pass_path = re
        .captures(&html)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str())
        .ok_or_else(|| {
            debug!("[Dood] pass_md5 path not found in {} bytes", html.len());
            anyhow!("pass_md5 path not found in dood embed")
        })?;

    let token = pass_path
        .rsplit('/')
        .next()
        .ok_or_else(|| anyhow!("pass_md5 token missing"))?
        .to_string();

    let pass_url = format!("{embed_origin}/pass_md5/{pass_path}");
    trace!("[Dood] pass_md5 URL: {}", pass_url);

    let base_url = curl_get(&pass_url, iframe_url).await?;
    debug!(
        "[Dood] pass_md5 {} body_len={} body={:?}",
        pass_url,
        base_url.len(),
        &base_url[..base_url.len().min(120)]
    );

    if base_url.is_empty() {
        return Err(anyhow!("pass_md5 returned empty base URL"));
    }
    if !base_url.starts_with("http") {
        return Err(anyhow!("pass_md5 returned unexpected base URL: {}", base_url));
    }

    let suffix = Alphanumeric.sample_string(&mut rand::thread_rng(), 10);
    let expiry = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let url = format!("{}{}?token={}&expiry={}", base_url, suffix, token, expiry);

    debug!("[Dood] extracted native mp4 for {}", iframe_url);
    Ok(NativeStream {
        url,
        stream_type: "mp4".to_string(),
        referer: Some(iframe_url.to_string()),
    })
}

/// Browser-like GET via `curl`. Used for hosts that fingerprint reqwest as a bot.
async fn curl_get(url: &str, referer: &str) -> Result<String> {
    let output = Command::new("curl")
        .args([
            "-s",
            "-L",
            "--max-time",
            "20",
            "--http1.1",
            "-A",
            UA,
            "-H",
            &format!("Referer: {referer}"),
            "-H",
            "Accept: text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,*/*;q=0.8",
            url,
        ])
        .output()
        .await
        .map_err(|e| anyhow!("curl spawn failed for {url}: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("curl failed for {url}: {stderr}"));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

// ═══════════════════════════════════════════════════════════════════════════
// Streamwish / Vidstack family (best-effort)
// ═══════════════════════════════════════════════════════════════════════════

/// Streamwish and its clones use an encrypted `/api/v1/video?id=<hash>`
/// response. We currently try the easy wins (direct `.m3u8`/`.mp4` in page JS)
/// and intentionally bail when the API returns encrypted payloads.
async fn streamwish_extract(
    client: &ClientWithMiddleware,
    iframe_url: &str,
    source_referer: &str,
) -> Result<NativeStream> {
    let html = fetch_embed_page(client, iframe_url, source_referer)
        .await
        .map_err(|e| anyhow!("failed to fetch streamwish embed: {e}"))?;

    // The Vidstack app sometimes inlines a sources array before encryption.
    if let Some(stream) = find_native_url(&html) {
        return Ok(stream);
    }

    // API path is known; response is encrypted, so a clean server-side
    // extractor is not yet implemented. Return a clear error so callers keep
    // serving the original iframe.
    Err(anyhow!("streamwish API response is encrypted"))
}

// ═══════════════════════════════════════════════════════════════════════════
// OK.ru (Odnoklassniki)
// ═══════════════════════════════════════════════════════════════════════════

async fn okru_extract(
    client: &ClientWithMiddleware,
    iframe_url: &str,
    source_referer: &str,
) -> Result<NativeStream> {
    let referer = if source_referer.is_empty() {
        "https://ok.ru/"
    } else {
        source_referer
    };
    let html = fetch_embed_page(client, iframe_url, referer)
        .await
        .map_err(|e| anyhow!("failed to fetch ok.ru embed: {e}"))?;

    // Newer embeds inline an escaped `"metadata":"..."` JSON blob.
    if let Some(meta) = extract_okru_metadata(&html) {
        if let Some(m3u8) = find_okru_hls(&meta) {
            return Ok(NativeStream {
                url: m3u8,
                stream_type: "hls".to_string(),
                referer: Some("https://ok.ru/".to_string()),
            });
        }
    }

    // Fallback: the `data-options` attribute holds player JSON with flashvars.
    let player_json = extract_okru_data_options(&html);
    if let Some(player_json) = player_json {
        let player: Value = serde_json::from_str(&player_json)
            .map_err(|e| anyhow!("ok.ru data-options is not valid JSON: {e}"))?;

        if let Some(metadata_json) = player
            .get("flashvars")
            .and_then(|f| f.get("metadata"))
            .and_then(|m| m.as_str())
        {
            let metadata: Value = serde_json::from_str(metadata_json)
                .map_err(|e| anyhow!("ok.ru flashvars.metadata is not valid JSON: {e}"))?;
            if let Some(m3u8) = find_okru_hls(&metadata) {
                return Ok(NativeStream {
                    url: m3u8,
                    stream_type: "hls".to_string(),
                    referer: Some("https://ok.ru/".to_string()),
                });
            }
        }

        if let Some(metadata_url) = player
            .get("flashvars")
            .and_then(|f| f.get("metadataUrl"))
            .and_then(|m| m.as_str())
        {
            let metadata_url = decode_html_entities(metadata_url);
            let metadata = client
                .get(&metadata_url)
                .headers(default_headers(referer))
                .send()
                .await?
                .json::<Value>()
                .await?;
            if let Some(m3u8) = find_okru_hls(&metadata) {
                return Ok(NativeStream {
                    url: m3u8,
                    stream_type: "hls".to_string(),
                    referer: Some("https://ok.ru/".to_string()),
                });
            }
        }
    }

    Err(anyhow!("ok.ru: no hls manifest found"))
}

fn extract_okru_metadata(html: &str) -> Option<Value> {
    let marker = "\"metadata\":\"";
    let idx = html.find(marker)?;
    let rest = &html[idx + marker.len()..];

    let mut raw = String::new();
    let mut escaped = false;
    for ch in rest.chars() {
        if escaped {
            raw.push(ch);
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == '"' {
            break;
        }
        raw.push(ch);
    }

    let json_str = raw
        .replace("\\\"", "\"")
        .replace("\\n", "\n")
        .replace("\\/", "/")
        .replace("\\\\", "\\");
    serde_json::from_str(&json_str).ok()
}

/// Extract the JSON payload from an OK.ru `data-options` attribute. We try
/// double-quoted and single-quoted forms separately because the Rust regex
/// engine does not support backreferences.
fn extract_okru_data_options(html: &str) -> Option<String> {
    let double_re = Regex::new(r#"data-options="(\{.+?\})""#).ok()?;
    if let Some(caps) = double_re.captures(html) {
        return caps.get(1).map(|m| decode_html_entities(m.as_str()));
    }
    let single_re = Regex::new(r#"data-options='(\{.+?\})'"#).ok()?;
    if let Some(caps) = single_re.captures(html) {
        return caps.get(1).map(|m| decode_html_entities(m.as_str()));
    }
    None
}

fn find_okru_hls(meta: &Value) -> Option<String> {
    ["hlsManifestUrl", "ondemandHls", "hlsMasterPlaylistUrl"]
        .iter()
        .find_map(|key| meta.get(*key).and_then(|v| v.as_str()).map(String::from))
}

// ═══════════════════════════════════════════════════════════════════════════
// Dailymotion
// ═══════════════════════════════════════════════════════════════════════════

async fn dailymotion_extract(
    client: &ClientWithMiddleware,
    iframe_url: &str,
    source_referer: &str,
) -> Result<NativeStream> {
    let video_id = extract_dailymotion_id(iframe_url)
        .ok_or_else(|| anyhow!("dailymotion: video id not found"))?;
    let meta_url = format!(
        "https://www.dailymotion.com/player/metadata/video/{}",
        video_id
    );
    let referer = if source_referer.is_empty() {
        "https://www.dailymotion.com/"
    } else {
        source_referer
    };

    let resp = client
        .get(&meta_url)
        .headers(default_headers_with_cookie(referer, "family_filter=off; ff=off"))
        .send()
        .await?
        .json::<Value>()
        .await?;

    let auto = resp
        .get("qualities")
        .and_then(|q| q.get("auto"))
        .and_then(|a| a.as_array());

    if let Some(qualities) = auto {
        for q in qualities {
            let url = q.get("url").and_then(|u| u.as_str());
            let is_hls = q
                .get("type")
                .and_then(|t| t.as_str())
                .map(|t| t == "application/x-mpegURL")
                .unwrap_or(false)
                || url.map(|u| u.contains(".m3u8")).unwrap_or(false);
            if is_hls {
                if let Some(url) = url {
                    return Ok(NativeStream {
                        url: url.to_string(),
                        stream_type: "hls".to_string(),
                        referer: Some("https://www.dailymotion.com/".to_string()),
                    });
                }
            }
        }
    }

    Err(anyhow!("dailymotion: no hls manifest found"))
}

fn extract_dailymotion_id(url: &str) -> Option<String> {
    let re = Regex::new(r#"(?:/embed/video/|/video/)([a-zA-Z0-9]+)"#).ok()?;
    re.captures(url)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
}

// ═══════════════════════════════════════════════════════════════════════════
// Node scraper microservice fallback
// ═══════════════════════════════════════════════════════════════════════════

/// URL of the Node scraper microservice. Set `SCRAPER_URL` to enable the
/// fallback path. Defaults to the local PM2-managed scraper.
fn scraper_url() -> Option<String> {
    std::env::var("SCRAPER_URL")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| Some("http://127.0.0.1:24610".to_string()))
}

async fn node_scraper_extract(
    client: &ClientWithMiddleware,
    iframe_url: &str,
    source_referer: &str,
) -> Result<NativeStream> {
    let base = scraper_url().ok_or_else(|| anyhow!("no scraper URL configured"))?;

    // First try the per-host iframe extractor route (fast, no browser).
    let iframe_url_path = format!("{base}/extract/iframe");
    let iframe_payload = serde_json::json!({
        "url": iframe_url,
        "referer": source_referer,
    });

    let iframe_resp = client
        .post(&iframe_url_path)
        .timeout(Duration::from_secs(8))
        .json(&iframe_payload)
        .send()
        .await
        .map_err(|e| anyhow!("scraper request failed: {e}"))?
        .json::<Value>()
        .await
        .map_err(|e| anyhow!("scraper returned invalid JSON: {e}"))?;

    if let Some(stream) = parse_scraper_stream(&iframe_resp, iframe_url) {
        return Ok(stream);
    }

    // If the host-specific extractor didn't handle it, fall back to a real
    // browser that sniffs the first media request the player makes.
    debug!("[NodeScraper] iframe extraction failed, trying headless sniffer for {}", iframe_url);
    node_headless_extract(client, iframe_url, source_referer, &base).await
}

async fn node_headless_extract(
    client: &ClientWithMiddleware,
    iframe_url: &str,
    source_referer: &str,
    base: &str,
) -> Result<NativeStream> {
    let url = format!("{base}/scrape/headless");
    let headless_timeout_ms = 10_000;

    let payload = serde_json::json!({
        "url": iframe_url,
        "referer": source_referer,
        "timeout": headless_timeout_ms,
        "mediaExtensions": [".m3u8", ".mp4", ".mpd", ".mkv", ".webm"],
    });

    let resp = client
        .post(&url)
        .timeout(Duration::from_millis(headless_timeout_ms + 3_000))
        .json(&payload)
        .send()
        .await
        .map_err(|e| anyhow!("headless scraper request failed: {e}"))?
        .json::<Value>()
        .await
        .map_err(|e| anyhow!("headless scraper returned invalid JSON: {e}"))?;

    parse_scraper_stream(&resp, iframe_url)
        .ok_or_else(|| anyhow!("headless scraper did not capture a media request"))
}

fn parse_scraper_stream(resp: &Value, iframe_url: &str) -> Option<NativeStream> {
    if !resp.get("success").and_then(|v| v.as_bool()).unwrap_or(false) {
        return None;
    }

    let stream_url = resp
        .get("url")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())?;

    let stream_type = resp
        .get("streamType")
        .and_then(|v| v.as_str())
        .unwrap_or("mp4")
        .to_string();

    let referer = resp.get("referer").and_then(|v| v.as_str()).map(String::from);

    debug!("[NodeScraper] extracted {} stream from {}", stream_type, iframe_url);
    Some(NativeStream {
        url: stream_url.to_string(),
        stream_type,
        referer,
    })
}

// ═══════════════════════════════════════════════════════════════════════════
// Generic fallback
// ═══════════════════════════════════════════════════════════════════════════

async fn generic_extract(
    client: &ClientWithMiddleware,
    iframe_url: &str,
    source_referer: &str,
) -> Result<NativeStream> {
    let html = fetch_embed_page(client, iframe_url, source_referer)
        .await
        .map_err(|e| anyhow!("failed to fetch iframe page: {e}"))?;

    find_native_url(&html).ok_or_else(|| anyhow!("no direct stream URL found in iframe"))
}

async fn fetch_embed_page(
    client: &ClientWithMiddleware,
    iframe_url: &str,
    source_referer: &str,
) -> Result<String> {
    Ok(client
        .get(iframe_url)
        .headers(default_headers(source_referer))
        .send()
        .await?
        .text()
        .await?)
}

/// Scan HTML/JS for an unprotected m3u8 or mp4 URL.
fn find_native_url(html: &str) -> Option<NativeStream> {
    // m3u8 first — prefer HLS over MP4.
    let m3u8_re = Regex::new(r#"(https?://[^"'\s<>]+?\.m3u8(?:\?[^"'\s<>]*)?)"#).ok()?;
    if let Some(cap) = m3u8_re.captures(html) {
        let url = cap.get(1)?.as_str().to_string();
        if !looks_like_ad(&url) {
            return Some(NativeStream {
                url,
                stream_type: "hls".to_string(),
                referer: None,
            });
        }
    }

    let mp4_re = Regex::new(r#"(https?://[^"'\s<>]+?\.mp4(?:\?[^"'\s<>]*)?)"#).ok()?;
    if let Some(cap) = mp4_re.captures(html) {
        let url = cap.get(1)?.as_str().to_string();
        if !looks_like_ad(&url) {
            return Some(NativeStream {
                url,
                stream_type: "mp4".to_string(),
                referer: None,
            });
        }
    }

    None
}

fn looks_like_ad(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    lower.contains("adserver")
        || lower.contains("adsystem")
        || lower.contains("doubleclick")
        || lower.contains("googleads")
        || lower.contains("pop")
        || lower.contains("pre-roll")
        || lower.contains("/ads/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_looks_like_ad_filters_common_ad_hosts() {
        assert!(looks_like_ad("https://adserver.example/vast.mp4"));
        assert!(looks_like_ad("https://googleads.g.doubleclick.net/pagead/ads?client=ca"));
        assert!(!looks_like_ad("https://cdn.example/video/1.m3u8"));
    }

    #[test]
    fn test_find_native_url_prefers_hls() {
        let html = r#"var sources = [{"file":"https://cdn.example/playlist.m3u8","label":"HD"},{"file":"https://cdn.example/video.mp4"}];"#;
        let stream = find_native_url(html).unwrap();
        assert_eq!(stream.url, "https://cdn.example/playlist.m3u8");
        assert_eq!(stream.stream_type, "hls");
    }

    #[test]
    fn test_find_native_url_falls_back_to_mp4() {
        let html = r#"sources: [{"file":"https://cdn.example/video.mp4"}]"#;
        let stream = find_native_url(html).unwrap();
        assert_eq!(stream.stream_type, "mp4");
    }

    #[test]
    fn test_find_native_url_ignores_ads() {
        let html = r#""file":"https://adserver.example/preroll.mp4""#;
        assert!(find_native_url(html).is_none());
    }

    #[test]
    fn test_dood_pass_md5_regex() {
        let html = r#"$.get('/pass_md5/abc123-0-0-0-deadbeef/token00x', function(data) { makePlay(data, 'token00x'); });"#;
        let re = Regex::new(r#"/pass_md5/([^'"\s]+)"#).unwrap();
        let caps = re.captures(html).unwrap();
        assert_eq!(caps.get(1).unwrap().as_str(), "abc123-0-0-0-deadbeef/token00x");
    }

    #[test]
    fn test_extract_dailymotion_id_embed_and_video_paths() {
        assert_eq!(
            extract_dailymotion_id("https://www.dailymotion.com/embed/video/x8ao5h7"),
            Some("x8ao5h7".to_string())
        );
        assert_eq!(
            extract_dailymotion_id("https://www.dailymotion.com/video/x8ao5h7"),
            Some("x8ao5h7".to_string())
        );
        assert_eq!(
            extract_dailymotion_id("https://www.dailymotion.com/embed/video/x8ao5h7?autoplay=1"),
            Some("x8ao5h7".to_string())
        );
        assert!(extract_dailymotion_id("https://www.dailymotion.com/").is_none());
    }

    #[test]
    fn test_extract_okru_metadata_reads_escaped_json() {
        let html = r#"var data = {"metadata":"{\"hlsManifestUrl\":\"https://ok.ru/video/hls/123.m3u8\"}"};"#;
        let meta = extract_okru_metadata(html).unwrap();
        assert_eq!(
            meta.get("hlsManifestUrl").unwrap().as_str().unwrap(),
            "https://ok.ru/video/hls/123.m3u8"
        );
    }

    #[test]
    fn test_find_okru_hls_prefers_hls_manifest_url() {
        let meta = serde_json::json!({
            "hlsManifestUrl": "https://ok.ru/manifest.m3u8",
            "ondemandHls": "https://ok.ru/ondemand.m3u8",
        });
        assert_eq!(find_okru_hls(&meta), Some("https://ok.ru/manifest.m3u8".to_string()));
    }

    #[test]
    fn test_decode_html_entities() {
        assert_eq!(decode_html_entities("&quot;hello&quot;"), "\"hello\"");
        assert_eq!(decode_html_entities("&amp;"), "&");
        assert_eq!(decode_html_entities("&#39;x&#39;"), "'x'");
    }

    /// The headless scraper returns a JSON envelope; `parse_scraper_stream`
    /// pulls out the captured stream URL, stream type, and referer. Anything
    /// missing `success=true` or `url` should return None so the caller falls
    /// through to a different strategy.
    #[test]
    fn test_parse_scraper_stream_happy_path() {
        let resp = serde_json::json!({
            "success": true,
            "url": "https://cdn.example.com/master.m3u8",
            "streamType": "hls",
            "referer": "https://embed.example.com/",
        });
        let stream = parse_scraper_stream(&resp, "https://embed.example.com/")
            .expect("should parse");
        assert_eq!(stream.url, "https://cdn.example.com/master.m3u8");
        assert_eq!(stream.stream_type, "hls");
        assert_eq!(stream.referer.as_deref(), Some("https://embed.example.com/"));
    }

    #[test]
    fn test_parse_scraper_stream_fails_on_success_false() {
        let resp = serde_json::json!({ "success": false, "error": "timeout" });
        assert!(parse_scraper_stream(&resp, "https://x/").is_none());
    }

    #[test]
    fn test_parse_scraper_stream_fails_on_missing_url() {
        let resp = serde_json::json!({ "success": true, "streamType": "hls" });
        assert!(parse_scraper_stream(&resp, "https://x/").is_none());
    }

    /// Real-network smoke test for OK.ru extraction. Ignored in CI because it
    /// depends on a live embed page.
    #[tokio::test]
    #[ignore = "network"]
    async fn test_okru_extract_live() {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(20))
            .build()
            .unwrap();
        let client = crate::http_client::wrap_with_retry(client);

        let stream = okru_extract(
            &client,
            "https://ok.ru/video/3483213367904",
            "https://ok.ru/",
        )
        .await
        .expect("ok.ru extraction should succeed");

        assert_eq!(stream.stream_type, "hls");
        assert!(stream.url.contains(".m3u8"));
        assert!(stream.url.starts_with("https://"));
        assert_eq!(stream.referer, Some("https://ok.ru/".to_string()));
    }
}
