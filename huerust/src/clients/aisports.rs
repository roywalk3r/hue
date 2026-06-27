//! AiSports Client
//! Fetches live sports streams from the Aoneroom API (thesports.today)

use anyhow::{anyhow, Result};
use reqwest_middleware::ClientWithMiddleware;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};
use chrono::prelude::*;

const API_LIST_URL: &str =
    "https://h5-api.aoneroom.com/wefeed-h5api-bff/live/match-list-v5?leagueId=0";
const API_LIST_V5_URL: &str = "https://h5-api.aoneroom.com/wefeed-h5api-bff/live/match-list-v5";
const API_LIST_V3_URL: &str = "https://h5-api.aoneroom.com/wefeed-h5api-bff/live/match-list-v3";
const API_DETAIL_URL: &str = "https://h5-sport-api.aoneroom.com/wefeed-h5api-bff/sport/detail-v1";
const API_LATEST_URL: &str = "https://h5-api.aoneroom.com/wefeed-h5api-bff/live/latest-match";
const API_AGGREGATE_URL: &str = "https://h5-api.aoneroom.com/wefeed-h5api-bff/sport/aggregate-v1";
const LEAGUE_TAB_URL: &str = "https://h5-api.aoneroom.com/wefeed-h5api-bff/live/league-tab";
const USER_AGENT: &str = "Mozilla/5.0 (Linux; Android 6.0; Nexus 5 Build/MRA58N) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/147.0.0.0 Mobile Safari/537.36";
const ORIGIN: &str = "https://thesports.today";
const REFERER: &str = "https://thesports.today/";
const X_CLIENT_INFO: &str = r#"{"timezone":"Africa/Accra","system_language":"en"}"#;
const X_DEVICE_INFO: &str = "{}";
const BLOCKED_CHANNEL_HOST: &str = "sportsteam368.com";

#[derive(Debug, Serialize, Deserialize, Clone, utoipa::ToSchema)]
pub struct SportsStream {
    pub title: String,
    pub url: String,
    pub quality: String,
    pub provider: String,
    pub source: String,
    pub is_live: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_replay: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<std::collections::HashMap<String, String>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GroupedSportsStreams {
    pub live: Vec<SportsStream>,
    pub replay: Vec<SportsStream>,
    pub highlights: Vec<SportsStream>,
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
pub struct Match {
    pub id: String,
    #[serde(default)]
    pub team1: Option<Team>,
    #[serde(default)]
    pub team2: Option<Team>,
    #[serde(default)]
    pub playPath: Option<String>,
    #[serde(default)]
    pub playSource: Option<Vec<PlaySource>>,
    #[serde(default)]
    pub highlights: Option<Vec<Highlight>>,
    #[serde(default)]
    pub replay: Option<Vec<Replay>>,
    #[serde(default, rename = "teamMatchInfo1")]
    pub team_match_info1: Option<TeamMatchInfo>,
    #[serde(default, rename = "teamMatchInfo2")]
    pub team_match_info2: Option<TeamMatchInfo>,
    #[allow(dead_code)]
    #[serde(default)]
    pub status: Option<String>,
    #[allow(dead_code)]
    #[serde(default, rename = "statusLive")]
    pub status_live: Option<String>,
    #[allow(dead_code)]
    #[serde(default)]
    pub league: Option<String>,
    #[serde(default, rename = "matchRound")]
    pub match_round: Option<String>,
    #[serde(default, rename = "timeDesc")]
    pub time_desc: Option<String>,
    #[serde(default, rename = "arrangedTime")]
    pub arranged_time: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct TeamMatchInfo {
    #[serde(default)]
    pub score: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Team {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub avatar: Option<String>,
    #[serde(default)]
    pub abbreviation: Option<String>,
    #[serde(default)]
    pub score: Option<String>,
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
    #[allow(dead_code)]
    #[serde(default)]
    duration: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
struct Replay {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    cover: Option<Cover>,
    #[allow(dead_code)]
    #[serde(default)]
    duration: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
struct Cover {
    #[serde(default)]
    url: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct MatchDetail {
    #[serde(default)]
    #[serde(rename = "match")]
    match_info: Option<Match>,
}

pub struct AiSportsClient {
    client: ClientWithMiddleware,
}

impl AiSportsClient {
    pub fn new(client: ClientWithMiddleware) -> Self {
        Self { client }
    }

    async fn get_json(&self, url: reqwest::Url, label: &str) -> Result<serde_json::Value> {
        let resp = self
            .client
            .get(url)
            .header("User-Agent", USER_AGENT)
            .header("Accept", "application/json")
            .header("Content-Type", "application/json")
            .header("Origin", ORIGIN)
            .header("Referer", REFERER)
            .header("x-client-info", X_CLIENT_INFO)
            .header("x-device-info", X_DEVICE_INFO)
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(anyhow!("AiSports {} API returned {}", label, resp.status()));
        }

        let api_resp: ApiResponse<serde_json::Value> = resp.json().await?;
        if api_resp.code != 0 {
            return Err(anyhow!("AiSports {} API code: {}", label, api_resp.code));
        }

        Ok(api_resp.data.unwrap_or_else(|| serde_json::json!({})))
    }

    pub async fn get_league_tabs(&self) -> Result<serde_json::Value> {
        self.get_json(reqwest::Url::parse(LEAGUE_TAB_URL)?, "league tab")
            .await
    }

    pub async fn get_match_list_v5(&self, league_id: &str) -> Result<serde_json::Value> {
        let league_id = if league_id.trim().is_empty() {
            "0"
        } else {
            league_id.trim()
        };
        let url = reqwest::Url::parse_with_params(API_LIST_V5_URL, [("leagueId", league_id)])?;

        self.get_json(url, "match list v5").await
    }

    pub async fn get_match_list_v3(
        &self,
        status: i32,
        match_type: &str,
        start_time: &str,
        end_time: &str,
    ) -> Result<serde_json::Value> {
        let status_string = status.to_string();
        let match_type = if match_type.trim().is_empty() {
            "football"
        } else {
            match_type.trim()
        };
        let url = reqwest::Url::parse_with_params(
            API_LIST_V3_URL,
            [
                ("status", status_string.as_str()),
                ("matchType", match_type),
                ("startTime", start_time),
                ("endTime", end_time),
            ],
        )?;

        self.get_json(url, "match list v3").await
    }

    pub async fn get_aggregate(&self, league_id: &str) -> Result<serde_json::Value> {
        let league_id = if league_id.trim().is_empty() {
            "0"
        } else {
            league_id.trim()
        };
        let url = reqwest::Url::parse_with_params(API_AGGREGATE_URL, [("leagueId", league_id)])?;

        self.get_json(url, "aggregate").await
    }

    async fn get_matches(&self) -> Result<Vec<Match>> {
        let resp = self
            .client
            .get(API_LIST_URL)
            .header("User-Agent", USER_AGENT)
            .header("Accept", "application/json")
            .header("Content-Type", "application/json")
            .header("Origin", ORIGIN)
            .header("Referer", REFERER)
            .header("x-client-info", X_CLIENT_INFO)
            .header("x-device-info", X_DEVICE_INFO)
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

    pub async fn get_latest_matches(
        &self,
        match_type: &str,
        count: u32,
    ) -> Result<serde_json::Value> {
        let count = count.clamp(1, 50);
        let match_type = if match_type.trim().is_empty() {
            "MTFootball"
        } else {
            match_type.trim()
        };
        let count_string = count.to_string();
        let url = reqwest::Url::parse_with_params(
            API_LATEST_URL,
            [("matchType", match_type), ("count", count_string.as_str())],
        )?;

        self.get_json(url, "latest").await
    }

    async fn get_match_detail(&self, match_id: &str) -> Result<Option<Match>> {
        let url = format!("{}?matchId={}", API_DETAIL_URL, match_id);
        let resp = self
            .client
            .get(&url)
            .header("User-Agent", USER_AGENT)
            .header("Accept", "application/json")
            .header("Content-Type", "application/json")
            .header("Origin", ORIGIN)
            .header("Referer", REFERER)
            .header("x-client-info", X_CLIENT_INFO)
            .header("x-device-info", X_DEVICE_INFO)
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
        let grouped = self.search_streams_grouped(query).await?;
        let mut streams = Vec::new();
        streams.extend(grouped.live);
        streams.extend(grouped.replay);
        streams.extend(grouped.highlights);
        Ok(streams)
    }

    pub async fn search_streams_grouped(
        &self,
        query: &str,
    ) -> Result<GroupedSportsStreams> {
        let clean_query = query.to_lowercase().replace("-", " ");
        info!("[AiSports] Searching for: {}", query);

        if query.chars().all(|c| c.is_ascii_digit()) && query.len() >= 15 {
            let Some(active) = self.get_match_detail(query).await? else {
                info!("[AiSports] No match detail found for '{}'", query);
                return Ok(GroupedSportsStreams {
                    live: vec![],
                    replay: vec![],
                    highlights: vec![],
                });
            };

            return Ok(Self::grouped_streams_from_match(&active));
        }

        let matches = self.get_matches().await?;
        if matches.is_empty() {
            warn!("[AiSports] No matches returned from API");
            return Ok(GroupedSportsStreams {
                live: vec![],
                replay: vec![],
                highlights: vec![],
            });
        }

        let best_match = matches.iter().find(|m| {
            let title = format!(
                "{} vs {}",
                m.team1
                    .as_ref()
                    .and_then(|t| t.name.clone())
                    .unwrap_or_default(),
                m.team2
                    .as_ref()
                    .and_then(|t| t.name.clone())
                    .unwrap_or_default()
            )
            .to_lowercase();
            let team1 = m
                .team1
                .as_ref()
                .and_then(|t| t.name.clone())
                .unwrap_or_default()
                .to_lowercase();
            let team2 = m
                .team2
                .as_ref()
                .and_then(|t| t.name.clone())
                .unwrap_or_default()
                .to_lowercase();

            title.contains(&clean_query)
                || clean_query.contains(&team1)
                || clean_query.contains(&team2)
                || m.id.to_lowercase().contains(query)
        });

        let m = match best_match {
            Some(m) => m.clone(),
            None => {
                info!("[AiSports] No match found for '{}'", query);
                return Ok(GroupedSportsStreams {
                    live: vec![],
                    replay: vec![],
                    highlights: vec![],
                });
            }
        };

        info!(
            "[AiSports] Found match: {} vs {} (ID: {})",
            m.team1
                .as_ref()
                .and_then(|t| t.name.clone())
                .unwrap_or_default(),
            m.team2
                .as_ref()
                .and_then(|t| t.name.clone())
                .unwrap_or_default(),
            m.id
        );

        // Fetch detail for more sources
        let detail = self.get_match_detail(&m.id).await?;
        let active = detail.as_ref().unwrap_or(&m);

        Ok(Self::grouped_streams_from_match(active))
    }

    pub async fn match_detail(&self, match_id: &str) -> Result<Option<Match>> {
        // Try the dedicated detail endpoint first.
        if let Ok(Some(m)) = self.get_match_detail(match_id).await {
            return Ok(Some(m));
        }

        // detail-v1 returns null match_info for many live/scheduled matches.
        // Fall back: scan all available match lists for the ID.  The grouped
        // endpoint already embeds playPath/playSource/replay/highlights so
        // the player gets exactly what it needs.
        let today = chrono::Utc::now();
        let start = (today - chrono::Duration::days(1))
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        let end = (today + chrono::Duration::days(3))
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();

        let scan_targets: &[(&str, &str)] = &[
            ("football", "0"),
            ("football", "1"),
            ("football", "2"),
            ("basketball", "0"),
            ("cricket", "0"),
        ];

        for (match_type, status) in scan_targets {
            let status_int: i32 = status.parse().unwrap_or(0);
            match self
                .get_match_list_v3(status_int, match_type, &start, &end)
                .await
            {
                Ok(json) => {
                    let list = json
                        .get("list")
                        .and_then(|l| l.as_array())
                        .cloned()
                        .unwrap_or_default();
                    for item in list {
                        if item.get("id").and_then(|v| v.as_str()) == Some(match_id) {
                            if let Ok(m) = serde_json::from_value::<Match>(item) {
                                info!("[AiSports] match_detail fallback found {} in {}/{}", match_id, match_type, status);
                                return Ok(Some(m));
                            }
                        }
                    }
                }
                Err(e) => {
                    warn!("[AiSports] match_detail fallback list error ({}/{}): {}", match_type, status, e);
                }
            }
        }

        Ok(None)
    }

    fn unwrap_m3u8_url(path: &str) -> String {
        let lower = path.to_lowercase();
        if !lower.contains("88player") || !lower.contains("m3u8.html") {
            return path.to_string();
        }
        if let Ok(url) = reqwest::Url::parse(path) {
            if let Some(inner) = url.query_pairs().find(|(k, _)| k == "url") {
                let inner = inner.1.into_owned();
                if inner.to_lowercase().contains("m3u8.html") {
                    return Self::unwrap_m3u8_url(&inner);
                }
                return inner;
            }
        }
        path.to_string()
    }

    pub fn grouped_streams_from_match(active: &Match) -> GroupedSportsStreams {
        let team1_name = active
            .team1
            .as_ref()
            .and_then(|t| t.name.clone())
            .unwrap_or_else(|| "Unknown".to_string());
        let team2_name = active
            .team2
            .as_ref()
            .and_then(|t| t.name.clone())
            .unwrap_or_else(|| "Unknown".to_string());

        let headers = {
            let mut h = std::collections::HashMap::new();
            h.insert("Referer".to_string(), REFERER.to_string());
            h.insert("Origin".to_string(), ORIGIN.to_string());
            h.insert("User-Agent".to_string(), USER_AGENT.to_string());
            h.insert("Accept".to_string(), "application/json".to_string());
            h.insert("x-client-info".to_string(), X_CLIENT_INFO.to_string());
            h.insert("x-device-info".to_string(), X_DEVICE_INFO.to_string());
            h
        };

        let mut live = Vec::new();

        // Main stream
        if let Some(path) = &active.playPath {
            if !path.is_empty() {
                live.push(SportsStream {
                    title: format!("{} vs {} (Main)", team1_name, team2_name),
                    url: Self::unwrap_m3u8_url(path),
                    quality: "Auto".to_string(),
                    provider: "Field".to_string(),
                    source: "aisports".to_string(),
                    is_live: true,
                    is_replay: None,
                    headers: Some(headers.clone()),
                });
            }
        }

        // Additional sources
        if let Some(sources) = &active.playSource {
            for (idx, source) in sources.iter().enumerate() {
                if let Some(path) = &source.path {
                    if !path.is_empty() && !path.to_lowercase().contains(BLOCKED_CHANNEL_HOST) {
                        live.push(SportsStream {
                            title: format!(
                                "{} vs {} ({})",
                                team1_name,
                                team2_name,
                                source
                                    .title
                                    .clone()
                                    .unwrap_or_else(|| format!("Channel {}", idx + 1))
                            ),
                            url: Self::unwrap_m3u8_url(path),
                            quality: "Auto".to_string(),
                            provider: "Field".to_string(),
                            source: "aisports".to_string(),
                            is_live: true,
                            is_replay: None,
                            headers: Some(headers.clone()),
                        });
                    }
                }
            }
        }

        let mut replay = Vec::new();
        if let Some(replays) = &active.replay {
            for r in replays {
                if let Some(path) = &r.path {
                    if !path.is_empty() {
                        replay.push(SportsStream {
                            title: format!(
                                "{} vs {} - {}",
                                team1_name,
                                team2_name,
                                r.title.clone().unwrap_or_else(|| "Replay".to_string())
                            ),
                            url: path.clone(),
                            quality: "720p".to_string(),
                            provider: "Stadium".to_string(),
                            source: "aisports-replay".to_string(),
                            is_live: false,
                            is_replay: Some(true),
                            headers: None,
                        });
                    }
                }
            }
        }

        let mut highlights = Vec::new();
        if let Some(items) = &active.highlights {
            for h in items {
                if let Some(path) = &h.path {
                    if !path.is_empty() {
                        highlights.push(SportsStream {
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
                            is_replay: None,
                            headers: None,
                        });
                    }
                }
            }
        }

        GroupedSportsStreams {
            live,
            replay,
            highlights,
        }
    }

    fn streams_from_match(active: &Match) -> Vec<SportsStream> {
        let grouped = Self::grouped_streams_from_match(active);
        let mut streams = Vec::new();
        streams.extend(grouped.live);
        streams.extend(grouped.replay);
        streams.extend(grouped.highlights);
        streams
    }
}
