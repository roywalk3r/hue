use anyhow::{anyhow, Result};
use regex::Regex;
use reqwest_middleware::ClientWithMiddleware;
use serde::Deserialize;
use tracing::debug;

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
    /// AnimeKill exposes two APIs:
    ///   /api/data/videos/{anime_id}/{episode_number} — primary (often dead hosters like gotaku1)
    ///   /api/data/parser/{anime_id}/{episode_number} — fallback with multiple live mirrors
    /// We use the parser API as the authoritative source because it returns Dailymotion,
    /// YouTube, OK.ru and other embeds that actually resolve.
    pub async fn get_servers(&self, episode_id: &str) -> Result<ServerList> {
        let (canonical, number) = canonical_episode_id(episode_id);
        let series_id = series_id_from_episode(&canonical);

        let url = format!("{}/api/data/parser/{}/{}", BASE_URL, series_id, number);
        let resp = self.client.get(&url).send().await?;
        let status = resp.status();
        let body = resp.text().await?;
        debug!(
            "[AnimeKill] parser status={} len={} id={}",
            status,
            body.len(),
            canonical
        );

        let payload: ParserApiResponse = serde_json::from_str(&body)
            .map_err(|e| anyhow!("AnimeKill parser response invalid: {}", e))?;

        let mut sub = Vec::new();
        for item in payload.data {
            if item.decode_url.is_empty() {
                continue;
            }
            sub.push(ServerEntry {
                id: item.decode_url.clone(),
                name: item.server_name.clone(),
                audio_type: "sub".to_string(),
                format: "iframe".to_string(),
            });
        }

        Ok(ServerList { sub, dub: vec![] })
    }

    /// Return the best available stream as a StreamPayload.
    /// Prefers well-known reliable players (Dailymotion, YouTube, OK.ru) over
    /// obscure hosters that frequently go offline.
    pub async fn get_stream(
        &self,
        episode_id: &str,
        server: Option<&str>,
    ) -> Result<StreamPayload> {
        let servers = self.get_servers(episode_id).await?;
        let pool = servers.sub;
        if pool.is_empty() {
            return Err(anyhow!("No stream sources found"));
        }

        let target = if let Some(srv) = server {
            pool.iter()
                .find(|s| s.name.eq_ignore_ascii_case(srv))
                .or_else(|| pool.first())
                .cloned()
        } else {
            pool.iter()
                .max_by_key(|s| server_reliability_score(&s.id))
                .cloned()
                .or_else(|| pool.first().cloned())
        }
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
        let status = resp.status();
        let html = resp.text().await?;
        debug!(
            "[AnimeKill] fetch_series_html status={} len={} id={}",
            status,
            html.len(),
            anime_id
        );
        if is_animekill_not_found(&html) {
            return Err(anyhow!("AnimeKill series page not found ({})", anime_id));
        }
        Ok(html)
    }

    async fn fetch_episode_html(&self, episode_id: &str) -> Result<String> {
        let (canonical, _) = canonical_episode_id(episode_id);
        let url = format!(
            "{}/{}",
            BASE_URL,
            canonical.trim_start_matches('/').trim_end_matches('/')
        );
        let resp = self.client.get(&url).send().await?;
        let status = resp.status();
        let html = resp.text().await?;
        debug!(
            "[AnimeKill] fetch_episode_html status={} len={} id={}",
            status,
            html.len(),
            canonical
        );
        if is_animekill_not_found(&html) {
            return Err(anyhow!(
                "AnimeKill episode page not found ({})",
                canonical
            ));
        }
        Ok(html)
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
struct ParserApiResponse {
    #[serde(default)]
    data: Vec<ParserResult>,
}

#[derive(Debug, Deserialize)]
struct ParserResult {
    #[serde(default, alias = "serverName")]
    server_name: String,
    #[serde(default, alias = "decodeUrl")]
    decode_url: String,
    #[serde(default)]
    host: String,
    #[serde(default)]
    path: String,
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
    // AnimeKill detail pages return 200 even for missing slugs, but render a
    // 404 shell. Bail out early so we don't return stale/error data.
    if is_animekill_not_found(html) {
        return Err(anyhow!("AnimeKill page not found for {}", anime_id));
    }

    // AnimeKill serializes the page as a JS string passed to
    // self.__next_f.push([1, "<flight payload>"). Find the payload chunk that
    // contains this anime_id and extract the anime object from the flight line.
    if let Some(anime) = find_anime_in_next_f(html, anime_id) {
        if anime.anime_id == anime_id {
            return Ok(anime);
        }
    }

    // Fallback: search any self-contained JSON object containing this anime_id.
    let fallback_re = Regex::new(
        &format!(
            r#"\"anime_id\":\"{}\"[^}}]*\"name\":\"[^\"]+\""#,
            regex::escape(anime_id)
        ),
    )?;
    if let Some(m) = fallback_re.find(html) {
        let extended = extend_json_object(html, m.start(), m.end());
        if let Ok(anime) = serde_json::from_str::<SsrAnime>(&extended) {
            if anime.anime_id == anime_id {
                return Ok(anime);
            }
        }
    }

    Err(anyhow!("AnimeKill detail payload not found for {}", anime_id))
}

fn is_animekill_not_found(html: &str) -> bool {
    html.contains(r#"<title>404"#)
        || html.to_lowercase().contains(r#"<title>not found"#)
}

/// Locate the __next_f.push payload chunk containing `anime_id` and parse the
/// flight component whose props object holds the anime data.
fn find_anime_in_next_f(html: &str, anime_id: &str) -> Option<SsrAnime> {
    let needle = format!(r#"\"anime_id\":\"{}\""#, anime_id);
    let mut search_start = 0;
    while let Some(pos) = html[search_start..].find(&needle) {
        let abs_pos = search_start + pos;

        let push_start = html[..abs_pos].rfind(r#"self.__next_f.push([1,""#)?;
        let literal_start = push_start + r#"self.__next_f.push([1,""#.len();

        let mut escaped = false;
        let mut literal_end = None;
        for (idx, ch) in html[literal_start..].char_indices() {
            if escaped {
                escaped = false;
                continue;
            }
            if ch == '\\' {
                escaped = true;
                continue;
            }
            if ch == '"' {
                literal_end = Some(literal_start + idx);
                break;
            }
        }
        let end = literal_end?;

        let flight_text = unescape_js_string(&html[literal_start..end]);
        debug!("[AnimeKill] flight payload len={}", flight_text.len());
        if let Some(value_json) = first_json_value(&flight_text) {
            if let Ok(root) = serde_json::from_str::<serde_json::Value>(value_json) {
                if let Some(props) = find_anime_props(&root, anime_id) {
                    if let Some(anime_value) = props.get("anime") {
                        if let Ok(anime) = serde_json::from_value::<SsrAnime>(anime_value.clone()) {
                            if anime.anime_id == anime_id {
                                return Some(anime);
                            }
                        }
                    }
                }
            }
        }

        search_start = abs_pos + 1;
    }
    None
}

/// Recursively locate the flight props object where `anime.anime_id` matches.
fn find_anime_props(value: &serde_json::Value, anime_id: &str) -> Option<serde_json::Value> {
    match value {
        serde_json::Value::Array(arr) if arr.len() == 4 => {
            if let Some(props) = arr.get(3) {
                if props.get("anime")
                    .and_then(|a| a.get("anime_id"))
                    .and_then(|v| v.as_str())
                    == Some(anime_id)
                {
                    return Some(props.clone());
                }
            }
            for v in arr.iter() {
                if let Some(found) = find_anime_props(v, anime_id) {
                    return Some(found);
                }
            }
            None
        }
        serde_json::Value::Array(arr) => {
            for v in arr.iter() {
                if let Some(found) = find_anime_props(v, anime_id) {
                    return Some(found);
                }
            }
            None
        }
        serde_json::Value::Object(obj) => {
            for v in obj.values() {
                if let Some(found) = find_anime_props(v, anime_id) {
                    return Some(found);
                }
            }
            None
        }
        _ => None,
    }
}

/// Skip an optional flight key (e.g. "25:") and return the JSON value substring
/// that begins at the first '[' or '{'.
fn first_json_value(text: &str) -> Option<&str> {
    let start = text.find('[').or_else(|| text.find('{'))?;
    Some(&text[start..])
}

fn unescape_js_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some('t') => out.push('\t'),
            Some('b') => out.push('\u{0008}'),
            Some('f') => out.push('\u{000C}'),
            Some('u') => {
                let mut code = String::with_capacity(4);
                for _ in 0..4 {
                    if let Some(c) = chars.next() {
                        code.push(c);
                    }
                }
                if let Ok(cp) = u32::from_str_radix(&code, 16) {
                    if let Some(c) = char::from_u32(cp) {
                        out.push(c);
                    }
                }
            }
            Some(other) => out.push(other),
            None => {}
        }
    }
    out
}

fn extract_episode_payload(html: &str, episode_id: &str) -> Result<SsrCurrentEpisode> {
    let (canonical, number) = canonical_episode_id(episode_id);
    let series_id = series_id_from_episode(&canonical);
    let anime = extract_anime_payload(html, &series_id)?;

    if let Some(list) = anime.episode_list {
        if let Some(ep) = list
            .iter()
            .find(|e| e.episode_id == canonical || e.number as u32 == number)
        {
            return Ok(SsrCurrentEpisode {
                episode_id: canonical,
                number: ep.number,
                title: ep.title.clone(),
                video_host: ep.video_host.clone(),
                video_path: ep.video_path.clone(),
            });
        }
    }

    Err(anyhow!(
        "AnimeKill episode payload not found for {}",
        canonical
    ))
}

/// Strip language suffixes from episode slugs and return the canonical short
/// form along with the episode number.
fn canonical_episode_id(episode_id: &str) -> (String, u32) {
    let id = episode_id.trim_start_matches('/').trim_end_matches('/');
    let re = Regex::new(r"^(.+)-episode-(\d+)").unwrap();
    if let Some(cap) = re.captures(id) {
        let series = cap.get(1).unwrap().as_str().to_string();
        let number: u32 = cap
            .get(2)
            .unwrap()
            .as_str()
            .parse()
            .unwrap_or(1);
        return (format!("{}-episode-{}", series, number), number);
    }
    (id.to_string(), 1)
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

/// Prefer reliable embeds over obscure hosters that drop offline.
fn server_reliability_score(url: &str) -> u32 {
    let lower = url.to_lowercase();
    if lower.contains("dailymotion.com") || lower.contains("youtube.com/embed") {
        return 100;
    }
    if lower.contains("ok.ru") {
        return 90;
    }
    if lower.contains("streamsb") || lower.contains("lvturbo.com") {
        return 50;
    }
    if lower.contains("dood") || lower.contains("dooood.com") {
        return 40;
    }
    if lower.contains("streamhide") || lower.contains("guccihide.com") {
        return 30;
    }
    10
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_anime_payload_from_saved_html() {
        let html = match std::fs::read_to_string("/tmp/ak-detail.html") {
            Ok(h) => h,
            Err(_) => return,
        };
        let anime = extract_anime_payload(
            &html,
            "dragons-triumph-in-the-celestial-realm",
        )
        .unwrap();
        assert_eq!(anime.anime_id, "dragons-triumph-in-the-celestial-realm");
        assert!(!anime.name.is_empty());
        let eps = anime.episode_list.unwrap();
        assert!(!eps.is_empty());
        let ep1 = eps.iter().find(|e| e.number as u32 == 1).expect("episode 1");
        assert_eq!(ep1.episode_id, "dragons-triumph-in-the-celestial-realm-episode-1");
    }

    #[test]
    fn test_extract_episode_payload_from_saved_html() {
        let html = match std::fs::read_to_string("/tmp/ak-ep1.html") {
            Ok(h) => h,
            Err(_) => return,
        };
        let ep = extract_episode_payload(
            &html,
            "dragons-triumph-in-the-celestial-realm-episode-1",
        )
        .unwrap();
        assert_eq!(ep.episode_id, "dragons-triumph-in-the-celestial-realm-episode-1");
        assert!(ep.video_host.is_some());
        assert!(!ep.video_host.unwrap().is_empty());
    }
}
