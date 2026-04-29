//! Hue Stream API
//! Professional Streaming Aggregator
//!
//! @author roywalk3r <roywalker440@gmail.com>
//! @repository https://github.com/roywalk3r/hue.git
//! @license MIT

use async_trait::async_trait;
use crate::providers::StreamProvider;
use crate::models::StreamResult;
use crate::tmdb::TmdbNormalizedInfo;
use crate::providers::sflix::SflixClient;
use anyhow::Result;
use std::sync::Arc;

pub struct SflixProvider {
    client: Arc<SflixClient>,
}

impl SflixProvider {
    pub fn new(client: Arc<SflixClient>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl StreamProvider for SflixProvider {
    fn name(&self) -> &'static str { "Nova" }
    fn real_name(&self) -> &'static str { "sflix" }
    fn is_fast(&self) -> bool { true }
    fn timeout_secs(&self) -> u64 { 30 }

    async fn fetch(
        &self,
        tmdb_info: &TmdbNormalizedInfo,
        media_type: &str,
        season: u32,
        episode: u32,
    ) -> Result<Vec<StreamResult>> {
        self.client.get_streams_normalized(&tmdb_info.title, media_type, tmdb_info.year, season, episode).await
    }
}
