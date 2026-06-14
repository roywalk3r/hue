//! Hue Stream API
//! Professional Streaming Aggregator
//!
//! @author roywalk3r <roywalker440@gmail.com>
//! @repository https://github.com/roywalk3r/hue.git
//! @license MIT

use crate::clients::tmdb::TmdbNormalizedInfo;
use crate::clients::vidvault::VidVaultClient;
use crate::models::{StreamResult, Subtitle};
use crate::providers::StreamProvider;
use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;

pub struct VidvaultProvider {
    client: Arc<VidVaultClient>,
}

impl VidvaultProvider {
    pub fn new(client: Arc<VidVaultClient>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl StreamProvider for VidvaultProvider {
    fn name(&self) -> &'static str {
        "Vault"
    }
    fn real_name(&self) -> &'static str {
        "vidvault"
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
        let tmdb_id = tmdb_info.id.to_string();
        let streams = self
            .client
            .get_downloads(&tmdb_id, media_type, season, episode)
            .await?;

        let mut results = Vec::with_capacity(streams.len());
        for s in streams {
            let subtitles = s.subtitles.map(|subs| {
                subs.into_iter()
                    .map(|sub| Subtitle {
                        url: sub.url,
                        lang: sub.lan.clone(),
                        label: sub.lan_name,
                    })
                    .collect::<Vec<_>>()
            });

            results.push(StreamResult {
                url: s.url,
                title: s.title,
                quality: s.quality,
                provider: "Vault".to_string(),
                format: s.format,
                subtitles: if subtitles.as_ref().map(|v| v.is_empty()).unwrap_or(true) {
                    None
                } else {
                    subtitles
                },
                headers: s.headers,
                extra_metadata: None,
            });
        }

        Ok(results)
    }
}
