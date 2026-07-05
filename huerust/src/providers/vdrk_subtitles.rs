//! Shared helper for fetching subtitles from sub.vdrk.site.
//! Used by xyra fast-path (which never returns subs) and the main aggregator
//! (for streams like VidZee that don't bundle their own).

use crate::models::Subtitle;
use reqwest_middleware::ClientWithMiddleware;
use serde::Deserialize;
use tracing::{info, warn};

const SUB_BASE: &str = "https://sub.vdrk.site";
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/137.0.0.0 Safari/537.36";

#[derive(Debug, Deserialize)]
struct VdrkTrack {
    label: String,
    file: String,
}

pub async fn fetch_vdrk_subtitles(
    client: &ClientWithMiddleware,
    tmdb_id: u64,
    media_type: &str,
    season: u32,
    episode: u32,
) -> Option<Vec<Subtitle>> {
    let url = if media_type == "movie" {
        format!("{SUB_BASE}/v2/movie/{tmdb_id}")
    } else {
        format!("{SUB_BASE}/v2/tv/{tmdb_id}/{season}/{episode}")
    };

    let resp = match client.get(&url).header("User-Agent", USER_AGENT).send().await {
        Ok(r) if r.status().is_success() => r,
        Ok(r) => {
            warn!("[vdrk] subs API returned {}", r.status());
            return None;
        }
        Err(e) => {
            warn!("[vdrk] subs request failed: {}", e);
            return None;
        }
    };

    match resp.json::<Vec<VdrkTrack>>().await {
        Ok(tracks) if !tracks.is_empty() => {
            let subs: Vec<Subtitle> = tracks
                .into_iter()
                .map(|t| {
                    let lang = t
                        .label
                        .to_lowercase()
                        .split_whitespace()
                        .next()
                        .unwrap_or("en")
                        .to_string();
                    Subtitle { url: t.file, lang, label: t.label }
                })
                .collect();
            info!("[vdrk] {} tracks for TMDB {} ({} S{}E{})", subs.len(), tmdb_id, media_type, season, episode);
            Some(subs)
        }
        Ok(_) => None,
        Err(e) => {
            warn!("[vdrk] subs JSON parse failed: {}", e);
            None
        }
    }
}
