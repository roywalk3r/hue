//! Hue Stream API
//! Professional Streaming Aggregator
//!
//! @author roywalk3r <roywalker440@gmail.com>
//! @repository https://github.com/roywalk3r/hue.git
//! @license MIT

use reqwest::Client;
use serde::de::DeserializeOwned;
use tracing::{info, warn};
use anyhow::{Result, anyhow};

#[allow(dead_code)]
fn microservice_url() -> String {
    std::env::var("microservice_url()").unwrap_or_else(|_| "http://127.0.0.1:24610".to_string())
}

#[derive(Debug, serde::Deserialize)]
#[allow(dead_code)]
pub struct ScraperResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub streams: Option<Vec<crate::models::StreamResult>>,
}

#[allow(dead_code)]
pub async fn scrape_via_microservice<T: DeserializeOwned>(
    client: &Client,
    provider: &str,
    payload: serde_json::Value,
) -> Result<Option<T>> {
    let url = format!("{}/scrape/{}", microservice_url(), provider);
    info!("[microservice] Calling {} with payload: {}", url, payload);

    let resp = client
        .post(&url)
        .json(&payload)
        .send()
        .await
        .map_err(|e| {
            warn!("[microservice] Request to {} failed: {}", url, e);
            anyhow!("Microservice request failed: {}", e)
        })?;

    if !resp.status().is_success() {
        warn!("[microservice] Non-success status from {}: {}", url, resp.status());
        return Ok(None);
    }

    let data: ScraperResponse<T> = resp.json().await.map_err(|e| {
        warn!("[microservice] Failed to decode JSON from {}: {}", url, e);
        anyhow!("Microservice JSON decode failed: {}", e)
    })?;

    if data.success {
        Ok(data.data.or_else(|| {
            // For backwards compatibility with endpoints that return streams directly
            None
        }))
    } else {
        warn!("[microservice] Scraper reported failure for {}", provider);
        Ok(None)
    }
}

#[allow(dead_code)]
pub async fn scrape_streams_via_microservice(
    client: &Client,
    provider: &str,
    payload: serde_json::Value,
) -> Result<Vec<crate::models::StreamResult>> {
    let url = format!("{}/scrape/{}", microservice_url(), provider);
    info!("[microservice] Calling {} for streams", url);

    let resp = client
        .post(&url)
        .json(&payload)
        .send()
        .await
        .map_err(|e| {
            warn!("[microservice] Request to {} failed: {}", url, e);
            anyhow!("Microservice request failed: {}", e)
        })?;

    if !resp.status().is_success() {
        warn!("[microservice] Non-success status from {}: {}", url, resp.status());
        return Ok(vec![]);
    }

    let data: ScraperResponse<serde_json::Value> = resp.json().await.map_err(|e| {
        warn!("[microservice] Failed to decode JSON from {}: {}", url, e);
        anyhow!("Microservice JSON decode failed: {}", e)
    })?;

    if data.success {
        if let Some(streams) = data.streams {
            return Ok(streams);
        }
    }

    warn!("[microservice] No streams returned from {}", provider);
    Ok(vec![])
}
