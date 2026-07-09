//! XyraAurexProvider — xyra.stream web-app "aurex-alpha" source.
//!
//! Separate from `XyraVixsrcProvider` because:
//!   * Different endpoint (xyra.stream web app vs api.xyra.stream streamhub)
//!   * Different format (mp4 only, not HLS)
//!   * Includes real subtitles (the v1/streamhub endpoint does not)
//!   * Browser-style headers required (Referer, Origin, sec-fetch)

use crate::clients::tmdb::TmdbNormalizedInfo;
use crate::models::StreamResult;
use crate::providers::xyra_aurex::XyraAurexClient;
use crate::providers::StreamProvider;
use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;

pub struct XyraAurexProvider {
    client: Arc<XyraAurexClient>,
}

impl XyraAurexProvider {
    pub fn new(client: Arc<XyraAurexClient>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl StreamProvider for XyraAurexProvider {
    fn name(&self) -> &'static str {
        "Aurex"
    }
    fn real_name(&self) -> &'static str {
        "aurex"
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
        let year_str = tmdb_info.year.map(|y| y.to_string());
        self.client
            .get_streams(
                tmdb_info.id,
                &tmdb_info.title,
                year_str.as_deref(),
                tmdb_info.imdb_id.as_deref(),
                media_type,
                season,
                episode,
            )
            .await
    }
}
