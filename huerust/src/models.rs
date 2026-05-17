//! Hue Stream API
//! Professional Streaming Aggregator
//!
//! @author roywalk3r <roywalker440@gmail.com>
//! @repository https://github.com/roywalk3r/hue.git
//! @license MIT

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use utoipa::ToSchema;

#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
pub struct StreamResult {
    pub url: String,
    pub title: String,
    pub quality: String,
    pub provider: String,
    pub format: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subtitles: Option<Vec<Subtitle>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra_metadata: Option<HashMap<String, String>>,
}

#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
pub struct Subtitle {
    pub url: String,
    pub lang: String,
    pub label: String,
}

pub mod anime;
