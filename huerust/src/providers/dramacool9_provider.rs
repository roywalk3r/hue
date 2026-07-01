//! Dramacool9 StreamProvider — wraps the Dramacool9 client (dramacool9.com.ro).

use crate::clients::tmdb::TmdbNormalizedInfo;
use crate::models::StreamResult;
use crate::providers::dramacool9::Dramacool9Client;
use crate::providers::StreamProvider;
use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;

pub struct Dramacool9Provider {
    client: Arc<Dramacool9Client>,
}

impl Dramacool9Provider {
    pub fn new(client: Arc<Dramacool9Client>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl StreamProvider for Dramacool9Provider {
    fn name(&self) -> &'static str {
        "Orient9"
    }
    fn real_name(&self) -> &'static str {
        "dramacool9"
    }
    fn is_fast(&self) -> bool {
        false
    }
    fn timeout_secs(&self) -> u64 {
        25
    }

    async fn fetch(
        &self,
        tmdb_info: &TmdbNormalizedInfo,
        media_type: &str,
        season: u32,
        episode: u32,
    ) -> Result<Vec<StreamResult>> {
        // Dramacool9 is a free-form search provider — we feed it the TMDB
        // normalized title (set for both movies and TV in TmdbNormalizedInfo).
        let title = tmdb_info.title.clone();
        if title.is_empty() {
            return Ok(vec![]);
        }
        // For TV, prefix with the season for better hit rate on multi-season shows.
        let query = if media_type == "tv" && season > 1 {
            format!("{} season {}", title, season)
        } else {
            title
        };
        self.client
            .get_streams_normalized(&query, season, episode)
            .await
    }
}
