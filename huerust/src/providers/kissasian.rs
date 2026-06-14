//! Hue Stream API
//! Professional Streaming Aggregator
//!
//! @author roywalk3r <roywalker440@gmail.com>
//! @repository https://github.com/roywalk3r/hue.git
//! @license MIT

use crate::models::StreamResult;
use anyhow::Result;
use regex::Regex;
use reqwest_middleware::ClientWithMiddleware;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

const BASE_URL: &str = "https://kissasian.cam";
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KissAsianSearchItem {
    pub id: String,
    pub title: String,
    pub slug: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KissAsianEpisode {
    pub number: u32,
    pub url: String,
}

pub struct KissAsianClient {
    client: ClientWithMiddleware,
    scraper_url: String,
}

impl KissAsianClient {
    pub fn new() -> Self {
        let scraper_url = std::env::var("MICROSERVICE_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:24610".to_string());
        Self {
            client: crate::http_client::wrap_with_retry(reqwest::Client::new()),
            scraper_url,
        }
    }

    pub async fn search(&self, query: &str) -> Result<Vec<KissAsianSearchItem>> {
        let url = format!("{}/?s={}", BASE_URL, urlencoding::encode(query));
        let resp = self
            .client
            .get(&url)
            .header("User-Agent", USER_AGENT)
            .send()
            .await?;
        let body = resp.text().await?;

        let article_re = Regex::new(r"<article[\s\S]*?</article>").unwrap();
        let link_re = Regex::new(r#"href="(https://kissasian\.cam/series/([^"/]+)/)""#).unwrap();
        let title_re = Regex::new(r"<h[1-6][^>]*>([^<]+)</h[1-6]>").unwrap();

        let mut results = Vec::new();

        for article in article_re.find_iter(&body) {
            let art = article.as_str();
            let link_match = link_re.captures(art);
            let title_match = title_re.captures(art);

            if let (Some(link), Some(title)) = (link_match, title_match) {
                results.push(KissAsianSearchItem {
                    id: link.get(2).unwrap().as_str().to_string(),
                    title: title.get(1).unwrap().as_str().trim().to_string(),
                    slug: link.get(2).unwrap().as_str().to_string(),
                    url: link.get(1).unwrap().as_str().to_string(),
                });
            }
        }

        Ok(results)
    }

    pub async fn get_episodes(&self, slug: &str) -> Result<Vec<KissAsianEpisode>> {
        let url = format!("{}/series/{}/", BASE_URL, urlencoding::encode(slug));
        let resp = self
            .client
            .get(&url)
            .header("User-Agent", USER_AGENT)
            .send()
            .await?;
        let body = resp.text().await?;

        let ep_re =
            Regex::new(r#"href="(https://kissasian\.cam/([^"]*episode-(\d+)[^"]*)\/)""#).unwrap();

        let mut episodes = Vec::new();
        let mut seen = std::collections::HashSet::new();

        for caps in ep_re.captures_iter(&body) {
            let ep_url = caps.get(1).unwrap().as_str();
            let path = caps.get(2).unwrap().as_str();
            let num: u32 = caps.get(3).unwrap().as_str().parse().unwrap_or(0);

            if num == 0 || seen.contains(ep_url) {
                continue;
            }
            seen.insert(ep_url);

            // Only include URLs that contain the series slug
            if !path.contains(slug) {
                continue;
            }

            episodes.push(KissAsianEpisode {
                number: num,
                url: ep_url.to_string(),
            });
        }

        episodes.sort_by_key(|e| e.number);
        Ok(episodes)
    }

    pub async fn get_stream(&self, episode_url: &str) -> Result<Vec<StreamResult>> {
        let payload = serde_json::json!({ "url": episode_url });

        let resp = self
            .client
            .post(format!("{}/scrape/kissasian", self.scraper_url))
            .json(&payload)
            .send()
            .await?;

        if !resp.status().is_success() {
            return Ok(vec![]);
        }

        #[derive(Deserialize)]
        struct ScraperResponse {
            success: bool,
            streams: Vec<StreamResult>,
        }

        let data: ScraperResponse = resp.json().await?;
        if data.success {
            Ok(data.streams)
        } else {
            Ok(vec![])
        }
    }

    pub async fn get_streams_by_slug(&self, slug: &str, episode: u32) -> Result<Vec<StreamResult>> {
        info!(
            "[Pearl] Getting streams for slug={} episode={}",
            slug, episode
        );

        let episodes = self.get_episodes(slug).await?;
        if episodes.is_empty() {
            warn!("[Pearl] No episodes found for slug={}", slug);
            return Ok(vec![]);
        }

        let target_ep = episodes.iter().find(|e| e.number == episode);
        let ep = match target_ep {
            Some(e) => e,
            None => {
                warn!(
                    "[Pearl] Episode {} not found for slug={} ({} episodes available)",
                    episode,
                    slug,
                    episodes.len()
                );
                return Ok(vec![]);
            }
        };

        self.get_stream(&ep.url).await
    }

    pub async fn get_streams_normalized(
        &self,
        title: &str,
        season: u32,
        episode: u32,
    ) -> Result<Vec<StreamResult>> {
        info!(
            "[Pearl] Searching for: \"{}\" S{}E{}",
            title, season, episode
        );

        let search_query = if season > 1 {
            format!("{} Season {}", title, season)
        } else {
            title.to_string()
        };

        let mut results = self.search(&search_query).await?;

        if results.is_empty() && season > 1 {
            info!("[Pearl] Fallback search: {}", title);
            results = self.search(title).await?;
        }

        if results.is_empty() {
            warn!("[Pearl] No search results for: {}", search_query);
            return Ok(vec![]);
        }

        let target_norm = normalize(title);
        let best_match = results.iter().find(|r| {
            let r_norm = normalize(&r.title);
            r_norm.contains(&target_norm) || target_norm.contains(&r_norm)
        });

        let best = match best_match {
            Some(b) => b,
            None => &results[0],
        };

        info!(
            "[Pearl] Chose series: \"{}\" (slug={})",
            best.title, best.slug
        );

        self.get_streams_by_slug(&best.slug, episode).await
    }
}

fn normalize(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect()
}
