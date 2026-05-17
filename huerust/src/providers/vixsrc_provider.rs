//! Hue Stream API
//! Professional Streaming Aggregator
//!
//! @author roywalk3r <roywalker440@gmail.com>
//! @repository https://github.com/roywalk3r/hue.git
//! @license MIT

use async_trait::async_trait;
use crate::providers::StreamProvider;
use crate::providers::vixsrc::VixsrcClient;
use crate::models::StreamResult;
use crate::clients::tmdb::TmdbNormalizedInfo;
use anyhow::Result;
use std::sync::Arc;

pub struct VixsrcProvider {
    vixsrc: Arc<VixsrcClient>,
}

impl VixsrcProvider {
    pub fn new(vixsrc: Arc<VixsrcClient>) -> Self {
        Self { vixsrc }
    }
}

#[async_trait]
impl StreamProvider for VixsrcProvider {
    fn name(&self) -> &'static str { "Aura" }
    fn real_name(&self) -> &'static str { "vixsrc" }
    fn is_fast(&self) -> bool { true }
    fn timeout_secs(&self) -> u64 { 15 }

    async fn fetch(
        &self,
        tmdb_info: &TmdbNormalizedInfo,
        media_type: &str,
        season: u32,
        episode: u32,
    ) -> Result<Vec<StreamResult>> {
        self.vixsrc.get_streams_normalized(tmdb_info.id, media_type, season, episode).await
    }
}
