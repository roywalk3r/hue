/*
 * HUE ENGINE ❄
 * Second-level iframe extraction for donghua embeds.
 *
 * Converts `iframe` StreamPayload URLs into direct HLS/MP4 URLs where the
 * embed host exposes them in page source or a known API.
 */

use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Result};
use rand::distributions::{Alphanumeric, DistString};
use regex::Regex;
use reqwest::header::{HeaderMap, HeaderValue};
use reqwest_middleware::ClientWithMiddleware;
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
/// generic regex scan of the embed page source.
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
        return dood_extract(client, iframe_url)
            .await
            .inspect_err(|e| debug!("[Dood] extraction failed: {}", e))
            .ok();
    }

    if is_streamwish_host(&lower) {
        return streamwish_extract(client, iframe_url, source_referer)
            .await
            .inspect_err(|e| debug!("[Streamwish] extraction failed: {}", e))
            .ok();
    }

    generic_extract(client, iframe_url, source_referer)
        .await
        .inspect_err(|e| debug!("[iframe] generic extraction failed: {}", e))
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
}
