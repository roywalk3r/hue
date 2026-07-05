//! Hue Stream API
//! Professional Streaming Aggregator
//!
//! @author roywalk3r <roywalker440@gmail.com>
//! @repository https://github.com/roywalk3r/hue.git
//! @license MIT

use crate::clients::moviebox::MovieBoxClient;
use crate::clients::tmdb::TmdbNormalizedInfo;
use crate::models::StreamResult;
use crate::providers::StreamProvider;
use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;

pub struct MovieBoxProvider {
    client: Arc<MovieBoxClient>,
}

impl MovieBoxProvider {
    pub fn new(client: Arc<MovieBoxClient>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl StreamProvider for MovieBoxProvider {
    fn name(&self) -> &'static str {
        "Cinema"
    }
    fn real_name(&self) -> &'static str {
        "moviebox"
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
            .get_streams_normalized(
                &tmdb_info.title,
                media_type,
                tmdb_info.year,
                season,
                episode,
            )
            .await
    }
}
