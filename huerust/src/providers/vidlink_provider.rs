/*
 * HUE ENGINE ❄
 * System Designation: The Architecture of Aggregation
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */

use crate::clients::tmdb::TmdbNormalizedInfo;
use crate::clients::vidlink::VidlinkClient;
use crate::models::StreamResult;
use crate::providers::StreamProvider;
use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;

pub struct VidlinkProvider {
    client: Arc<VidlinkClient>,
}

impl VidlinkProvider {
    pub fn new(client: Arc<VidlinkClient>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl StreamProvider for VidlinkProvider {
    fn name(&self) -> &'static str {
        "Vidlink"
    }
    fn real_name(&self) -> &'static str {
        "vidlink"
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
            .get_streams(tmdb_info.id, media_type, season, episode)
            .await
    }
}
