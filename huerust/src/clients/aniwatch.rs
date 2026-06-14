use anyhow::{anyhow, Result};
use reqwest_middleware::ClientWithMiddleware;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;

use crate::constants::UA_DESKTOP;
use crate::models::anime::{
    AnimeCard, AnimeDetail, AnimeHomeFeed, AnimeSearchResponse, EpisodeSummary, FilterParams,
    ServerEntry, ServerList, SkipRange, StreamPayload, SubtitleTrack,
};

const BASE_URL: &str = "https://aniwatch.to";
const AJAX_URL: &str = "https://aniwatch.to/ajax";

#[derive(Clone)]
pub struct AniWatchClient {
    client: ClientWithMiddleware,
}

#[derive(Debug, Deserialize)]
struct SearchResult {
    #[serde(default)]
    id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    poster: String,
    #[serde(default)]
    #[serde(rename = "jname")]
    jname: String,
    #[serde(default)]
    #[serde(rename = "api_data")]
    api_data: Option<ApiData>,
}

#[derive(Debug, Deserialize)]
struct ApiData {
    #[serde(default)]
    #[serde(rename = "eps_count")]
    eps_count: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct SearchResponse {
    #[serde(default)]
    animes: Vec<SearchResult>,
}

#[derive(Debug, Deserialize)]
struct AnimeInfoResponse {
    #[serde(default)]
    id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    poster: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    #[serde(rename = "stats")]
    stats: Option<AnimeStats>,
    #[serde(default)]
    episodes: Vec<AniWatchEpisode>,
}

#[derive(Debug, Deserialize)]
struct AnimeStats {
    #[serde(default)]
    rating: String,
    #[serde(default)]
    quality: String,
    #[serde(default)]
    #[serde(rename = "episodes")]
    eps: Option<AnimeStatEps>,
    #[serde(default)]
    #[serde(rename = "type")]
    type_: String,
    #[serde(default)]
    duration: String,
}

#[derive(Debug, Deserialize)]
struct AnimeStatEps {
    #[serde(default)]
    sub: Option<u32>,
    #[serde(default)]
    dub: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct AniWatchEpisode {
    #[serde(default)]
    id: String,
    #[serde(default)]
    number: u32,
    #[serde(default)]
    title: String,
    #[serde(default)]
    is_filler: bool,
}

#[derive(Debug, Deserialize)]
struct ServersResponse {
    #[serde(default)]
    sub: Vec<ServerItem>,
    #[serde(default)]
    dub: Vec<ServerItem>,
}

#[derive(Debug, Deserialize)]
struct ServerItem {
    #[serde(default)]
    server_name: String,
    #[serde(default)]
    server_id: u32,
}

#[derive(Debug, Deserialize)]
struct SourcesResponse {
    #[serde(default)]
    tracks: Vec<TrackItem>,
    #[serde(default)]
    intro: Option<SkipItem>,
    #[serde(default)]
    outro: Option<SkipItem>,
    #[serde(default)]
    sources: Vec<SourceItem>,
    #[serde(default)]
    #[serde(rename = "anilistID")]
    anilist_id: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct TrackItem {
    #[serde(default)]
    file: String,
    #[serde(default)]
    label: String,
    #[serde(default)]
    kind: String,
    #[serde(default)]
    #[serde(rename = "default")]
    is_default: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct SkipItem {
    start: u32,
    end: u32,
}

#[derive(Debug, Deserialize)]
struct SourceItem {
    #[serde(default)]
    #[serde(rename = "type")]
    type_: String,
    #[serde(default)]
    url: String,
}

impl AniWatchClient {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(20))
            .connect_timeout(std::time::Duration::from_secs(10))
            .tcp_keepalive(std::time::Duration::from_secs(60))
            .user_agent(UA_DESKTOP)
            .build()
            .unwrap();
        Self {
            client: crate::http_client::wrap_with_retry(client),
        }
    }

    pub async fn search(&self, keyword: &str) -> Result<AnimeSearchResponse> {
        let url = format!(
            "{}/search/suggest?keyword={}",
            AJAX_URL,
            urlencoding::encode(keyword)
        );
        let resp = self
            .client
            .get(&url)
            .header("Referer", BASE_URL)
            .header("X-Requested-With", "XMLHttpRequest")
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(anyhow!("AniWatch search returned {}", resp.status()));
        }

        let html = resp.text().await?;
        let results = parse_search_html(&html);

        Ok(AnimeSearchResponse {
            query: keyword.to_string(),
            total: results.len(),
            page: 1,
            limit: results.len() as u32,
            results,
        })
    }

    /// Browse / discover. AniWatch's `/filter` endpoint returns a page of
    /// anime cards filtered by category. Genre/status not wired in this
    /// client — AniList carries the more comprehensive filter data.
    pub async fn discover(&self, p: &FilterParams) -> Result<Vec<AnimeCard>> {
        let typ = p.r#type.as_deref().unwrap_or("tv");
        let category = match typ {
            "movie" => "movie",
            "ova" | "ona" | "special" => "special",
            _ => "tv",
        };
        let page = p.page.unwrap_or(1);
        let url = format!(
            "{}/filter?category={}&page={}",
            AJAX_URL, category, page
        );
        let resp = self
            .client
            .get(&url)
            .header("Referer", BASE_URL)
            .header("X-Requested-With", "XMLHttpRequest")
            .send()
            .await?;
        if !resp.status().is_success() {
            return Err(anyhow!("AniWatch filter returned {}", resp.status()));
        }
        let html = resp.text().await?;
        Ok(parse_search_html(&html))
    }

    pub async fn get_detail(&self, id: &str) -> Result<AnimeDetail> {
        let url = format!("{}/anime/info?id={}", BASE_URL, urlencoding::encode(id));
        let resp = self
            .client
            .get(&url)
            .header("Referer", BASE_URL)
            .header("X-Requested-With", "XMLHttpRequest")
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(anyhow!("AniWatch info returned {}", resp.status()));
        }

        let info: AnimeInfoResponse = resp.json().await?;

        let total_eps = info
            .stats
            .as_ref()
            .and_then(|s| s.eps.as_ref())
            .and_then(|e| e.sub)
            .unwrap_or(0);
        let has_dub = info
            .stats
            .as_ref()
            .and_then(|s| s.eps.as_ref())
            .and_then(|e| e.dub)
            .unwrap_or(0)
            > 0;

        let episodes: Vec<EpisodeSummary> = info
            .episodes
            .iter()
            .map(|ep| EpisodeSummary {
                id: ep.id.clone(),
                number: ep.number,
                title: if ep.title.is_empty() {
                    None
                } else {
                    Some(ep.title.clone())
                },
                sub_count: 1,
                dub_count: if has_dub { 1 } else { 0 },
                slug: Some(ep.id.clone()),
                series_slug: Some(info.id.clone()),
                slugs: None,
            })
            .collect();

        let card = AnimeCard {
            id: info.id.clone(),
            title: info.name.clone(),
            japanese_title: None,
            synopsis: if info.description.is_empty() {
                None
            } else {
                Some(info.description.clone())
            },
            image: if info.poster.is_empty() {
                None
            } else {
                Some(info.poster.clone())
            },
            banner_image: None,
            anime_type: info.stats.as_ref().map(|s| s.type_.clone()),
            status: None,
            score: info.stats.as_ref().map(|s| s.rating.clone()),
            rating: None,
            genres: None,
            total_episodes: if total_eps > 0 { Some(total_eps) } else { None },
            total_subbed: if total_eps > 0 { Some(total_eps) } else { None },
            total_dubbed: if has_dub { Some(total_eps) } else { None },
            slug: Some(info.id.clone()),
            mal_id: None,
            view_count: None,
            aired: None,
            duration: info.stats.as_ref().map(|s| s.duration.clone()),
            has_sub: total_eps > 0,
            has_dub,
        };

        Ok(AnimeDetail {
            card,
            broadcast: None,
            premiered: None,
            producers: None,
            licensors: None,
            source: None,
            members: None,
            favorites: None,
            popularity: None,
            ranked: None,
            episodes: Some(episodes),
        })
    }

    pub async fn get_servers(&self, episode_id: &str) -> Result<ServerList> {
        let url = format!(
            "{}/episode/servers?episodeId={}",
            BASE_URL,
            urlencoding::encode(episode_id)
        );
        let resp = self
            .client
            .get(&url)
            .header("Referer", BASE_URL)
            .header("X-Requested-With", "XMLHttpRequest")
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(anyhow!("AniWatch servers returned {}", resp.status()));
        }

        let data: ServersResponse = resp.json().await?;

        let sub = data
            .sub
            .iter()
            .map(|s| ServerEntry {
                id: s.server_id.to_string(),
                name: s.server_name.clone(),
                audio_type: "sub".to_string(),
                format: if s.server_name == "hd-1" || s.server_name == "hd-2" {
                    "json".to_string()
                } else {
                    "embed".to_string()
                },
            })
            .collect();

        let dub = data
            .dub
            .iter()
            .map(|s| ServerEntry {
                id: s.server_id.to_string(),
                name: s.server_name.clone(),
                audio_type: "dub".to_string(),
                format: if s.server_name == "hd-1" || s.server_name == "hd-2" {
                    "json".to_string()
                } else {
                    "embed".to_string()
                },
            })
            .collect();

        Ok(ServerList { sub, dub })
    }

    pub async fn get_stream(
        &self,
        episode_id: &str,
        server_name: &str,
        category: &str,
    ) -> Result<StreamPayload> {
        let url = format!(
            "{}/episode/sources?episodeId={}&server={}&category={}",
            BASE_URL,
            urlencoding::encode(episode_id),
            urlencoding::encode(server_name),
            urlencoding::encode(category)
        );
        let resp = self
            .client
            .get(&url)
            .header("Referer", BASE_URL)
            .header("X-Requested-With", "XMLHttpRequest")
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(anyhow!("AniWatch sources returned {}", resp.status()));
        }

        let data: SourcesResponse = resp.json().await?;

        let source = data
            .sources
            .first()
            .ok_or_else(|| anyhow!("No sources found"))?;

        let stream_type = if source.url.contains(".m3u8") || source.type_.to_lowercase() == "hls" {
            "hls"
        } else {
            "mp4"
        };

        let subtitles = if data.tracks.is_empty() {
            None
        } else {
            Some(
                data.tracks
                    .iter()
                    .filter(|t| t.kind == "captions")
                    .map(|t| SubtitleTrack {
                        file: t.file.clone(),
                        label: t.label.clone(),
                        kind: t.kind.clone(),
                        default: t.is_default,
                    })
                    .collect(),
            )
        };

        let intro = data.intro.map(|i| SkipRange {
            start: i.start,
            end: i.end,
        });
        let outro = data.outro.map(|o| SkipRange {
            start: o.start,
            end: o.end,
        });

        Ok(StreamPayload {
            url: source.url.clone(),
            stream_type: stream_type.to_string(),
            subtitles,
            intro,
            outro,
            server: Some(server_name.to_string()),
            referer: Some(BASE_URL.to_string()),
        })
    }

    pub async fn get_home(&self) -> Result<AnimeHomeFeed> {
        let url = format!("{}/home", BASE_URL);
        let resp = self
            .client
            .get(&url)
            .header("Referer", BASE_URL)
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(anyhow!("AniWatch home returned {}", resp.status()));
        }

        let html = resp.text().await?;

        let trending = extract_section_cards(&html, "Trending");
        let latest = extract_section_cards(&html, "Latest");
        let popular = extract_section_cards(&html, "Popular");

        Ok(AnimeHomeFeed {
            trending,
            currently_airing: None,
            featured: None,
            latest,
            popular,
            finished_airing: None,
        })
    }
}

fn parse_search_html(html: &str) -> Vec<AnimeCard> {
    // AniWatch search suggest returns HTML with anime items
    // Parse the HTML to extract anime entries
    let mut results = Vec::new();

    // Simple regex-like parsing for film-poster items
    let re =
        regex::Regex::new(r#"film-pa[^"]*"[^>]*data-id="([^"]+)"[^>]*film-name[^>]*>([^<]+)"#).ok();
    if let Some(re) = re {
        for cap in re.captures_iter(html) {
            let id = cap
                .get(1)
                .map(|m| m.as_str().to_string())
                .unwrap_or_default();
            let title = cap
                .get(2)
                .map(|m| m.as_str().to_string())
                .unwrap_or_default();
            if !id.is_empty() && !title.is_empty() {
                results.push(AnimeCard {
                    id: id.clone(),
                    title,
                    japanese_title: None,
                    synopsis: None,
                    image: None,
                    banner_image: None,
                    anime_type: None,
                    status: None,
                    score: None,
                    rating: None,
                    genres: None,
                    total_episodes: None,
                    total_subbed: None,
                    total_dubbed: None,
                    slug: Some(id),
                    mal_id: None,
                    view_count: None,
                    aired: None,
                    duration: None,
                    has_sub: false,
                    has_dub: false,
                });
            }
        }
    }

    results
}

fn extract_section_cards(html: &str, _section: &str) -> Option<Vec<AnimeCard>> {
    // Extract anime cards from a section of the home page HTML
    let mut cards = Vec::new();

    let re =
        regex::Regex::new(r#"film-pa[^"]*"[^>]*data-id="([^"]+)"[^>]*film-name[^>]*>([^<]+)"#).ok();
    if let Some(re) = re {
        for cap in re.captures_iter(html) {
            let id = cap
                .get(1)
                .map(|m| m.as_str().to_string())
                .unwrap_or_default();
            let title = cap
                .get(2)
                .map(|m| m.as_str().to_string())
                .unwrap_or_default();
            if !id.is_empty() && !title.is_empty() {
                cards.push(AnimeCard {
                    id: id.clone(),
                    title,
                    japanese_title: None,
                    synopsis: None,
                    image: None,
                    banner_image: None,
                    anime_type: None,
                    status: None,
                    score: None,
                    rating: None,
                    genres: None,
                    total_episodes: None,
                    total_subbed: None,
                    total_dubbed: None,
                    slug: Some(id),
                    mal_id: None,
                    view_count: None,
                    aired: None,
                    duration: None,
                    has_sub: false,
                    has_dub: false,
                });
            }
        }
    }

    if cards.is_empty() {
        None
    } else {
        Some(cards)
    }
}
