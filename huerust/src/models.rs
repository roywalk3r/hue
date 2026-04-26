use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize, Clone)]
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
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Subtitle {
    pub url: String,
    pub lang: String,
    pub label: String,
}
