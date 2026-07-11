//! Hue Stream API
//! Professional Streaming Aggregator
//!
//! @author roywalk3r <roywalker440@gmail.com>
//! @repository https://github.com/roywalk3r/hue.git
//! @license MIT

use crate::clients::animekill::AnimeKillClient;
use anyhow::Result;
use std::sync::Arc;
use tracing::info;

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
