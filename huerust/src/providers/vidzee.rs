/*
 * HUE ENGINE ❄
 * System Designation: The Architecture of Aggregation
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */

//! VidZee Provider — Two-layer AES encryption for streaming links
//!
//! API flow:
//!   1. GET core.vidzee.wtf/api-key → AES-256-GCM decrypt with SHA-256(hex_secret) → actual CBC key
//!   2. GET player.vidzee.wtf/api/server?id={tmdbId}&sr={serverId} → JSON with encrypted links
//!   3. AES-256-CBC decrypt each link using actual key → m3u8 playlist URL
//!   4. Tracks (subtitles) returned inline in the API response

use reqwest::Client;
use anyhow::{Result, anyhow};
use aes::Aes256;
use cbc::Decryptor as CbcDecryptor;
use cbc::cipher::{KeyIvInit, BlockDecryptMut};
use cbc::cipher::block_padding::Pkcs7;
use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use sha2::{Sha256, Digest};
use base64::Engine;
use regex::Regex;
use tracing::{info, warn};
use crate::models::{StreamResult, Subtitle};
use std::collections::HashMap;
use tokio::sync::OnceCell;

type Aes256Cbc = CbcDecryptor<Aes256>;

use crate::constants::UA_DESKTOP;

const VIDZEE_API: &str = "https://player.vidzee.wtf/api/server";
const DEFAULT_KEY_API: &str = "https://core.vidzee.wtf/api-key";
const DEFAULT_HEX_SECRET: &str = "4f2a9c7d1e8b3a6f0d5c2e9a7b1f4d8c";
const VIDZEE_REFERER: &str = "https://core.vidzee.wtf/";

/// Cached secrets extracted from the VidZee JS bundle at runtime.
struct VidzeeSecrets {
    hex_secret: String,
    key_api: String,
}

static SECRETS: OnceCell<VidzeeSecrets> = OnceCell::const_new();

/// Fetch the embed page, locate the embed JS chunk, and extract the hex secret
/// and api-key URL. Result is cached so subsequent calls are free.
async fn fetch_secrets(client: &Client) -> &VidzeeSecrets {
    SECRETS
        .get_or_init(|| async {
            match try_fetch_secrets(client).await {
                Ok(s) => {
                    info!("[VidZee] Self-healed: hex_secret={}, key_api={}", s.hex_secret, s.key_api);
                    s
                }
                Err(e) => {
                    warn!("[VidZee] Self-heal failed (using defaults): {:#}", e);
                    VidzeeSecrets {
                        hex_secret: DEFAULT_HEX_SECRET.to_string(),
                        key_api: DEFAULT_KEY_API.to_string(),
                    }
                }
            }
        })
        .await
}

async fn try_fetch_secrets(client: &Client) -> Result<VidzeeSecrets> {
    // Step 1: Fetch the embed page HTML
    let embed_url = "https://player.vidzee.wtf/embed/movie/550";
    let html = client
        .get(embed_url)
        .header("User-Agent", UA_DESKTOP)
        .header("Referer", "https://core.vidzee.wtf/")
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await?
        .text()
        .await?;

    // Step 2: Extract the embed page JS chunk URL from the HTML
    // Pattern: /_next/static/chunks/app/embed/[[...params]]/page-<hash>.js
    let chunk_re = Regex::new(r#"/_next/static/chunks/app/embed/[^"]+\.js"#)
        .map_err(|e| anyhow!("chunk regex: {}", e))?;
    let chunk_path = chunk_re
        .find(&html)
        .map(|m| m.as_str().to_string())
        .ok_or_else(|| anyhow!("embed chunk URL not found in HTML"))?;

    let chunk_url = format!("https://player.vidzee.wtf{}", chunk_path);

    // Step 3: Fetch the JS chunk
    let js = client
        .get(&chunk_url)
        .header("User-Agent", UA_DESKTOP)
        .header("Referer", embed_url)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await?
        .text()
        .await?;

    // Step 4: Extract hex secret and api-key URL
    // Pattern: let a="<32-hex>",l="https://core.vidzee.wtf/api-key"
    let secret_re = Regex::new(
        r#"let a="([a-f0-9]{32})",l="(https://core\.vidzee\.wtf/api-key)""#
    ).map_err(|e| anyhow!("secret regex: {}", e))?;

    let caps = secret_re
        .captures(&js)
        .ok_or_else(|| anyhow!("hex secret pattern not found in chunk JS"))?;

    Ok(VidzeeSecrets {
        hex_secret: caps[1].to_string(),
        key_api: caps[2].to_string(),
    })
}

pub struct VidzeeClient {
    client: Client,
}

impl VidzeeClient {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    /// Fetch and decrypt the dynamic AES-CBC key from the api-key endpoint.
    async fn get_cbc_key(&self) -> Result<String> {
        let secrets = fetch_secrets(&self.client).await;

        let resp = self.client
            .get(&secrets.key_api)
            .header("Referer", "https://player.vidzee.wtf/")
            .header("User-Agent", UA_DESKTOP)
            .timeout(std::time::Duration::from_secs(8))
            .send()
            .await?;

        let enc_key_b64 = resp.text().await?.trim().to_string();
        let raw = base64::engine::general_purpose::STANDARD
            .decode(&enc_key_b64)
            .map_err(|e| anyhow!("api-key base64: {}", e))?;

        if raw.len() <= 28 {
            return Err(anyhow!("api-key too short: {}", raw.len()));
        }

        let gcm_iv = &raw[0..12];
        let gcm_tag = &raw[12..28];
        let gcm_ct = &raw[28..];

        // GCM key = SHA-256 of the hex secret
        let mut hasher = Sha256::new();
        hasher.update(secrets.hex_secret.as_bytes());
        let gcm_key: [u8; 32] = hasher.finalize().into();

        let cipher = Aes256Gcm::new_from_slice(&gcm_key)
            .map_err(|e| anyhow!("GCM init: {}", e))?;
        let nonce = Nonce::from_slice(gcm_iv);

        // ciphertext || auth_tag
        let mut combined = Vec::with_capacity(gcm_ct.len() + gcm_tag.len());
        combined.extend_from_slice(gcm_ct);
        combined.extend_from_slice(gcm_tag);

        let plaintext = cipher.decrypt(nonce, combined.as_slice())
            .map_err(|_| anyhow!("GCM decrypt failed"))?;

        String::from_utf8(plaintext).map_err(|e| anyhow!("utf8 after GCM: {}", e))
    }

    /// Decrypt a single VidZee link token using AES-256-CBC.
    /// Validates the result is a direct stream URL (m3u8/mp4), not an iframe embed.
    fn decrypt_link(token: &str, key: &str) -> Option<String> {
        let candidate = if token.starts_with("http") {
            token.to_string()
        } else {
            // Outer base64 → ivB64:cipherB64
            let raw_bytes = base64::engine::general_purpose::STANDARD
                .decode(token)
                .ok()?;
            let raw = String::from_utf8(raw_bytes).ok()?;
            let (iv_b64, ct_b64) = raw.split_once(':')?;

            let iv = base64::engine::general_purpose::STANDARD
                .decode(iv_b64.trim())
                .ok()?;
            let ct = base64::engine::general_purpose::STANDARD
                .decode(ct_b64.trim())
                .ok()?;

            // Key: UTF-8 bytes, padded to 32 bytes with nulls
            let mut key_bytes = [0u8; 32];
            let key_utf8 = key.as_bytes();
            let copy_len = key_utf8.len().min(32);
            key_bytes[..copy_len].copy_from_slice(&key_utf8[..copy_len]);

            let mut buf = ct.clone();
            let plaintext = Aes256Cbc::new(key_bytes.as_slice().into(), iv.as_slice().into())
                .decrypt_padded_mut::<Pkcs7>(&mut buf)
                .ok()?;

            String::from_utf8(plaintext.to_vec()).ok()?
        };

        let trimmed = candidate.trim();
        // Reject iframe/embed pages; only accept direct stream URLs
        if trimmed.starts_with("http")
            && !trimmed.contains("/embed/")
            && (trimmed.ends_with(".m3u8")
                || trimmed.ends_with(".mp4")
                || trimmed.contains(".m3u8?")
                || trimmed.contains("playlist")
                || trimmed.contains("master"))
        {
            Some(trimmed.to_string())
        } else {
            None
        }
    }

    pub async fn get_streams(
        &self,
        tmdb_id: u64,
        media_type: &str,
        season: Option<u32>,
        episode: Option<u32>,
    ) -> Result<Vec<StreamResult>> {
        info!("[VidZee] Fetching: TMDB {} ({}) S{:?} E{:?}", tmdb_id, media_type, season, episode);

        // Step 1: Get the dynamic CBC key
        let cbc_key = match self.get_cbc_key().await {
            Ok(k) => {
                info!("[VidZee] Got CBC key: {}", k);
                k
            }
            Err(e) => {
                warn!("[VidZee] Failed to get CBC key: {}", e);
                return Ok(vec![]);
            }
        };

        // Step 2: Query all servers concurrently
        let servers = [0u32, 3, 4, 7, 9, 10]; // Active servers from live testing
        let mut tasks = tokio::task::JoinSet::new();

        for &sr in &servers {
            let client = self.client.clone();
            let key = cbc_key.clone();
            let tmdb = tmdb_id;
            let mt = media_type.to_string();
            let s = season;
            let ep = episode;

            tasks.spawn(async move {
                let mut url = format!("{}?id={}&sr={}", VIDZEE_API, tmdb, sr);
                if mt == "tv" {
                    url.push_str(&format!("&ss={}&ep={}", s.unwrap_or(1), ep.unwrap_or(1)));
                }

                let resp = match client
                    .get(&url)
                    .header("Referer", format!("https://player.vidzee.wtf/embed/movie/{}", tmdb))
                    .header("User-Agent", UA_DESKTOP)
                    .timeout(std::time::Duration::from_secs(10))
                    .send()
                    .await
                {
                    Ok(r) => r,
                    Err(_) => return (sr, Vec::new(), Vec::new()),
                };

                let data: serde_json::Value = match resp.json().await {
                    Ok(d) => d,
                    Err(_) => return (sr, Vec::new(), Vec::new()),
                };

                if data.get("error").is_some() {
                    return (sr, Vec::new(), Vec::new());
                }

                // Extract subtitle tracks
                let mut subtitles = Vec::new();
                if let Some(tracks) = data["tracks"].as_array() {
                    for track in tracks {
                        if let (Some(u), Some(lang)) = (
                            track["url"].as_str(),
                            track["lang"].as_str(),
                        ) {
                            let lang_code = match lang.to_lowercase().as_str() {
                                "english" => "en",
                                "arabic" => "ar",
                                "bengali" => "bn",
                                "chinese" => "zh",
                                "filipino" => "tl",
                                "french" => "fr",
                                "german" => "de",
                                "indonesian" => "id",
                                "malay" => "ms",
                                _ => "en",
                            };
                            subtitles.push(Subtitle {
                                url: u.to_string(),
                                lang: lang_code.to_string(),
                                label: lang.to_string(),
                            });
                        }
                    }
                }

                // Extract and decrypt stream URLs
                let mut streams = Vec::new();
                if let Some(urls) = data["url"].as_array() {
                    for entry in urls {
                        let link = entry["link"].as_str().unwrap_or("");
                        let name = entry["name"].as_str().unwrap_or("VidZee");
                        let lang = entry["lang"].as_str().unwrap_or("Unknown");
                        let stream_type = entry["type"].as_str().unwrap_or("hls");

                        let decoded = match Self::decrypt_link(link, &key) {
                            Some(u) => u,
                            None => continue,
                        };

                        let title = format!("VidZee {} - {} - {}", name, lang, stream_type.to_uppercase());
                        let mut headers = HashMap::new();
                        headers.insert("Referer".to_string(), VIDZEE_REFERER.to_string());
                        headers.insert("User-Agent".to_string(), UA_DESKTOP.to_string());

                        let subs = if subtitles.is_empty() { None } else { Some(subtitles.clone()) };

                        streams.push(StreamResult {
                            url: decoded,
                            title,
                            quality: "Auto".to_string(),
                            provider: "VidZee".to_string(),
                            format: if stream_type == "hls" { "hls".to_string() } else { "mp4".to_string() },
                            subtitles: subs,
                            headers: Some(headers.clone()),
                            extra_metadata: None,
                        });
                    }
                }

                (sr, streams, subtitles)
            });
        }

        let mut all_streams = Vec::new();
        while let Some(result) = tasks.join_next().await {
            if let Ok((_sr, streams, _subs)) = result {
                all_streams.extend(streams);
            }
        }

        info!("[VidZee] Returning {} total streams", all_streams.len());
        Ok(all_streams)
    }
}
