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
use crate::providers::kissasian::KissAsianClient;
use anyhow::Result;
use std::sync::Arc;

pub struct KissAsianProvider {
    client: Arc<KissAsianClient>,
}

impl KissAsianProvider {
    pub fn new(client: Arc<KissAsianClient>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl StreamProvider for KissAsianProvider {
    fn name(&self) -> &'static str { "Pearl" }
    fn real_name(&self) -> &'static str { "kissasian" }
    fn is_fast(&self) -> bool { false }
    fn timeout_secs(&self) -> u64 { 25 }

    async fn fetch(
        &self,
        tmdb_info: &TmdbNormalizedInfo,
        media_type: &str,
        season: u32,
        episode: u32,
    ) -> Result<Vec<StreamResult>> {
        if media_type == "movie" {
            tracing::info!("[Pearl Provider] Skipping movie request");
            return Ok(vec![]);
        }

        self.client
            .get_streams_normalized(&tmdb_info.title, season, episode)
            .await
    }
}
