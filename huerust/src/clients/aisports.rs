//! AiSports Client
//! Fetches live sports streams from the Aoneroom API (sportslivenow.top)

use anyhow::{Result, anyhow};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

const API_LIST_URL: &str = "https://h5.aoneroom.com/wefeed-h5-bff/live/match-list-v5?leagueId=0";
const API_DETAIL_URL: &str = "https://h5.aoneroom.com/wefeed-h5-bff/sport/detail-v1";
const USER_AGENT: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 18_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.5 Mobile/15E148 Safari/604.1";

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SportsStream {
    pub title: String,
    pub url: String,
    pub quality: String,
    pub provider: String,
    pub source: String,
    pub is_live: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<std::collections::HashMap<String, String>>,
}

#[derive(Debug, Deserialize)]
struct ApiResponse<T> {
    code: i32,
    #[serde(default)]
    data: Option<T>,
}

#[derive(Debug, Deserialize, Default)]
struct MatchListData {
    #[serde(default)]
    list: Vec<Match>,
}

#[derive(Debug, Deserialize, Clone)]
#[allow(non_snake_case)]
struct Match {
    id: String,
    #[serde(default)]
    team1: Option<Team>,
    #[serde(default)]
    team2: Option<Team>,
    #[serde(default)]
    playPath: Option<String>,
    #[serde(default)]
    playSource: Option<Vec<PlaySource>>,
    #[serde(default)]
    highlights: Option<Vec<Highlight>>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    league: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
struct Team {
    #[serde(default)]
    name: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
struct PlaySource {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    path: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
struct Highlight {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    duration: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct MatchDetail {
    #[serde(default)]
    #[serde(rename = "match")]
    match_info: Option<Match>,
}

pub struct AiSportsClient {
    client: Client,
}

impl AiSportsClient {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    async fn get_matches(&self) -> Result<Vec<Match>> {
        let resp = self.client
            .get(API_LIST_URL)
            .header("User-Agent", USER_AGENT)
            .header("Origin", "https://sportslivenow.top")
            .header("Referer", "https://sportslivenow.top/")
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(anyhow!("AiSports API returned {}", resp.status()));
        }

        let api_resp: ApiResponse<MatchListData> = resp.json().await?;
        if api_resp.code != 0 {
            return Err(anyhow!("AiSports API code: {}", api_resp.code));
        }

        Ok(api_resp.data.map(|d| d.list).unwrap_or_default())
    }

    async fn get_match_detail(&self, match_id: &str) -> Result<Option<Match>> {
        let url = format!("{}?matchId={}", API_DETAIL_URL, match_id);
        let resp = self.client
            .get(&url)
            .header("User-Agent", USER_AGENT)
            .header("Origin", "https://sportslivenow.top")
            .header("Referer", "https://sportslivenow.top/")
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(anyhow!("AiSports detail API returned {}", resp.status()));
        }

        let api_resp: ApiResponse<MatchDetail> = resp.json().await?;
        if api_resp.code != 0 {
            return Err(anyhow!("AiSports detail API code: {}", api_resp.code));
        }

        Ok(api_resp.data.and_then(|d| d.match_info))
    }

    pub async fn search_streams(&self, query: &str) -> Result<Vec<SportsStream>> {
        let clean_query = query.to_lowercase().replace("-", " ");
        info!("[AiSports] Searching for: {}", query);

        let matches = self.get_matches().await?;
        if matches.is_empty() {
            warn!("[AiSports] No matches returned from API");
            return Ok(vec![]);
        }

        let best_match = matches.iter().find(|m| {
            let title = format!(
                "{} vs {}",
                m.team1.as_ref().and_then(|t| t.name.clone()).unwrap_or_default(),
                m.team2.as_ref().and_then(|t| t.name.clone()).unwrap_or_default()
            )
            .to_lowercase();
            let team1 = m.team1.as_ref().and_then(|t| t.name.clone()).unwrap_or_default().to_lowercase();
            let team2 = m.team2.as_ref().and_then(|t| t.name.clone()).unwrap_or_default().to_lowercase();

            title.contains(&clean_query)
                || clean_query.contains(&team1)
                || clean_query.contains(&team2)
                || m.id.to_lowercase().contains(query)
        });

        let m = match best_match {
            Some(m) => m.clone(),
            None => {
                info!("[AiSports] No match found for '{}'", query);
                return Ok(vec![]);
            }
        };

        info!(
            "[AiSports] Found match: {} vs {} (ID: {})",
            m.team1.as_ref().and_then(|t| t.name.clone()).unwrap_or_default(),
            m.team2.as_ref().and_then(|t| t.name.clone()).unwrap_or_default(),
            m.id
        );

        // Fetch detail for more sources
        let detail = self.get_match_detail(&m.id).await?;
        let active = detail.as_ref().unwrap_or(&m);

        let team1_name = active.team1.as_ref().and_then(|t| t.name.clone()).unwrap_or_else(|| "Unknown".to_string());
        let team2_name = active.team2.as_ref().and_then(|t| t.name.clone()).unwrap_or_else(|| "Unknown".to_string());

        let headers = {
            let mut h = std::collections::HashMap::new();
            h.insert("Referer".to_string(), "https://sportslivenow.top/".to_string());
            h.insert("Origin".to_string(), "https://sportslivenow.top".to_string());
            h.insert("User-Agent".to_string(), USER_AGENT.to_string());
            h
        };

        let mut streams = Vec::new();

        // Main stream
        if let Some(path) = &active.playPath {
            if !path.is_empty() {
                streams.push(SportsStream {
                    title: format!("{} vs {} (Main)", team1_name, team2_name),
                    url: path.clone(),
                    quality: "Auto".to_string(),
                    provider: "Field".to_string(),
                    source: "aisports".to_string(),
                    is_live: true,
                    headers: Some(headers.clone()),
                });
            }
        }

        // Additional sources
        if let Some(sources) = &active.playSource {
            for (idx, source) in sources.iter().enumerate() {
                if let Some(path) = &source.path {
                    if !path.is_empty() {
                        streams.push(SportsStream {
                            title: format!(
                                "{} vs {} ({})",
                                team1_name,
                                team2_name,
                                source.title.clone().unwrap_or_else(|| format!("Channel {}", idx + 1))
                            ),
                            url: path.clone(),
                            quality: "Auto".to_string(),
                            provider: "Field".to_string(),
                            source: "aisports".to_string(),
                            is_live: true,
                            headers: Some(headers.clone()),
                        });
                    }
                }
            }
        }

        // Highlights
        if let Some(highlights) = &active.highlights {
            for h in highlights {
                if let Some(path) = &h.path {
                    if !path.is_empty() {
                        streams.push(SportsStream {
                            title: format!(
                                "{} vs {} - {}",
                                team1_name,
                                team2_name,
                                h.title.clone().unwrap_or_else(|| "Highlight".to_string())
                            ),
                            url: path.clone(),
                            quality: "720p".to_string(),
                            provider: "Stadium".to_string(),
                            source: "aisports-highlight".to_string(),
                            is_live: false,
                            headers: None,
                        });
                    }
                }
            }
        }

        info!("[AiSports] Returning {} streams", streams.len());
        Ok(streams)
    }
}
