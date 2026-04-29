use reqwest::Client;
use anyhow::Result;
use regex::Regex;
use std::collections::HashMap;

const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";
const VIXSRC_REFERER: &str = "https://vixsrc.to/";

#[tokio::main]
async fn main() -> Result<()> {
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()?;
    
    let tmdb_id = 76479;
    let media_type = "tv";
    let season = 1;
    let episode = 1;

    println!("Fetching: TMDB {} ({}) S{} E{}", tmdb_id, media_type, season, episode);

    let api_url = format!("https://vixsrc.to/api/tv/{}/{}/{}", tmdb_id, season, episode);
    let referer = format!("https://vixsrc.to/tv/{}/{}/{}", tmdb_id, season, episode);

    let api_resp = client
        .get(&api_url)
        .header("User-Agent", USER_AGENT)
        .header("Referer", &referer)
        .header("Accept", "application/json")
        .send()
        .await?;

    if !api_resp.status().is_success() {
        println!("API request failed with status: {}", api_resp.status());
        return Ok(());
    }

    let api_data: serde_json::Value = api_resp.json().await?;
    let embed_path = match api_data["src"].as_str() {
        Some(p) => p,
        None => {
            println!("No 'src' field in API response");
            return Ok(());
        }
    };

    println!("Embed Path: {}", embed_path);

    let embed_url = format!("https://vixsrc.to{}", embed_path);
    let embed_resp = client
        .get(&embed_url)
        .header("User-Agent", USER_AGENT)
        .header("Referer", VIXSRC_REFERER)
        .send()
        .await?;

    let html = embed_resp.text().await?;
    
    if html.contains("window.masterPlaylist") {
        println!("Found window.masterPlaylist");

        let url_re = Regex::new(r#"url\s*:\s*['"]([^'"]+)['"]"#)?;
        let token_re = Regex::new(r#"['"]?token['"]?\s*:\s*['"]([^'"]+)['"]"#)?;
        let expires_re = Regex::new(r#"['"]?expires['"]?\s*:\s*['"]([^'"]+)['"]"#)?;

        let base_url = url_re.captures(&html).and_then(|c| c.get(1)).map(|m| m.as_str());
        let token = token_re.captures(&html).and_then(|c| c.get(1)).map(|m| m.as_str());
        let expires = expires_re.captures(&html).and_then(|c| c.get(1)).map(|m| m.as_str());

        if let (Some(base), Some(t), Some(e)) = (base_url, token, expires) {
            println!("Extracted base_url={}, token={}, expires={}", base, t, e);
            let constructed = if base.contains('?') {
                format!("{}&token={}&expires={}&h=1&lang=en", base, t, e)
            } else {
                format!("{}?token={}&expires={}&h=1&lang=en", base, t, e)
            };
            println!("Final URL: {}", constructed);
        } else {
            println!("Failed to extract all fields from window.masterPlaylist");
            println!("Base: {:?}, Token: {:?}, Expires: {:?}", base_url, token, expires_re.captures(&html));
        }
    } else {
        println!("window.masterPlaylist NOT found in HTML");
    }

    Ok(())
}
