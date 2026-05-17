//! Hue Stream API
//! Professional Streaming Aggregator
//!
//! @author roywalk3r <roywalker440@gmail.com>
//! @repository https://github.com/roywalk3r/hue.git
//! @license MIT

use async_trait::async_trait;
use crate::providers::StreamProvider;
use crate::models::StreamResult;
use crate::clients::tmdb::TmdbNormalizedInfo;
use crate::providers::sflix::SflixClient;
use anyhow::Result;
use chrono::{NaiveDate, Utc};
use std::sync::Arc;

pub struct SflixProvider {
    client: Arc<SflixClient>,
}

impl SflixProvider {
    pub fn new(client: Arc<SflixClient>) -> Self {
        Self { client }
    }
}

fn is_recent_or_unreleased(release_date: Option<&str>, window_days: i64) -> bool {
    let Some(release_date) = release_date else {
        return false;
    };
    let Ok(date) = NaiveDate::parse_from_str(release_date, "%Y-%m-%d") else {
        return false;
    };
    let today = Utc::now().date_naive();
    let age_days = today.signed_duration_since(date).num_days();
    age_days <= window_days
}

#[async_trait]
impl StreamProvider for SflixProvider {
    fn name(&self) -> &'static str { "Nova" }
    fn real_name(&self) -> &'static str { "sflix" }
    fn is_fast(&self) -> bool { true }
    fn timeout_secs(&self) -> u64 { 30 }

    async fn fetch(
        &self,
        tmdb_info: &TmdbNormalizedInfo,
        media_type: &str,
        season: u32,
        episode: u32,
    ) -> Result<Vec<StreamResult>> {
        if media_type == "movie" && is_recent_or_unreleased(tmdb_info.release_date.as_deref(), 14) {
            tracing::info!(
                "[SFlix Provider] Skipping recent/unreleased movie {} ({:?}); catalog streams are likely placeholders",
                tmdb_info.id,
                tmdb_info.release_date
            );
            return Ok(vec![]);
        }

        self.client.get_streams_normalized(&tmdb_info.title, media_type, tmdb_info.year, season, episode).await
    }
}
