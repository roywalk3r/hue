#![allow(dead_code)]
/*
 * HUE ENGINE ❄
 * System Designation: The Architecture of Aggregation
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */

//! VidRock Provider — AES-256-CBC encrypted API for HD movies/TV streams
//!
//! API flow:
//!   1. AES-256-CBC encrypt item ID with static passphrase (base64url output)
//!   2. GET vidrock.net/api/{type}/{encrypted} → source URLs keyed by provider
//!   3. GET hls2.vdrk.site/playlist/{id} → resolution-specific MP4 URLs
//!   4. GET sub.vdrk.site/v2/{type}/{tmdbId} → VTT subtitle files

use serde::{Deserialize, Serialize};
use reqwest_middleware::ClientWithMiddleware;
use anyhow::{Result, anyhow};
use aes::Aes256;
use cbc::Encryptor;
use cbc::cipher::{KeyIvInit, BlockEncryptMut};
use cbc::cipher::block_padding::Pkcs7;
use base64::Engine;
use crate::constants::UA_DESKTOP;

type Aes256Cbc = Encryptor<Aes256>;

const PASSPHRASE: &str = "x7k9mPqT2rWvY8zA5bC3nF6hJ2lK4mN9";
const API_BASE: &str = "https://vidrock.net";
const SUB_BASE: &str = "https://sub.vdrk.site";

// ─── API Response Types ────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct SourceEntry {
    url: Option<String>,
    language: Option<String>,
    _flag: Option<String>,
    #[serde(rename = "type")]
    source_type: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PlaylistEntry {
    resolution: i32,
    url: String,
}

#[derive(Debug, Deserialize)]
struct SubtitleEntry {
    label: String,
    file: String,
}

// ─── Output Types ──────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Clone)]
pub struct VidRockStream {
    pub url: String,
    pub title: String,
    pub quality: String,
    pub format: String,
    pub provider: String,
    pub headers: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subtitles: Option<Vec<SubtitleInfo>>,
}

#[derive(Debug, Serialize, Clone)]
pub struct SubtitleInfo {
    pub url: String,
    pub lang: String,
    pub label: String,
}

// ─── Crypto ────────────────────────────────────────────────────────────────────

/// AES-256-CBC encrypt → base64url (matches Node.js encryptItemId)
fn encrypt_item_id(item_id: &str) -> Result<String> {
    let key = PASSPHRASE.as_bytes(); // 32 bytes
    let iv = &key[..16]; // first 16 bytes

    let plaintext = item_id.as_bytes();
    // Pad to block size
    let buf_len = plaintext.len() + 16;
    let mut buf = vec![0u8; buf_len];
    buf[..plaintext.len()].copy_from_slice(plaintext);

    let encrypted = Aes256Cbc::new(key.into(), iv.into())
        .encrypt_padded_mut::<Pkcs7>(&mut buf, plaintext.len())
        .map_err(|e| anyhow!("AES encrypt: {}", e))?;

    // base64url: + → -, / → _, strip =
    let b64 = base64::engine::general_purpose::STANDARD.encode(encrypted);
    let b64url = b64.replace('+', "-").replace('/', "_").trim_end_matches('=').to_string();

    Ok(b64url)
}

// ─── Client ────────────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct VidRockClient {
    client: ClientWithMiddleware,
}

impl VidRockClient {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(20))
            .connect_timeout(std::time::Duration::from_secs(10))
            .tcp_keepalive(std::time::Duration::from_secs(60))
            .pool_max_idle_per_host(4)
            .user_agent(UA_DESKTOP)
            .build()
            .unwrap();
        Self {
            client: crate::http_client::wrap_with_retry(client),
        }
    }

    pub async fn get_streams(&self, tmdb_id: &str, media_type: &str, season: Option<u32>, episode: Option<u32>) -> Result<Vec<VidRockStream>> {
        let (item_id, api_type) = if media_type == "tv" || media_type == "series" {
            let s = season.unwrap_or(1);
            let e = episode.unwrap_or(1);
            (format!("{}_{}_{}", tmdb_id, s, e), "tv")
        } else {
            (tmdb_id.to_string(), "movie")
        };

        let encrypted = encrypt_item_id(&item_id)?;
        let api_url = format!("{}/api/{}/{}", API_BASE, api_type, encrypted);
        let referer_url = format!("{}/{}/{}", API_BASE, api_type, tmdb_id);

        // Fetch source map from API
        let resp = self.client.get(&api_url)
            .header("Referer", &referer_url)
            .header("Origin", API_BASE)
            .header("Accept", "application/json, text/plain, */*")
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(anyhow!("VidRock API returned {}", resp.status()));
        }

        let sources: std::collections::HashMap<String, SourceEntry> = resp.json().await?;

        // Fetch subtitles in parallel
        let sub_url = format!("{}/v2/{}/{}", SUB_BASE, api_type, tmdb_id);
        let subs_fut = async {
            match self.client.get(&sub_url)
                .header("Referer", API_BASE)
                .send().await
            {
                Ok(r) if r.status().is_success() => {
                    r.json::<Vec<SubtitleEntry>>().await.ok().map(|entries| {
                        entries.into_iter().map(|s| SubtitleInfo {
                            url: s.file,
                            lang: s.label.split(',').next().unwrap_or(&s.label).to_string(),
                            label: s.label,
                        }).collect::<Vec<_>>()
                    })
                }
                _ => None,
            }
        };

        // Resolve playlist URLs in parallel
        let mut stream_futs = Vec::new();
        for (provider_name, entry) in &sources {
            if let Some(ref playlist_url) = entry.url {
                if playlist_url.is_empty() { continue; }
                let client = self.client.clone();
                let name = provider_name.clone();
                let lang = entry.language.clone().unwrap_or_else(|| "Unknown".to_string());
                let fmt = entry.source_type.clone().unwrap_or_else(|| "mp4".to_string());
                let url = playlist_url.clone();
                let referer = referer_url.clone();

                stream_futs.push(tokio::spawn(async move {
                    match client.get(&url).header("Referer", &referer).send().await {
                        Ok(r) if r.status().is_success() => {
                            match r.json::<Vec<PlaylistEntry>>().await {
                                Ok(playlist) => {
                                    playlist.into_iter().map(move |p| {
                                        let is_hls = p.url.contains(".m3u8") || fmt == "hls";
                                        VidRockStream {
                                            url: p.url,
                                            title: format!("VidRock {} - {}p - {}", name, p.resolution, lang),
                                            quality: format!("{}p", p.resolution),
                                            format: if is_hls { "hls".to_string() } else { fmt.clone() },
                                            provider: format!("VidRock/{}", name),
                                            headers: serde_json::json!({
                                                "Referer": referer,
                                                "Origin": API_BASE,
                                                "User-Agent": UA_DESKTOP
                                            }),
                                            subtitles: None,
                                        }
                                    }).collect::<Vec<_>>()
                                }
                                Err(_) => vec![]
                            }
                        }
                        _ => vec![]
                    }
                }));
            }
        }

        let (subs, stream_results) = tokio::join!(subs_fut, async {
            let mut all = Vec::new();
            for fut in stream_futs {
                if let Ok(streams) = fut.await {
                    all.extend(streams);
                }
            }
            all
        });

        // Attach subtitles to the first stream
        let mut streams = stream_results;
        if let Some(ref sub_list) = subs {
            if !streams.is_empty() && !sub_list.is_empty() {
                streams[0].subtitles = Some(sub_list.clone());
            }
        }

        // Sort by resolution descending
        streams.sort_by_key(|s| std::cmp::Reverse(s.quality.trim_end_matches('p').parse::<i32>().unwrap_or(0)));

        Ok(streams)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_item_id_deterministic() {
        let result1 = encrypt_item_id("12345").unwrap();
        let result2 = encrypt_item_id("12345").unwrap();
        assert_eq!(result1, result2);
    }

    #[test]
    fn test_encrypt_item_id_not_empty() {
        let result = encrypt_item_id("12345").unwrap();
        assert!(!result.is_empty());
    }

    #[test]
    fn test_encrypt_item_id_different_inputs() {
        let r1 = encrypt_item_id("111").unwrap();
        let r2 = encrypt_item_id("222").unwrap();
        assert_ne!(r1, r2);
    }

    #[test]
    fn test_encrypt_item_id_tv_format() {
        let result = encrypt_item_id("1399_1_5").unwrap();
        assert!(!result.is_empty());
    }

    #[test]
    fn test_encrypt_item_id_no_equals() {
        let result = encrypt_item_id("99999").unwrap();
        assert!(!result.contains('='));
    }

    #[test]
    fn test_encrypt_item_id_no_plus_slash() {
        let result = encrypt_item_id("12345").unwrap();
        assert!(!result.contains('+'));
        assert!(!result.contains('/'));
    }

    #[test]
    fn test_sort_by_resolution_descending() {
        // Verify the sort_key parsing logic
        let q = "1080p".trim_end_matches('p').parse::<i32>().unwrap_or(0);
        assert_eq!(q, 1080);
        let q = "720p".trim_end_matches('p').parse::<i32>().unwrap_or(0);
        assert_eq!(q, 720);
        let q = "4K".trim_end_matches('p').parse::<i32>().unwrap_or(0);
        assert_eq!(q, 0); // non-numeric defaults to 0

        // 1080p should sort before 720p (descending)
        let a = std::cmp::Reverse(1080);
        let b = std::cmp::Reverse(720);
        assert!(a < b);
    }
}
