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
use crate::providers::dramacool::DramacoolClient;
use anyhow::Result;
use std::sync::Arc;

pub struct DramacoolProvider {
    client: Arc<DramacoolClient>,
}

impl DramacoolProvider {
    pub fn new(client: Arc<DramacoolClient>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl StreamProvider for DramacoolProvider {
    fn name(&self) -> &'static str { "Orient" }
    fn real_name(&self) -> &'static str { "dramacool" }
    fn is_fast(&self) -> bool { false }
    fn timeout_secs(&self) -> u64 { 20 }

    async fn fetch(
        &self,
        tmdb_info: &TmdbNormalizedInfo,
        _media_type: &str,
        season: u32,
        episode: u32,
    ) -> Result<Vec<StreamResult>> {
        self.client.get_streams_normalized(&tmdb_info.title, season, episode).await
    }
}
