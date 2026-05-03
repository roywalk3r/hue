//! Hue Stream API
//! Professional Streaming Aggregator
//!
//! @author roywalk3r <roywalker440@gmail.com>
//! @repository https://github.com/roywalk3r/hue.git
//! @license MIT

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;
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

#[derive(Debug, Serialize, ToSchema)]
pub struct ErrorResponse {
    pub success: bool,
    pub error: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Error)]
#[allow(dead_code)]
pub enum HueError {
    #[error("Provider not found: {0}")]
    ProviderNotFound(String),
    #[error("TMDB lookup failed: {0}")]
    TmdbError(String),
    #[error("Stream extraction failed: {0}")]
    StreamError(String),
    #[error("Rate limit exceeded")]
    RateLimited,
}
