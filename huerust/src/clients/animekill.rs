use anyhow::{anyhow, Result};
use regex::Regex;
use reqwest_middleware::ClientWithMiddleware;
use serde::Deserialize;

use crate::models::anime::{
    AnimeCard, AnimeDetail, AnimeHomeFeed, AnimeSearchResponse, EpisodeSummary, FilterParams,
    PaginatedResponse, ServerEntry, ServerList, StreamPayload,
};

const BASE_URL: &str = "https://animekill.com";
const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36";

#[derive(Clone)]
pub struct AnimeKillClient {
    client: ClientWithMiddleware,
}

impl AnimeKillClient {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .connect_timeout(std::time::Duration::from_secs(8))
            .user_agent(UA)
            .build()
            .unwrap();
        Self {
            client: crate::http_client::wrap_with_retry(client),
        }
    }

    /// Search AnimeKill catalog by keyword.
    pub async fn search(&self, keyword: &str) -> Result<AnimeSearchResponse> {
        let url = format!(
            "{}/api/data/search?q={}",
            BASE_URL,
            urlencoding::encode(keyword)
        );
        let resp = self.client.get(&url).send().await?;
        let payload: SearchApiResponse = resp.json().await?;

        let results: Vec<AnimeCard> = payload
            .data
            .results
            .into_iter()
            .filter_map(api_result_to_card)
            .collect();

        let total = results.len();
        Ok(AnimeSearchResponse {
            query: keyword.to_string(),
            total,
            page: 1,
            limit: total as u32,
            results,
        })
    }

    /// Scrape a series detail page for metadata + episode list.
    pub async fn get_detail(&self, anime_id: &str) -> Result<AnimeDetail> {
        let html = self.fetch_series_html(anime_id).await?;
        let anime = extract_anime_payload(&html, anime_id)?;

        let episodes: Vec<EpisodeSummary> = anime
            .episode_list
            .as_ref()
            .map(|list| {
                list.iter()
                    .map(|ep| EpisodeSummary {
                        id: ep.episode_id.clone(),
                        number: ep.number as u32,
                        title: Some(ep.title.clone()),
                        sub_count: 1,
                        dub_count: 0,
                        slug: Some(ep.episode_id.clone()),
                        series_slug: Some(anime.anime_id.clone()),
                        slugs: None,
                    })
                    .collect()
            })
            .unwrap_or_default();

        let total_eps = if episodes.is_empty() {
            None
        } else {
            Some(episodes.len() as u32)
        };

        let genres = anime.genres.as_ref().map(|g| g.clone());

        let card = AnimeCard {
            id: anime.anime_id.clone(),
            title: clean_title(&anime.name),
            japanese_title: None,
            synopsis: anime.description.clone(),
            image: absolute_url(&anime.small_poster),
            banner_image: absolute_url(&anime.large_poster),
            anime_type: Some(anime.anime_type.clone()),
            status: anime.status.clone(),
            score: Some(anime.score.clone()),
            rating: None,
            genres,
            total_episodes: total_eps,
            total_subbed: total_eps,
            total_dubbed: None,
            slug: Some(anime.anime_id.clone()),
            mal_id: None,
            view_count: anime.total_watch.map(|n| n as u32),
            aired: None,
            duration: None,
            country: Some(country_from_origin(&anime.anime_origin)),
            source: Some("animekill".to_string()),
            has_sub: true,
            has_dub: false,
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

    /// Return the episode list for a series. Reuses the detail page payload.
    pub async fn get_episodes(&self, anime_id: &str) -> Result<Vec<EpisodeSummary>> {
        let detail = self.get_detail(anime_id).await?;
        Ok(detail.episodes.unwrap_or_default())
    }

    /// Scrape an episode page for available servers and stream info.
    pub async fn get_servers(&self, episode_id: &str) -> Result<ServerList> {
        let html = self.fetch_episode_html(episode_id).await?;
        let episode = extract_episode_payload(&html, episode_id)?;

        let mut sub = Vec::new();
        if let (Some(host), Some(path)) = (episode.video_host, episode.video_path) {
            let url = format!("https://{}{}", host, path);
            sub.push(ServerEntry {
                id: url.clone(),
                name: extract_server_name_from_url(&url),
                audio_type: "sub".to_string(),
                format: "iframe".to_string(),
            });
        }

        Ok(ServerList { sub, dub: vec![] })
    }

    /// Return the first available stream as a StreamPayload.
    pub async fn get_stream(
        &self,
        episode_id: &str,
        _server: Option<&str>,
    ) -> Result<StreamPayload> {
        let servers = self.get_servers(episode_id).await?;
        let target = servers
            .sub
            .first()
            .ok_or_else(|| anyhow!("No stream sources found"))?;

        Ok(StreamPayload {
            url: target.id.clone(),
            stream_type: "iframe".to_string(),
            subtitles: None,
            intro: None,
            outro: None,
            server: Some(target.name.clone()),
            referer: Some(BASE_URL.to_string()),
        })
    }

    /// Build a homepage-style feed.
    /// Maps:
    ///   trending → popular
    ///   latest   → latest
    ///   popular  → popular
    pub async fn get_home(&self) -> Result<AnimeHomeFeed> {
        let popular: Vec<AnimeCard> = self
            .api_list("popular", 1, 12, None)
            .await?
            .into_iter()
            .filter_map(api_result_to_card)
            .collect();
        let latest: Vec<AnimeCard> = self
            .api_list("latest", 1, 12, None)
            .await?
            .into_iter()
            .filter_map(api_result_to_card)
            .collect();

        Ok(AnimeHomeFeed {
            trending: Some(popular.clone()),
            currently_airing: None,
            featured: None,
            latest: Some(latest),
            popular: Some(popular),
            finished_airing: None,
        })
    }

    /// Discover / browse the catalog.
    pub async fn discover(&self, params: &FilterParams) -> Result<PaginatedResponse<AnimeCard>> {
        let page = params.page.unwrap_or(1).max(1) as usize;
        let limit = params.limit.unwrap_or(24).clamp(1, 100) as usize;

        let (category, letter) = match params.sort.as_deref() {
            Some("popularity") | Some("trending") => ("popular", None),
            Some("latest") | Some("added") | Some("update") => ("latest", None),
            Some("alphabetical") | Some("az") | Some("a-z") => {
                ("a-z", params.genre.as_deref().map(|g| g.to_lowercase()))
            }
            _ => ("a-z", None),
        };

        let results: Vec<ApiResult> = self.api_list(category, page, limit, letter.as_deref()).await?;
        let has_more = results.len() >= limit;

        let items: Vec<AnimeCard> = results.into_iter().filter_map(api_result_to_card).collect();
        let total = items.len() as u32;

        Ok(PaginatedResponse {
            items,
            total,
            page: page as u32,
            limit: limit as u32,
            has_more,
        })
    }

    async fn api_list(
        &self,
        category: &str,
        page: usize,
        limit: usize,
        letter: Option<&str>,
    ) -> Result<Vec<ApiResult>> {
        let mut url = format!(
            "{}/api/data/anime-list?category={}&page={}&limit={}",
            BASE_URL, category, page, limit
        );
        if let Some(l) = letter {
            url.push_str("&letter=");
            url.push_str(&urlencoding::encode(l));
        }

        let resp = self.client.get(&url).send().await?;
        let payload: ListApiResponse = resp.json().await?;
        Ok(payload.data.results)
    }

    async fn fetch_series_html(&self, anime_id: &str) -> Result<String> {
        let url = format!("{}/anime/{}", BASE_URL, anime_id.trim_end_matches('/'));
        let resp = self.client.get(&url).send().await?;
        Ok(resp.text().await?)
    }

    async fn fetch_episode_html(&self, episode_id: &str) -> Result<String> {
        let url = format!(
            "{}/{}/",
            BASE_URL,
            episode_id.trim_start_matches('/').trim_end_matches('/')
        );
        let resp = self.client.get(&url).send().await?;
        Ok(resp.text().await?)
    }
}

// ── API response types ──────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct SearchApiResponse {
    data: SearchData,
}

#[derive(Debug, Deserialize)]
struct SearchData {
    #[serde(default)]
    results: Vec<ApiResult>,
}

#[derive(Debug, Deserialize)]
struct ListApiResponse {
    data: ListData,
}

#[derive(Debug, Deserialize)]
struct ListData {
    #[serde(default)]
    results: Vec<ApiResult>,
    #[serde(default)]
    total: u32,
    #[serde(default)]
    page: u32,
    #[serde(default)]
    limit: u32,
    #[serde(default)]
    total_pages: u32,
    #[serde(default)]
    has_next: bool,
    #[serde(default)]
    has_prev: bool,
}

#[derive(Debug, Deserialize)]
struct ApiResult {
    anime_id: String,
    #[serde(default)]
    slug: String,
    title: String,
    #[serde(default)]
    small_poster: String,
    #[serde(default)]
    large_poster: String,
    #[serde(default)]
    poster_proxy: String,
    #[serde(default)]
    anime_type: String,
    #[serde(default)]
    anime_origin: String,
    #[serde(default)]
    genre: String,
    #[serde(default)]
    score: String,
    #[serde(default)]
    total_watch: Option<usize>,
    #[serde(default)]
    episode_url: Option<String>,
    #[serde(default)]
    episode_number: Option<u32>,
}

fn api_result_to_card(r: ApiResult) -> Option<AnimeCard> {
    let id = if r.anime_id.is_empty() {
        r.slug.clone()
    } else {
        r.anime_id.clone()
    };
    if id.is_empty() {
        return None;
    }
    let image = if !r.poster_proxy.is_empty() {
        Some(r.poster_proxy.clone())
    } else if !r.small_poster.is_empty() {
        Some(r.small_poster.clone())
    } else {
        None
    };
    let genres = if r.genre.is_empty() {
        None
    } else {
        Some(r.genre.split(',').map(|s| s.trim().to_string()).collect())
    };

    Some(AnimeCard {
        id: id.clone(),
        title: clean_title(&r.title),
        japanese_title: None,
        synopsis: None,
        image: image.as_deref().map(absolute_url).flatten(),
        banner_image: image.as_deref().map(absolute_url).flatten(),
        anime_type: Some(r.anime_type.clone()),
        status: None,
        score: Some(r.score.clone()),
        rating: None,
        genres,
        total_episodes: r.episode_number,
        total_subbed: r.episode_number,
        total_dubbed: None,
        slug: Some(id.clone()),
        mal_id: None,
        view_count: r.total_watch.map(|n| n as u32),
        aired: None,
        duration: None,
        country: Some(country_from_origin(&r.anime_origin)),
        source: Some("animekill".to_string()),
        has_sub: true,
        has_dub: false,
    })
}

// ── SSR HTML payload parsing ────────────────────────────────────────────────

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct SsrAnime {
    anime_id: String,
    name: String,
    #[serde(alias = "description")]
    description: Option<String>,
    #[serde(alias = "small_poster")]
    small_poster: String,
    #[serde(alias = "large_poster")]
    large_poster: String,
    #[serde(alias = "orginal_poster")]
    original_poster: Option<String>,
    #[serde(alias = "anime_type")]
    anime_type: String,
    #[serde(alias = "anime_origin")]
    anime_origin: String,
    status: Option<String>,
    score: String,
    total_watch: Option<usize>,
    #[serde(alias = "genre")]
    genre_string: Option<String>,
    genres: Option<Vec<String>>,
    #[serde(alias = "episodeList")]
    episode_list: Option<Vec<SsrEpisode>>,
    #[serde(alias = "max_episode_number")]
    max_episode_number: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct SsrEpisode {
    #[serde(alias = "episode_id")]
    episode_id: String,
    number: f64,
    title: String,
    #[serde(default)]
    thumbnail: String,
    url: String,
    #[serde(alias = "video_host")]
    video_host: Option<String>,
    #[serde(alias = "video_path")]
    video_path: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct SsrCurrentEpisode {
    #[serde(alias = "episode_id")]
    episode_id: String,
    number: f64,
    title: String,
    #[serde(alias = "video_host")]
    video_host: Option<String>,
    #[serde(alias = "video_path")]
    video_path: Option<String>,
}

fn extract_anime_payload(html: &str, anime_id: &str) -> Result<SsrAnime> {
    // AnimeKill renders the detail inside a Next.js "flight" payload keyed by
    // "25:[$,\"$L2c\",null,{\"anime\":{...}}]". The value at index 3 is the
    // props object containing the anime. Grab the whole flight line and parse it
    // as JSON, then extract the anime field.
    let re = Regex::new(
        r#"25:\\["\$","\$L2c",null,\{\"anime\":\{.*?\}\}\]"#,
    )?;

    for cap in re.captures_iter(html) {
        if let Some(m) = cap.get(0) {
            if let Some(props) = parse_flight_props(m.as_str()) {
                if props.anime_id == anime_id {
                    return Ok(props);
                }
            }
        }
    }

    // Fallback: search any flight line containing this anime_id and try to parse.
    let fallback_re = Regex::new(
        &format!(
            r#"\{{[^}}]*\"anime_id\":\"{}\"[^}}]*\}}"#,
            regex::escape(anime_id)
        ),
    )?;
    for cap in fallback_re.captures_iter(html) {
        if let Some(m) = cap.get(0) {
            let extended = extend_json_object(html, m.start(), m.end());
            if let Ok(anime) = serde_json::from_str::<SsrAnime>(&extended) {
                return Ok(anime);
            }
        }
    }

    Err(anyhow!("AnimeKill detail payload not found for {}", anime_id))
}

fn extract_episode_payload(html: &str, episode_id: &str) -> Result<SsrCurrentEpisode> {
    let series_id = series_id_from_episode(episode_id);
    if let Ok(anime) = extract_anime_payload(html, &series_id) {
        if let Some(list) = anime.episode_list {
            if let Some(ep) = list.iter().find(|e| e.episode_id == episode_id) {
                return Ok(SsrCurrentEpisode {
                    episode_id: ep.episode_id.clone(),
                    number: ep.number,
                    title: ep.title.clone(),
                    video_host: ep.video_host.clone(),
                    video_path: ep.video_path.clone(),
                });
            }
        }
    }

    // Fallback: look for a self-contained object containing this episode_id with video_host.
    let re = Regex::new(&format!(
        r#"\"episode_id\":\"{}\"[^}}]*\"video_host\":\"([^\"]+)\"[^}}]*\"video_path\":\"([^\"]+)\""#,
        regex::escape(episode_id)
    ))?;
    if let Some(cap) = re.captures(html) {
        if let Some(m) = cap.get(0) {
            let extended = extend_json_object(html, m.start(), m.end());
            if let Ok(ep) = serde_json::from_str::<SsrCurrentEpisode>(&extended) {
                return Ok(ep);
            }
        }
    }

    Err(anyhow!(
        "AnimeKill episode payload not found for {}",
        episode_id
    ))
}

/// Parse a Next.js flight line of shape `25:["$","$L2c",null,{...}]` and
/// return the props object at index 3.
fn parse_flight_props(line: &str) -> Option<SsrAnime> {
    // Strip the leading "25:" so we can parse the value as JSON.
    let json_start = line.find(":")? + 1;
    let value_json = &line[json_start..];
    let arr: serde_json::Value = serde_json::from_str(value_json).ok()?;
    let props = arr.get(3)?;
    serde_json::from_value(props.clone()).ok()
}

fn series_id_from_episode(episode_id: &str) -> String {
    episode_id
        .trim_start_matches('/')
        .trim_end_matches('/')
        .trim_end_matches("-episode")
        .trim_end_matches("-episode-")
        .split("-episode-")
        .next()
        .unwrap_or(episode_id)
        .to_string()
}

/// Try to capture a balanced JSON object around [start, end].
fn extend_json_object(html: &str, start: usize, end: usize) -> String {
    let mut depth = 0i32;
    let bytes = html.as_bytes();

    let mut obj_start = start;
    for i in (0..=start).rev() {
        match bytes.get(i) {
            Some(b'{') => {
                if depth == 0 {
                    obj_start = i;
                    break;
                }
            }
            Some(b'}') => depth += 1,
            _ => {}
        }
    }

    let mut depth = 0i32;
    let mut obj_end = end.min(html.len());
    for i in end..html.len() {
        match bytes.get(i) {
            Some(b'{') => depth += 1,
            Some(b'}') => {
                if depth == 0 {
                    obj_end = i + 1;
                    break;
                }
            }
            _ => {}
        }
    }

    html[obj_start..obj_end].to_string()
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn absolute_url(path: &str) -> Option<String> {
    if path.is_empty() {
        return None;
    }
    if path.starts_with("http://") || path.starts_with("https://") {
        Some(path.to_string())
    } else {
        Some(format!("{}{}", BASE_URL, path))
    }
}

fn country_from_origin(origin: &str) -> String {
    match origin.to_lowercase().as_str() {
        "chinese" | "cn" | "china" => "CN".into(),
        "japanese" | "jp" | "japan" => "JP".into(),
        "korean" | "kr" | "korea" => "KR".into(),
        _ => "CN".into(),
    }
}

fn clean_title(raw: &str) -> String {
    raw.trim().to_string()
}

fn extract_server_name_from_url(url: &str) -> String {
    let host = url
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_start_matches("//")
        .split('/')
        .next()
        .unwrap_or("Unknown");
    let name = host
        .trim_start_matches("www.")
        .split('.')
        .next()
        .unwrap_or("Unknown");
    name.to_uppercase()
}
