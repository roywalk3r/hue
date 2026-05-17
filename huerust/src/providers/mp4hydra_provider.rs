//! Hue Stream API
//! Professional Streaming Aggregator
//!
//! @author roywalk3r <roywalker440@gmail.com>
//! @repository https://github.com/roywalk3r/hue.git
//! @license MIT

use async_trait::async_trait;
use crate::providers::StreamProvider;
use crate::models::StreamResult;
use crate::clients::tmdb::TmdbNormalizedInfo;
use anyhow::Result;
use tracing::info;

pub struct Mp4HydraProvider {
    client: crate::providers::mp4hydra::Mp4HydraClient,
}

impl Mp4HydraProvider {
    pub fn new() -> Self {
        Self {
            client: crate::providers::mp4hydra::Mp4HydraClient::new(),
        }
    }
}

#[async_trait]
impl StreamProvider for Mp4HydraProvider {
    fn name(&self) -> &'static str { "Hydra" }
    fn real_name(&self) -> &'static str { "mp4hydra" }
    fn is_fast(&self) -> bool { true }
    fn timeout_secs(&self) -> u64 { 10 }

    async fn fetch(
        &self,
        tmdb_info: &TmdbNormalizedInfo,
        media_type: &str,
        season: u32,
        episode: u32,
    ) -> Result<Vec<StreamResult>> {
        info!("[Hydra Provider] Fetching for \"{}\" ({}) S{} E{}", tmdb_info.title, media_type, season, episode);
        self.client.get_streams_normalized(
            &tmdb_info.title,
            None,
            tmdb_info.year,
            media_type,
            season,
            episode,
        ).await
    }
}
