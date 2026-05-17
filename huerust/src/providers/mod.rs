//! Hue Stream API
//! Professional Streaming Aggregator
//!
//! @author roywalk3r <roywalker440@gmail.com>
//! @repository https://github.com/roywalk3r/hue.git
//! @license MIT

pub mod registry;
pub mod dispatch;
pub mod vixsrc;
pub mod kisskh;
pub mod sflix;
pub mod dramacool;
pub mod mp4hydra;
pub mod vidzee;
pub mod videasy;
pub mod moviebox_provider;
pub mod kisskh_provider;
pub mod sflix_provider;
pub mod dramacool_provider;
pub mod vixsrc_provider;
pub mod mp4hydra_provider;
pub mod vidzee_provider;
pub mod videasy_provider;
pub mod vidrock_provider;

use async_trait::async_trait;
use crate::models::StreamResult;
use crate::clients::tmdb::TmdbNormalizedInfo;
use anyhow::Result;

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
