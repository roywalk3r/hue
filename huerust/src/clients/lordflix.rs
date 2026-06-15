/*
 * HUE ENGINE ❄
 * System Designation: The Architecture of Aggregation
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */

//! Lordflix provider — multi-server HLS aggregator.
//!
//! Per server (10 servers queried in parallel):
//!   1. GET enc-dec.app/api/enc-lordflix?url=<server-url>  →  {url, sign}
//!   2. GET <url> with browser headers  →  encrypted body
//!   3. POST enc-dec.app/api/dec-lordflix  {text, sign}  →  decrypted JSON
//!   4. Use the first HLS stream's `playlist` URL.
//!
//! TMDB lookup is done by the aggregator before calling `fetch`, so the
//! `TmdbNormalizedInfo` passed in already has title, year, imdb_id.

use crate::clients::tmdb::TmdbNormalizedInfo;
use crate::constants::UA_DESKTOP;
use crate::models::{StreamResult, Subtitle};
use anyhow::Result;
use reqwest_middleware::ClientWithMiddleware;
use serde::Deserialize;
use tokio::task::JoinSet;
use tracing::{info, warn};

const LORDFLIX_API: &str = "https://snowhouse.lordflix.club";
const MULTI_DECRYPT_API: &str = "https://enc-dec.app/api";
const LORDFLIX_REFERER: &str = "https://lordflix.org";

const SERVERS: &[&str] = &[
    "Berlin",
    "Marseille",
    "Backrooms",
    "Phoenix",
    "Oslo",
    "Luna",
    "Sakura",
    "Rio",
    "Ativa",
    "Moscow",
];

fn browser_headers() -> Vec<(&'static str, String)> {
    vec![
        ("Accept", "*/*".to_string()),
        ("Origin", LORDFLIX_REFERER.to_string()),
        ("Referer", format!("{}/", LORDFLIX_REFERER)),
        ("User-Agent", UA_DESKTOP.to_string()),
    ]
}

#[derive(Deserialize)]
struct EncBridgeResult {
    url: String,
    sign: String,
}

#[derive(Deserialize)]
struct EncBridgeResponse {
    status: u16,
    result: Option<EncBridgeResult>,
}

#[derive(Deserialize)]
struct DecStreamItem {
    #[serde(rename = "type")]
    stream_type: Option<String>,
    playlist: Option<String>,
}

#[derive(Deserialize)]
struct DecResult {
    stream: Option<Vec<DecStreamItem>>,
    error: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct DecResponse {
    status: u16,
    result: Option<DecResult>,
}

#[derive(Clone)]
pub struct LordflixClient {
    client: ClientWithMiddleware,
}

impl LordflixClient {
    pub fn new(client: ClientWithMiddleware) -> Self {
        Self { client }
    }

    /// Fetch streams for a movie or TV episode.
    pub async fn get_streams(
        &self,
        tmdb_info: &TmdbNormalizedInfo,
        media_type: &str,
        season: u32,
        episode: u32,
    ) -> Result<Vec<StreamResult>> {
        info!(
            "[Lordflix] Fetching: TMDB {} ({}) S{} E{}",
            tmdb_info.id, media_type, season, episode
        );

        let title = tmdb_info.title.clone();
        let year = tmdb_info.year.map(|y| y.to_string()).unwrap_or_default();
        let imdb_id = tmdb_info.imdb_id.clone().unwrap_or_default();
        if title.is_empty() || imdb_id.is_empty() {
            warn!(
                "[Lordflix] Missing title or imdbId for TMDB {} (title='{}' imdb_id={:?})",
                tmdb_info.id, title, tmdb_info.imdb_id
            );
            return Ok(vec![]);
        }
        let type_param = if media_type == "tv" {
            "series"
        } else {
            "movie"
        };
        let title_enc = url_encode_quote(&title);
        let tmdb_id_s = tmdb_info.id.to_string();

        let mut tasks: JoinSet<Option<StreamResult>> = JoinSet::new();
        for server in SERVERS {
            let client = self.client.clone();
            let imdb_s = imdb_id.clone();
            let year_s = year.clone();
            let title_s = title_enc.clone();
            let type_p = type_param.to_string();
            let server_s = (*server).to_string();
            let media = media_type.to_string();
            let tmdb_id_local = tmdb_id_s.clone();
            let season_s = season;
            let episode_s = episode;
            tasks.spawn(async move {
                let mut server_url = format!(
                    "{LORDFLIX_API}/?title={title_s}&type={type_p}&year={year_s}&imdb={imdb_s}&tmdb={tmdb_id_local}&server={server_s}"
                );
                if media == "tv" {
                    server_url.push_str(&format!("&season={season_s}&episode={episode_s}"));
                }
                let server_url_enc = url_encode_quote(&server_url);

                // Step 1: enc-dec.app encrypt
                let enc_url = format!("{MULTI_DECRYPT_API}/enc-lordflix?url={server_url_enc}");
                let enc_body: EncBridgeResponse = match client.get(&enc_url).send().await {
                    Ok(r) => match r.json().await {
                        Ok(b) => b,
                        Err(e) => {
                            warn!("[Lordflix] enc parse fail {server_s}: {e}");
                            return None;
                        }
                    },
                    Err(e) => {
                        warn!("[Lordflix] enc fail {server_s}: {e}");
                        return None;
                    }
                };
                if enc_body.status != 200 {
                    return None;
                }
                let (proxy_url, sign) = match enc_body.result {
                    Some(r) => (r.url, r.sign),
                    None => return None,
                };

                // Step 2: fetch encrypted body with browser headers
                let mut req = client.get(&proxy_url);
                for (k, v) in browser_headers() {
                    req = req.header(k, v);
                }
                let remote_text = match req.send().await {
                    Ok(r) => match r.text().await {
                        Ok(t) => t,
                        Err(e) => {
                            warn!("[Lordflix] remote text fail {server_s}: {e}");
                            return None;
                        }
                    },
                    Err(e) => {
                        warn!("[Lordflix] remote fetch fail {server_s}: {e}");
                        return None;
                    }
                };

                // Step 3: dec-lordflix
                let dec_url = format!("{MULTI_DECRYPT_API}/dec-lordflix");
                let dec_body: DecResponse = match client
                    .post(&dec_url)
                    .json(&serde_json::json!({ "text": remote_text, "sign": sign }))
                    .send()
                    .await
                {
                    Ok(r) => match r.json().await {
                        Ok(b) => b,
                        Err(e) => {
                            warn!("[Lordflix] dec parse fail {server_s}: {e}");
                            return None;
                        }
                    },
                    Err(e) => {
                        warn!("[Lordflix] dec fail {server_s}: {e}");
                        return None;
                    }
                };
                if dec_body.status != 200 {
                    return None;
                }
                let result = match dec_body.result {
                    Some(r) => r,
                    None => return None,
                };
                if result.error.is_some() {
                    return None;
                }
                let stream_list = match result.stream {
                    Some(l) => l,
                    None => return None,
                };
                if let Some(top) = stream_list.first() {
                    if top.stream_type.as_deref() == Some("hls") {
                        if let Some(playlist) = top.playlist.clone() {
                            let mut headers = std::collections::HashMap::new();
                            for (k, v) in browser_headers() {
                                headers.insert(k.to_string(), v);
                            }
                            info!("[Lordflix] Server {} ok", server_s);
                            return Some(StreamResult {
                                url: playlist,
                                title: format!("Lordflix[{}]", server_s),
                                quality: "Auto".to_string(),
                                provider: "Lordflix".to_string(),
                                format: "hls".to_string(),
                                subtitles: None::<Vec<Subtitle>>,
                                headers: Some(headers),
                                extra_metadata: None,
                            });
                        }
                    }
                }
                None
            });
        }
        let mut out = Vec::new();
        while let Some(res) = tasks.join_next().await {
            if let Ok(Some(s)) = res {
                out.push(s);
            }
        }
        info!("[Lordflix] Total streams: {}", out.len());
        Ok(out)
    }
}

/// URL-encode that mirrors Node's `encodeURIComponent` with `+` → `%20`
/// (the so-called "encodeQuote" pattern in TMDB-Embed-API).
fn url_encode_quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*b as char);
            }
            b' ' => out.push_str("%20"),
            b'+' => out.push_str("%20"),
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}
