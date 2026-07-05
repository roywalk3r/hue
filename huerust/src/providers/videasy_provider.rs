/*
 * HUE ENGINE ❄
 * System Designation: The Architecture of Aggregation
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */

#![allow(dead_code)]

use crate::clients::tmdb::TmdbNormalizedInfo;
use crate::models::StreamResult;
use crate::providers::videasy::VideasyClient;
use crate::providers::StreamProvider;
use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;

pub struct VideasyProvider {
    client: Arc<VideasyClient>,
}

impl VideasyProvider {
    pub fn new(client: Arc<VideasyClient>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl StreamProvider for VideasyProvider {
    fn name(&self) -> &'static str {
        "Vortex"
    }
    fn real_name(&self) -> &'static str {
        "videasy"
    }
    fn is_fast(&self) -> bool {
        false
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
            .get_streams_normalized(
                &tmdb_info.title,
                media_type,
                tmdb_info.year,
                tmdb_info.id,
                tmdb_info.imdb_id.as_deref(),
                season,
                episode,
            )
            .await
    }
}
