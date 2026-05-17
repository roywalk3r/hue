/*
 * HUE ENGINE ❄
 * System Designation: The Architecture of Aggregation
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */

use async_trait::async_trait;
use crate::providers::StreamProvider;
use crate::providers::ppvto::PpvToClient;
use crate::models::StreamResult;
use crate::clients::tmdb::TmdbNormalizedInfo;
use anyhow::Result;
use std::sync::Arc;

pub struct PpvToProvider {
    client: Arc<PpvToClient>,
}

impl PpvToProvider {
    pub fn new(client: Arc<PpvToClient>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl StreamProvider for PpvToProvider {
    fn name(&self) -> &'static str { "PpVTo" }
    fn real_name(&self) -> &'static str { "ppvto" }
    fn is_fast(&self) -> bool { false }
    fn timeout_secs(&self) -> u64 { 15 }

    async fn fetch(
        &self,
        tmdb_info: &TmdbNormalizedInfo,
        _media_type: &str,
        _season: u32,
        _episode: u32,
    ) -> Result<Vec<StreamResult>> {
        self.client.get_streams(&tmdb_info.title).await
    }
}
