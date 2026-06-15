/*
 * HUE ENGINE ❄
 * System Designation: The Architecture of Aggregation
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */

use crate::clients::lordflix::LordflixClient;
use crate::clients::tmdb::TmdbNormalizedInfo;
use crate::models::StreamResult;
use crate::providers::StreamProvider;
use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;

pub struct LordflixProvider {
    client: Arc<LordflixClient>,
}

impl LordflixProvider {
    pub fn new(client: Arc<LordflixClient>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl StreamProvider for LordflixProvider {
    fn name(&self) -> &'static str {
        "Lordflix"
    }
    fn real_name(&self) -> &'static str {
        "lordflix"
    }
    fn is_fast(&self) -> bool {
        true
    }
    fn timeout_secs(&self) -> u64 {
        15
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
