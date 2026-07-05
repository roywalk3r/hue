//! Hue Stream API
//! Professional Streaming Aggregator
//!
//! @author roywalk3r <roywalker440@gmail.com>
//! @repository https://github.com/roywalk3r/hue.git
//! @license MIT

use crate::models::StreamResult;
use crate::providers::kissasian_top::KissAsianTopClient;
use crate::providers::StreamProvider;
use anyhow::Result;
use std::sync::Arc;

pub struct KissAsianTopProvider {
    client: Arc<KissAsianTopClient>,
}

impl KissAsianTopProvider {
    pub fn new(client: Arc<KissAsianTopClient>) -> Self {
        Self { client }
    }
}

#[async_trait::async_trait]
impl StreamProvider for KissAsianTopProvider {
    fn name(&self) -> &'static str {
        "Rose"
    }
    fn real_name(&self) -> &'static str {
        "kissasian_top"
    }
    /// Direct MP4 — no scraping step, just one upstream GET.
    fn is_fast(&self) -> bool {
        true
    }
    fn timeout_secs(&self) -> u64 {
        6
    }

    async fn fetch(
        &self,
        tmdb_info: &crate::clients::tmdb::TmdbNormalizedInfo,
        _media_type: &str,
        _season: u32,
        episode: u32,
    ) -> Result<Vec<StreamResult>> {
        // kissasian.top is slug-keyed (no TMDB↔slug mapping yet), so the SSE
        // path can't usefully dispatch here. Left wired for future use.
        tracing::info!(
            "[Rose Provider] SSE fetch not supported for kissasian.top ({} E{})",
            tmdb_info.title,
            episode
        );
        Ok(vec![])
    }
}
