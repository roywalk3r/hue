//! StreamHub StreamProvider — wraps the StreamHub aggregator client.

use crate::clients::tmdb::TmdbNormalizedInfo;
use crate::models::StreamResult;
use crate::providers::streamhub::{streamhub_to_result, StreamHubClient};
use crate::providers::StreamProvider;
use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;

pub struct StreamHubProvider {
    client: Arc<StreamHubClient>,
}

impl StreamHubProvider {
    pub fn new(client: Arc<StreamHubClient>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl StreamProvider for StreamHubProvider {
    fn name(&self) -> &'static str {
        "StreamHub"
    }
    fn real_name(&self) -> &'static str {
        "streamhub"
    }
    fn is_fast(&self) -> bool {
        true
    }
    fn timeout_secs(&self) -> u64 {
        4
    }

    async fn fetch(
        &self,
        tmdb_info: &TmdbNormalizedInfo,
        media_type: &str,
        season: u32,
        episode: u32,
    ) -> Result<Vec<StreamResult>> {
        let tmdb_id = tmdb_info.id;
        let streams = self
            .client
            .get_streams(tmdb_id, media_type, season, episode)
            .await?;
        Ok(streams.iter().map(streamhub_to_result).collect())
    }
}
