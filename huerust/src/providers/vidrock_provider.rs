//! Hue Stream API
//! Professional Streaming Aggregator
//!
//! @author roywalk3r <roywalker440@gmail.com>
//! @repository https://github.com/roywalk3r/hue.git
//! @license MIT

use async_trait::async_trait;
use crate::providers::StreamProvider;
use crate::clients::vidrock::VidRockClient;
use crate::models::{StreamResult, Subtitle};
use crate::clients::tmdb::TmdbNormalizedInfo;
use anyhow::Result;
use std::sync::Arc;

pub struct VidrockProvider {
    client: Arc<VidRockClient>,
}

impl VidrockProvider {
    pub fn new(client: Arc<VidRockClient>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl StreamProvider for VidrockProvider {
    fn name(&self) -> &'static str { "Luna" }
    fn real_name(&self) -> &'static str { "vidrock" }
    fn is_fast(&self) -> bool { true }
    fn timeout_secs(&self) -> u64 { 15 }

    async fn fetch(
        &self,
        tmdb_info: &TmdbNormalizedInfo,
        media_type: &str,
        season: u32,
        episode: u32,
    ) -> Result<Vec<StreamResult>> {
        let tmdb_id = tmdb_info.id.to_string();
        let streams = self.client.get_streams(
            &tmdb_id,
            media_type,
            Some(season),
            Some(episode),
        ).await?;

        let mut results = Vec::with_capacity(streams.len());
        for s in streams {
            // Convert serde_json::Value headers to HashMap<String, String>
            let headers = match s.headers {
                serde_json::Value::Object(map) => {
                    let mut h = std::collections::HashMap::new();
                    for (k, v) in map {
                        if let Some(vs) = v.as_str() {
                            h.insert(k, vs.to_string());
                        }
                    }
                    if h.is_empty() { None } else { Some(h) }
                }
                _ => None,
            };

            // Convert VidRock subtitles to Subtitle
            let subtitles = s.subtitles.map(|subs| {
                subs.into_iter()
                    .map(|sub| Subtitle {
                        url: sub.url,
                        lang: sub.lang,
                        label: sub.label,
                    })
                    .collect::<Vec<_>>()
            });

            results.push(StreamResult {
                url: s.url,
                title: s.title,
                quality: s.quality,
                provider: "Luna".to_string(),
                format: s.format,
                subtitles: if subtitles.as_ref().map(|v| v.is_empty()).unwrap_or(true) {
                    None
                } else {
                    subtitles
                },
                headers,
                extra_metadata: None,
            });
        }

        Ok(results)
    }
}
