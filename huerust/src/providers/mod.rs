//! Hue Stream API
//! Professional Streaming Aggregator
//!
//! @author roywalk3r <roywalker440@gmail.com>
//! @repository https://github.com/roywalk3r/hue.git
//! @license MIT

pub mod dispatch;
pub mod dramacool;
pub mod dramacool_provider;
pub mod kissasian;
pub mod kissasian_provider;
pub mod kissasian_top;
pub mod kissasian_top_provider;
pub mod kisskh;
pub mod kisskh_provider;
pub mod kisskh_xyra;
pub mod kisskh_xyra_provider;
pub mod lordflix_provider;
pub mod moviebox_provider;
pub mod mp4hydra;
pub mod mp4hydra_provider;
pub mod notorrent_provider;
pub mod registry;
// pub mod streamhub;          // disabled — upstream returns Aura duplicates
// pub mod streamhub_provider;
pub mod sflix;
pub mod sflix_provider;
pub mod videasy;
pub mod videasy_provider;
pub mod vidlink_provider;
pub mod vidrock_provider;
pub mod vidsrcembed_provider;
pub mod vidvault_provider;
pub mod vidzee;
pub mod vidzee_provider;
pub mod vixsrc;
pub mod vixsrc_provider;
pub mod xyra_subtitle;

use crate::clients::tmdb::TmdbNormalizedInfo;
use crate::models::StreamResult;
use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
pub trait StreamProvider: Send + Sync {
    /// Public display name (e.g. "Cinema", "Nova", "Zenith")
    fn name(&self) -> &'static str;

    /// Internal provider name (e.g. "moviebox", "sflix", "kisskh")
    fn real_name(&self) -> &'static str;

    /// Whether this provider is classified as fast (low latency)
    fn is_fast(&self) -> bool;

    /// Individual timeout for this provider in seconds
    fn timeout_secs(&self) -> u64;

    /// Fetch streams for the given title / episode
    async fn fetch(
        &self,
        tmdb_info: &TmdbNormalizedInfo,
        media_type: &str,
        season: u32,
        episode: u32,
    ) -> Result<Vec<StreamResult>>;
}

/// Map a provider's public display name to its internal `real_name`.
/// This is the stable mapping used by sticky preferences and circuit breakers.
pub fn display_to_real_name(display: &str) -> &str {
    match display {
        "Cinema" => "moviebox",
        "Nova" => "sflix",
        "Luna" => "vidrock",
        "Aura" => "vixsrc",
        "Zenith" => "kisskh",
        "Aurora" => "kisskh_xyra",
        "Orient" => "dramacool",
        "Pearl" => "kissasian",
        "Rose" => "kissasian_top",
        "Hydra" => "mp4hydra",
        "VidZee" => "vidzee",
        "Vortex" => "videasy",
        "Nebula" => "vidsrcembed",
        "Vault" => "vidvault",
        "StreamHub" => "streamhub",
        other => other,
    }
}

/// Pick the "best" provider from a non-empty result list. The SSE aggregator
/// inserts results in the order providers complete, so the first stream's
/// provider is the fastest healthy one for this title.
pub fn best_provider_real_name(streams: &[StreamResult]) -> Option<&str> {
    streams.first().map(|s| display_to_real_name(&s.provider))
}
