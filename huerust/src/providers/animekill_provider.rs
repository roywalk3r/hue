//! Hue Stream API
//! Professional Streaming Aggregator
//!
//! @author roywalk3r <roywalker440@gmail.com>
//! @repository https://github.com/roywalk3r/hue.git
//! @license MIT

use crate::clients::animekill::AnimeKillClient;
use crate::clients::tmdb::TmdbNormalizedInfo;
use crate::models::StreamResult;
use crate::models::anime::StreamPayload;
use crate::providers::StreamProvider;
use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{info, warn};

/// Search animekill for a title and resolve an episode stream. Bypasses TMDB —
/// animekill owns its own catalog, so the search query is the title the
/// caller (anime aggregator UI) already resolved.
pub struct AnimeKillSearch {
    client: Arc<AnimeKillClient>,
}

impl AnimeKillSearch {
    pub fn new(client: Arc<AnimeKillClient>) -> Self {
        Self { client }
    }

    pub async fn search(
        &self,
        title: &str,
        episode: u32,
    ) -> Result<crate::models::anime::StreamPayload> {
        info!("[animekill] Searching donghua: \"{}\" E{}", title, episode);
        let search = self.client.search(title).await?;
        let card = crate::providers::best_donghua_match(&search.results, title)
            .ok_or_else(|| anyhow::anyhow!("no donghua match for \"{}\"", title))?;
        let episodes = self.client.get_episodes(&card.id).await?;
        let ep = episodes
            .iter()
            .find(|e| e.number == episode)
            .ok_or_else(|| {
                anyhow::anyhow!("episode {} not found for \"{}\"", episode, card.title)
            })?;
        self.client.get_stream(&ep.id, None).await
    }
}

/// Standard-aggregator `StreamProvider` wrapper. Fans out into the
/// animekill client from the `/api/streams/tv/:tmdbId` path so the
/// SSE player race picks up donghua streams without a separate route.
/// Public name: "Phoenix". Pre-checks `looks_chinese` to short-circuit
/// non-Chinese requests before any network I/O.
pub struct AnimeKillProvider {
    client: Arc<AnimeKillClient>,
}

impl AnimeKillProvider {
    pub fn new(client: Arc<AnimeKillClient>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl StreamProvider for AnimeKillProvider {
    fn name(&self) -> &'static str {
        "Phoenix"
    }
    fn real_name(&self) -> &'static str {
        "animekill"
    }
    fn is_fast(&self) -> bool {
        false
    }
    fn timeout_secs(&self) -> u64 {
        12
    }

    async fn fetch(
        &self,
        tmdb_info: &TmdbNormalizedInfo,
        media_type: &str,
        _season: u32,
        episode: u32,
    ) -> Result<Vec<StreamResult>> {
        if media_type == "movie" {
            return Ok(vec![]);
        }
        if !super::looks_chinese(tmdb_info) {
            return Ok(vec![]);
        }

        let search = self.client.search(&tmdb_info.title).await?;
        let Some(card) = crate::providers::best_donghua_match(&search.results, &tmdb_info.title)
        else {
            return Ok(vec![]);
        };

        let episodes = self.client.get_episodes(&card.id).await?;
        let Some(ep) = episodes.iter().find(|e| e.number == episode) else {
            return Ok(vec![]);
        };

        match self.client.get_stream(&ep.id, None).await {
            Ok(payload) => Ok(vec![payload_to_stream_result(payload, "Phoenix", &tmdb_info.title)]),
            Err(e) => {
                warn!("[Phoenix] get_stream failed: {}", e);
                Ok(vec![])
            }
        }
    }
}

fn payload_to_stream_result(payload: StreamPayload, provider: &str, title: &str) -> StreamResult {
    let server = payload.server.unwrap_or_default();
    let referer = payload.referer.unwrap_or_default();
    let mut headers = HashMap::new();
    if !referer.is_empty() {
        headers.insert("Referer".to_string(), referer);
    }
    StreamResult {
        url: payload.url,
        title: format!("{} — {}", title, server),
        quality: "Auto".to_string(),
        provider: provider.to_string(),
        format: "iframe".to_string(),
        subtitles: None,
        headers: Some(headers),
        extra_metadata: None,
    }
}
