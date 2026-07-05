//! KissKH (xyra) StreamProvider — wraps the xyra.stream KissKH client.

use crate::clients::tmdb::TmdbNormalizedInfo;
use crate::models::StreamResult;
use crate::providers::kisskh_xyra::KissKhXyraClient;
use crate::providers::StreamProvider;
use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;

pub struct KissKhXyraProvider {
    client: Arc<KissKhXyraClient>,
}

impl KissKhXyraProvider {
    pub fn new(client: Arc<KissKhXyraClient>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl StreamProvider for KissKhXyraProvider {
    fn name(&self) -> &'static str {
        "Aurora"
    }
    fn real_name(&self) -> &'static str {
        "kisskh_xyra"
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
        // KissKH (xyra) is a drama-only provider; skip for movie/tv to
        // avoid wrong matches. The non-xyra kisskh client (Zenith) handles
        // tv already; this one is drama-only.
        if media_type == "movie" {
            return Ok(vec![]);
        }
        let title = tmdb_info.title.clone();
        if title.is_empty() {
            return Ok(vec![]);
        }
        self.client
            .get_streams_normalized(&title, season, episode)
            .await
    }
}
