/*
 * HUE ENGINE ❄
 * System Designation: The Architecture of Aggregation
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */

//! Vidlink provider — encrypted TMDB id → HLS playlist URL.
//!
//! API flow:
//!   1. GET https://enc-dec.app/api/enc-vidlink?text={tmdbId}  →  encrypted id
//!   2. GET https://vidlink.pro/api/b/{tv|movie}/{encoded}[/{s}/{e}]?multiLang=0
//!      → `{ stream: { playlist: "https://...m3u8" } }`
//!
//! The playlist URL is an HLS master served behind `Referer: https://vidlink.pro`
//! and a signed `auth` query param. The Rust player is expected to set those
//! headers when fetching; the proxy layer forwards them as-is.

use crate::constants::UA_DESKTOP;
use crate::models::{StreamResult, Subtitle};
use anyhow::{anyhow, Result};
use reqwest_middleware::ClientWithMiddleware;
use serde::Deserialize;
use tracing::{info, warn};
const ENC_DEC_URL: &str = "https://enc-dec.app/api/enc-vidlink";
const VIDLINK_API: &str = "https://vidlink.pro/api/b";
const VIDLINK_REFERER: &str = "https://vidlink.pro";

#[derive(Clone)]
pub struct VidlinkClient {
    client: ClientWithMiddleware,
}

#[derive(Debug, Deserialize)]
struct EncDecResponse {
    status: u16,
    result: Option<String>,
}

#[derive(Debug, Deserialize)]
struct VidlinkStreamInner {
    playlist: Option<String>,
}

#[derive(Debug, Deserialize)]
struct VidlinkStreamWrapper {
    stream: Option<VidlinkStreamInner>,
}

impl VidlinkClient {
    pub fn new(client: ClientWithMiddleware) -> Self {
        Self { client }
    }

    /// Fetch streams for a movie or TV episode.
    pub async fn get_streams(
        &self,
        tmdb_id: u64,
        media_type: &str,
        season: u32,
        episode: u32,
    ) -> Result<Vec<StreamResult>> {
        info!(
            "[Vidlink] Fetching: TMDB {} ({}) S{} E{}",
            tmdb_id, media_type, season, episode
        );

        // Step 1: encrypt the TMDB id.
        let encoded = match self.encrypt_id(tmdb_id).await? {
            Some(e) => e,
            None => {
                warn!("[Vidlink] enc-dec.app returned no result");
                return Ok(vec![]);
            }
        };

        // Step 2: resolve playlist URL.
        let url = if media_type == "movie" {
            format!("{}/movie/{}?multiLang=0", VIDLINK_API, encoded)
        } else {
            format!(
                "{}/tv/{}/{}/{}?multiLang=0",
                VIDLINK_API, encoded, season, episode
            )
        };

        let resp = self
            .client
            .get(&url)
            .header("User-Agent", UA_DESKTOP)
            .header("Referer", VIDLINK_REFERER)
            .send()
            .await?;

        if !resp.status().is_success() {
            warn!("[Vidlink] API returned {}", resp.status());
            return Ok(vec![]);
        }

        let data: VidlinkStreamWrapper = resp.json().await?;
        let playlist = data
            .stream
            .and_then(|s| s.playlist)
            .filter(|u| !u.is_empty());

        let playlist = match playlist {
            Some(u) => u,
            None => {
                warn!("[Vidlink] No playlist in response");
                return Ok(vec![]);
            }
        };

        info!("[Vidlink] Got playlist URL");

        let mut headers = std::collections::HashMap::new();
        headers.insert("Referer".to_string(), VIDLINK_REFERER.to_string());

        let streams = vec![StreamResult {
            url: playlist,
            title: "Vidlink".to_string(),
            quality: "Auto".to_string(),
            provider: "Vidlink".to_string(),
            format: "hls".to_string(),
            subtitles: None::<Vec<Subtitle>>,
            headers: Some(headers),
            extra_metadata: None,
        }];
        Ok(streams)
    }

    /// Hit enc-dec.app to encrypt the TMDB id. Returns None on upstream failure.
    async fn encrypt_id(&self, tmdb_id: u64) -> Result<Option<String>> {
        let url = format!("{}?text={}", ENC_DEC_URL, tmdb_id);
        let resp = self.client.get(&url).send().await?;
        if !resp.status().is_success() {
            return Err(anyhow!("enc-dec.app returned {}", resp.status()));
        }
        let data: EncDecResponse = resp.json().await?;
        if data.status != 200 {
            return Ok(None);
        }
        Ok(data.result)
    }
}
