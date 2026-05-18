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
use crate::providers::kisskh::KissKhClient;
use anyhow::Result;
use std::sync::Arc;

pub struct KissKhProvider {
    client: Arc<KissKhClient>,
}

impl KissKhProvider {
    pub fn new(client: Arc<KissKhClient>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl StreamProvider for KissKhProvider {
    fn name(&self) -> &'static str { "Zenith" }
    fn real_name(&self) -> &'static str { "kisskh" }
    fn is_fast(&self) -> bool { false }
    fn timeout_secs(&self) -> u64 { 60 }

    async fn fetch(
        &self,
        tmdb_info: &TmdbNormalizedInfo,
        media_type: &str,
        season: u32,
        episode: u32,
    ) -> Result<Vec<StreamResult>> {
        // KissKH is a drama-only provider; skip for movie/tv to avoid wrong matches
        if media_type == "movie" || media_type == "tv" {
            return Ok(vec![]);
        }
        self.client.get_streams_normalized(&tmdb_info.title, season, episode).await
    }
}
