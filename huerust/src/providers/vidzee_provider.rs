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
use crate::providers::vidzee::VidzeeClient;
use crate::models::StreamResult;
use crate::clients::tmdb::TmdbNormalizedInfo;
use anyhow::Result;
use std::sync::Arc;

pub struct VidzeeProvider {
    vidzee: Arc<VidzeeClient>,
}

impl VidzeeProvider {
    pub fn new(vidzee: Arc<VidzeeClient>) -> Self {
        Self { vidzee }
    }
}

#[async_trait]
impl StreamProvider for VidzeeProvider {
    fn name(&self) -> &'static str { "VidZee" }
    fn real_name(&self) -> &'static str { "vidzee" }
    fn is_fast(&self) -> bool { true }
    fn timeout_secs(&self) -> u64 { 15 }

    async fn fetch(
        &self,
        tmdb_info: &TmdbNormalizedInfo,
        media_type: &str,
        season: u32,
        episode: u32,
    ) -> Result<Vec<StreamResult>> {
        self.vidzee.get_streams(tmdb_info.id, media_type, Some(season), Some(episode)).await
    }
}
