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
use crate::clients::vidsrcembed::VidsrcEmbedClient;
use crate::models::StreamResult;
use crate::clients::tmdb::TmdbNormalizedInfo;
use anyhow::Result;
use std::sync::Arc;

pub struct VidsrcEmbedProvider {
    client: Arc<VidsrcEmbedClient>,
}

impl VidsrcEmbedProvider {
    pub fn new(client: Arc<VidsrcEmbedClient>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl StreamProvider for VidsrcEmbedProvider {
    fn name(&self) -> &'static str { "Nebula" }
    fn real_name(&self) -> &'static str { "vidsrc-embed" }
    fn is_fast(&self) -> bool { false }
    fn timeout_secs(&self) -> u64 { 60 }

    async fn fetch(
        &self,
        tmdb_info: &TmdbNormalizedInfo,
        media_type: &str,
        season: u32,
        episode: u32,
    ) -> Result<Vec<StreamResult>> {
        self.client.get_streams(
            &tmdb_info.id.to_string(),
            media_type,
            if media_type == "tv" || media_type == "series" { Some(season) } else { None },
            if media_type == "tv" || media_type == "series" { Some(episode) } else { None },
        ).await
    }
}
