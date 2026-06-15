/*
 * HUE ENGINE ❄
 * System Designation: The Architecture of Aggregation
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */

use crate::clients::notorrent::NoTorrentClient;
use crate::clients::tmdb::TmdbNormalizedInfo;
use crate::models::StreamResult;
use crate::providers::StreamProvider;
use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;

pub struct NoTorrentProvider {
    client: Arc<NoTorrentClient>,
}

impl NoTorrentProvider {
    pub fn new(client: Arc<NoTorrentClient>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl StreamProvider for NoTorrentProvider {
    fn name(&self) -> &'static str {
        "NoTorrent"
    }
    fn real_name(&self) -> &'static str {
        "notorrent"
    }
    fn is_fast(&self) -> bool {
        true
    }
    fn timeout_secs(&self) -> u64 {
        20
    }

    async fn fetch(
        &self,
        tmdb_info: &TmdbNormalizedInfo,
        media_type: &str,
        season: u32,
        episode: u32,
    ) -> Result<Vec<StreamResult>> {
        self.client
            .get_streams(tmdb_info, media_type, season, episode)
            .await
    }
}
