use reqwest::Client;
use anyhow::{Result, anyhow};
use tracing::info;

pub async fn get_vixsrc_streams(client: &Client, m_type: &str, id: &str) -> Result<String> {
    let url = if m_type == "movie" {
        format!("https://vixsrc.to/movie/{}", id)
    } else {
        format!("https://vixsrc.to/tv/{}/1/1", id)
    };

    let resp = client.get(&url).send().await?;
    let html = resp.text().await?;
    info!("Scraped Vixsrc for {} {}, length: {}", m_type, id, html.len());
    
    // Scraper implementation here
    Ok("Scraper parsing coming soon".to_string())
}
