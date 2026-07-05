//! XyraVixsrcProvider — Aura (Vixsrc) via the xyra.stream streamhub proxy.

use crate::clients::tmdb::TmdbNormalizedInfo;
use crate::models::StreamResult;
use crate::providers::xyra_vixsrc::XyraVixsrcClient;
use crate::providers::StreamProvider;
use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;

pub struct XyraVixsrcProvider {
    client: Arc<XyraVixsrcClient>,
}

impl XyraVixsrcProvider {
    pub fn new(client: Arc<XyraVixsrcClient>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl StreamProvider for XyraVixsrcProvider {
    fn name(&self) -> &'static str {
        "Aura"
    }
    fn real_name(&self) -> &'static str {
        "vixsrc"
    }
    fn is_fast(&self) -> bool {
        true
    }
    fn timeout_secs(&self) -> u64 {
        8
    }

    async fn fetch(
        &self,
        tmdb_info: &TmdbNormalizedInfo,
        media_type: &str,
        season: u32,
        episode: u32,
    ) -> Result<Vec<StreamResult>> {
        self.client
            .get_streams_normalized(tmdb_info.id, media_type, season, episode)
            .await
    }
}
