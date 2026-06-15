use anyhow::{anyhow, Result};
use moka::future::Cache;
use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::StatusCode;
use serde::Deserialize;
use std::time::Duration;
use tokio::process::Command;

use crate::models::anime::{
    AnimeCard, AnimeDetail, AnimeHomeFeed, AnimeSearchResponse, EpisodeSummary, FilterParams,
    PaginatedResponse, ServerEntry, ServerList, SkipRange, StreamPayload, SubtitleTrack,
};

const UA_DESKTOP: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/137.0.0.0 Safari/537.36";

const BASE_URL: &str = "https://aniwaves.ru";
const AJAX: &str = "https://aniwaves.ru/ajax";

/// Aniwave (aniwaves.ru) upstream scraper.
///
/// Flow (see curl examples):
/// 1. `GET /watch/<slug>` — confirm the slug exists.
/// 2. `GET /ajax/episode/list/<id>?vrf=` — list episodes with per-episode
///    `data-ids` and `data-sub`/`data-dub` flags.
/// 3. `GET /ajax/server/list?servers=<id>&eps=<n>` — list sub/dub servers
///    for an episode; each `<li>` carries an `id="<encoded-link-id>"`.
/// 4. `GET /ajax/sources?id=<encoded>&asi=0&autoPlay=0` — base64-decode
///    the `link-id` to get the real ID, then hit the JSON endpoint to
///    resolve the iframe URL. Server id is the field on each `<li>`.
/// 5. The iframe URL has a `?v=1&asi=0&autoPlay=0&ao=0` suffix;
///    parse the `id` query param out and call
///    `GET <host>/embed-1/getSources?id=<id>` to get the m3u8.
pub struct AniwaveClient {
    client: reqwest::Client,
    episode_cache: Cache<String, Vec<AniwaveEpisode>>,
    stream_cache: Cache<String, AniwaveStream>,
    card_cache: Cache<String, Vec<AnimeCard>>,
}

#[derive(Clone, Debug)]
struct AniwaveEpisode {
    number: u32,
    ep_ids: String, // raw `data-ids` like "82510&eps=1"
    has_sub: bool,
    has_dub: bool,
    /// First few chars of the `data-ids` is the show's id (numeric).
    show_id: String,
}

#[derive(Clone, Debug)]
struct AniwaveStream {
    url: String,
    server_id: u32,
    server_name: String,
    referer: String,
    intro: Option<SkipRange>,
    outro: Option<SkipRange>,
    /// Subtitle tracks parsed from the iframe page (m3u8 + lang).
    tracks: Vec<SubtitleTrack>,
}

impl Default for AniwaveClient {
    fn default() -> Self {
        Self::new()
    }
}

impl AniwaveClient {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(20))
            .connect_timeout(Duration::from_secs(10))
            .tcp_keepalive(Duration::from_secs(60))
            .user_agent(UA_DESKTOP)
            .build()
            .unwrap();
        Self {
            client,
            episode_cache: Cache::builder()
                .max_capacity(500)
                .time_to_live(Duration::from_secs(60 * 60))
                .build(),
            stream_cache: Cache::builder()
                .max_capacity(500)
                .time_to_live(Duration::from_secs(15 * 60))
                .build(),
            card_cache: Cache::builder()
                .max_capacity(200)
                .time_to_live(Duration::from_secs(10 * 60))
                .build(),
        }
    }

    fn base_headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert("User-Agent", HeaderValue::from_static(UA_DESKTOP));
        headers.insert(
            "X-Requested-With",
            HeaderValue::from_static("XMLHttpRequest"),
        );
        headers
    }

    /// Resolve a slug to the numeric show id. Accepts:
    ///   - series slug:   "one-piece-81553"  -> "81553"
    ///   - per-ep slug:   "81553-ep1"        -> "81553"
    ///   - bare numeric:  "81553"            -> "81553"
    fn show_id_from_slug(slug: &str) -> Option<String> {
        if slug.is_empty() {
            return None;
        }
        // Bare numeric
        if slug.chars().all(|c| c.is_ascii_digit()) {
            return Some(slug.to_string());
        }
        // Per-episode slug: "<digits>-ep<digits>" — take the leading digits.
        if let Some(idx) = slug.find("-ep") {
            if idx > 0 {
                let head = &slug[..idx];
                if !head.is_empty() && head.chars().all(|c| c.is_ascii_digit()) {
                    let tail = &slug[idx + 3..];
                    if !tail.is_empty() && tail.chars().all(|c| c.is_ascii_digit()) {
                        return Some(head.to_string());
                    }
                }
            }
        }
        // Series slug: "<name>-<digits>" — take the trailing digits segment.
        slug.rsplit('-')
            .next()
            .filter(|s| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit()))
            .map(|s| s.to_string())
    }

    /// Fetch the episode list for a series. `slug_or_id` may be either a
    /// series slug (`one-piece-81553`) or a numeric show id (`81553`).
    pub async fn get_episodes(&self, slug_or_id: &str) -> Result<Vec<EpisodeSummary>> {
        let show_id =
            AniwaveClient::show_id_from_slug(slug_or_id).unwrap_or_else(|| slug_or_id.to_string());
        if let Some(cached) = self.episode_cache.get(&show_id).await {
            return Ok(to_episode_summaries(cached));
        }

        let url = format!("{}/episode/list/{}?vrf=", AJAX, show_id);
        let (status, text) = self.fetch_text(&url, BASE_URL, true).await?;
        if !status.is_success() {
            return Err(anyhow!(
                "aniwave: episode list status {} body={}",
                status,
                &text[..text.len().min(200)]
            ));
        }
        let parsed: AniwaveListResponse = serde_json::from_str(&text).map_err(|e| {
            anyhow!(
                "aniwave: parse episode list: {} body={}",
                e,
                &text[..text.len().min(200)]
            )
        })?;
        if parsed.status != 200 {
            return Err(anyhow!(
                "aniwave: episode list status={} body={}",
                parsed.status,
                &parsed.result
            ));
        }
        let episodes = parse_episode_list(&show_id, &parsed.result);
        self.episode_cache.insert(show_id, episodes.clone()).await;
        Ok(to_episode_summaries(episodes))
    }

    /// Fetch the server list for an episode. The `id` is the
    /// `data-ids` string from the episode list (e.g. "82510&eps=1"),
    /// or just the episode number if a slug was provided.
    pub async fn get_servers(
        &self,
        show_id: &str,
        episode: u32,
        audio_type: &str,
    ) -> Result<ServerList> {
        let episodes = self.get_episodes_raw(show_id).await?;
        let ep = episodes
            .iter()
            .find(|e| e.number == episode)
            .or_else(|| episodes.first())
            .ok_or_else(|| anyhow!("aniwave: no episode {} for {}", episode, show_id))?;
        let url = format!(
            "{}/server/list?servers={}&eps={}",
            AJAX, ep.show_id, ep.number
        );
        let (status, text) = self.fetch_text(&url, BASE_URL, true).await?;
        if !status.is_success() {
            return Err(anyhow!(
                "aniwave: server list status {} body={}",
                status,
                &text[..text.len().min(200)]
            ));
        }
        let parsed: AniwaveListResponse = serde_json::from_str(&text).map_err(|e| {
            anyhow!(
                "aniwave: parse server list: {} body={}",
                e,
                &text[..text.len().min(200)]
            )
        })?;
        if parsed.status != 200 {
            return Err(anyhow!("aniwave: server list status={}", parsed.status));
        }
        Ok(parse_server_list(&parsed.result, audio_type))
    }

    /// Resolve a stream for an episode. Calls the aniwave sources endpoint
    /// with the encoded link id, then the embed getSources endpoint.
    pub async fn get_stream(
        &self,
        show_id: &str,
        episode: u32,
        server_id: Option<u32>,
        audio_type: &str,
    ) -> Result<StreamPayload> {
        let audio = if audio_type == "dub" { "dub" } else { "sub" };
        let cache_key = format!(
            "{}:{}:{}:{}",
            show_id,
            episode,
            server_id.unwrap_or(0),
            audio
        );
        if let Some(cached) = self.stream_cache.get(&cache_key).await {
            return Ok(stream_to_payload(&cached, audio));
        }

        let episodes = self.get_episodes_raw(show_id).await?;
        let ep = episodes
            .iter()
            .find(|e| e.number == episode)
            .or_else(|| episodes.first())
            .ok_or_else(|| anyhow!("aniwave: no episode {} for {}", episode, show_id))?;

        // 1. List servers for the episode so we can pick the right encoded link id
        let url = format!(
            "{}/server/list?servers={}&eps={}",
            AJAX, ep.show_id, ep.number
        );
        let (_, text) = self.fetch_text(&url, BASE_URL, true).await?;
        let parsed: AniwaveListResponse = serde_json::from_str(&text)?;
        let links = parse_server_links(&parsed.result, audio);
        let link = match server_id {
            Some(sid) => links.iter().find(|l| l.server_id == sid).cloned(),
            None => links.first().cloned(),
        }
        .ok_or_else(|| anyhow!("aniwave: no {} servers for ep {}", audio, episode))?;

        // 2. /ajax/sources?id=<encoded>&asi=0&autoPlay=0 returns the iframe URL
        let sources_url = format!("{}/sources?id={}&asi=0&autoPlay=0", AJAX, link.encoded_id);
        let (_, text) = self.fetch_text(&sources_url, BASE_URL, true).await?;
        let parsed: AniwaveSourcesResponse = serde_json::from_str(&text)?;
        if parsed.status != 200 {
            return Err(anyhow!("aniwave: sources status={}", parsed.status));
        }
        let url = parsed.result.url;
        if url.is_empty() {
            return Err(anyhow!("aniwave: empty stream url"));
        }

        // 3. Extract the path id from URLs like:
        //    https://play.echovideo.ru/embed-1/<id>?v=1&asi=0&autoPlay=0&ao=0
        let (embed_base, embed_id) = extract_embed_endpoint(&url)
            .ok_or_else(|| anyhow!("aniwave: could not extract embed id from {}", url))?;
        if embed_id.is_empty() {
            return Err(anyhow!("aniwave: could not extract embed id from {}", url));
        }
        let get_sources_url = format!("{}getSources?id={}", embed_base, embed_id);
        let (_, text) = self.fetch_text(&get_sources_url, &url, false).await?;
        let parsed: EmbedSourcesResponse = serde_json::from_str(&text).map_err(|e| {
            anyhow!(
                "aniwave: parse embed sources: {} body={}",
                e,
                &text[..text.len().min(200)]
            )
        })?;
        let stream_url = parsed.sources;
        if stream_url.is_empty() {
            return Err(anyhow!("aniwave: empty embed sources url"));
        }
        let intro = parsed.intro.and_then(|i| {
            if i.end > i.start {
                Some(SkipRange {
                    start: i.start as u32,
                    end: i.end as u32,
                })
            } else {
                None
            }
        });
        let outro = parsed.outro.and_then(|o| {
            if o.end > o.start {
                Some(SkipRange {
                    start: o.start as u32,
                    end: o.end as u32,
                })
            } else {
                None
            }
        });

        let stream = AniwaveStream {
            url: stream_url.clone(),
            server_id: link.server_id,
            server_name: link.name.clone(),
            referer: url.clone(),
            intro: intro.clone(),
            outro: outro.clone(),
            tracks: vec![],
        };
        self.stream_cache.insert(cache_key, stream.clone()).await;
        let is_hls = stream.url.contains(".m3u8");
        let subtitles = if stream.tracks.is_empty() {
            None
        } else {
            Some(stream.tracks)
        };
        Ok(StreamPayload {
            url: stream.url,
            stream_type: if is_hls {
                "hls".to_string()
            } else {
                "mp4".to_string()
            },
            subtitles,
            intro: stream.intro,
            outro: stream.outro,
            server: Some(format!("aniwave-{}", link.name)),
            referer: Some(stream.referer),
        })
    }

    /// Search Aniwave's filter page and return normalized cards.
    pub async fn search(
        &self,
        keyword: &str,
        page: u32,
        limit: u32,
    ) -> Result<AnimeSearchResponse> {
        let page = page.max(1);
        let limit = limit.clamp(1, 50);
        let url = format!(
            "{}/filter?keyword={}&page={}",
            BASE_URL,
            urlencoding::encode(keyword),
            page
        );
        let cards = self.fetch_cards_cached(&url, BASE_URL).await?;
        Ok(AnimeSearchResponse {
            query: keyword.to_string(),
            total: cards.len(),
            page,
            limit,
            results: cards.into_iter().take(limit as usize).collect(),
        })
    }

    /// Home feed using Aniwave's public widgets/pages.
    pub async fn get_home(&self) -> Result<AnimeHomeFeed> {
        let trending = self.fetch_widget_cards("trending", 1).await.ok();
        let latest = self
            .fetch_cards_cached(&format!("{}/updated", BASE_URL), BASE_URL)
            .await
            .ok();
        let featured = self
            .fetch_cards_cached(&format!("{}/added", BASE_URL), BASE_URL)
            .await
            .ok();
        let currently_airing = self
            .fetch_cards_cached(&format!("{}/ongoing", BASE_URL), BASE_URL)
            .await
            .ok();
        let popular = self
            .fetch_widget_cards("trending", 2)
            .await
            .ok()
            .filter(|cards| !cards.is_empty());

        Ok(AnimeHomeFeed {
            trending: trending.and_then(non_empty),
            currently_airing: currently_airing.and_then(non_empty),
            featured: featured.and_then(non_empty),
            latest: latest.and_then(non_empty),
            popular: popular.and_then(non_empty),
            finished_airing: None,
        })
    }

    /// Browse Aniwave with the same typed params as the shared discover API.
    pub async fn discover(&self, params: &FilterParams) -> Result<PaginatedResponse<AnimeCard>> {
        let page = params.page.unwrap_or(1).max(1);
        let limit = params.limit.unwrap_or(20).clamp(1, 50);
        let url = self.discover_url(params, page);
        let cards = self.fetch_cards_cached(&url, BASE_URL).await?;
        let total = cards.len() as u32;
        let items = cards.into_iter().take(limit as usize).collect::<Vec<_>>();
        Ok(PaginatedResponse {
            has_more: total > limit,
            items,
            total,
            page,
            limit,
        })
    }

    /// Look up the series page to extract title/poster/synopsis for the
    /// detail endpoint.
    pub async fn get_detail(&self, slug: &str) -> Result<AnimeDetail> {
        let url = format!("{}/watch/{}", BASE_URL, slug);
        let resp = self
            .client
            .get(&url)
            .header("Referer", BASE_URL)
            .header("User-Agent", UA_DESKTOP)
            .send()
            .await?;
        let status = resp.status();
        let text = resp.text().await?;
        if !status.is_success() {
            return Err(anyhow!("aniwave: watch page status {}", status));
        }
        let mut detail = parse_watch_page(&text, slug)?;
        if let Ok(episodes) = self.get_episodes(slug).await {
            if !episodes.is_empty() {
                let total = episodes.len() as u32;
                let subbed = episodes.iter().filter(|e| e.sub_count > 0).count() as u32;
                let dubbed = episodes.iter().filter(|e| e.dub_count > 0).count() as u32;
                detail.card.total_episodes = detail.card.total_episodes.or(Some(total));
                detail.card.total_subbed = detail.card.total_subbed.or(Some(subbed));
                detail.card.total_dubbed = detail.card.total_dubbed.or(Some(dubbed));
                detail.card.has_sub = subbed > 0;
                detail.card.has_dub = dubbed > 0;
                detail.episodes = Some(episodes);
            }
        }
        Ok(detail)
    }

    // ── helpers ───────────────────────────────────────────────────────────

    async fn fetch_text(
        &self,
        url: &str,
        referer: &str,
        ajax: bool,
    ) -> Result<(StatusCode, String)> {
        let mut request = self
            .client
            .get(url)
            .header("Referer", referer)
            .header("Accept", "application/json, text/javascript, */*; q=0.01")
            .header("Accept-Language", "en-US,en;q=0.9")
            .headers(self.base_headers());
        if !ajax {
            request = request.header(
                "X-Requested-With",
                HeaderValue::from_static("XMLHttpRequest"),
            );
        }

        let resp = request.send().await?;
        let status = resp.status();
        let text = resp.text().await?;
        if !status.is_success() || !text.is_empty() {
            return Ok((status, text));
        }

        let fallback = curl_fetch_text(url, referer, ajax).await?;
        Ok((status, fallback))
    }

    async fn get_episodes_raw(&self, show_id_or_slug: &str) -> Result<Vec<AniwaveEpisode>> {
        let show_id = AniwaveClient::show_id_from_slug(show_id_or_slug)
            .unwrap_or_else(|| show_id_or_slug.to_string());
        if let Some(cached) = self.episode_cache.get(&show_id).await {
            return Ok(cached);
        }
        self.get_episodes(&show_id).await?;
        self.episode_cache
            .get(&show_id)
            .await
            .ok_or_else(|| anyhow!("aniwave: episode list empty after fetch"))
    }

    async fn fetch_widget_cards(&self, widget: &str, page: u32) -> Result<Vec<AnimeCard>> {
        let url = format!("{}/home/widget/{}?page={}", AJAX, widget, page.max(1));
        if let Some(cached) = self.card_cache.get(&url).await {
            return Ok(cached);
        }
        let (status, text) = self
            .fetch_text(&url, &format!("{}/home", BASE_URL), true)
            .await?;
        if !status.is_success() {
            return Err(anyhow!("aniwave: widget status {}", status));
        }
        let parsed: AniwaveListResponse = serde_json::from_str(&text)?;
        if parsed.status != 200 {
            return Err(anyhow!("aniwave: widget status={}", parsed.status));
        }
        let cards = parse_card_list(&parsed.result);
        self.card_cache.insert(url, cards.clone()).await;
        Ok(cards)
    }

    async fn fetch_cards_cached(&self, url: &str, referer: &str) -> Result<Vec<AnimeCard>> {
        if let Some(cached) = self.card_cache.get(url).await {
            return Ok(cached);
        }
        let (status, text) = self.fetch_text(url, referer, false).await?;
        if !status.is_success() {
            return Err(anyhow!("aniwave: list page status {}", status));
        }
        let cards = parse_card_list(&text);
        self.card_cache.insert(url.to_string(), cards.clone()).await;
        Ok(cards)
    }

    fn discover_url(&self, params: &FilterParams, page: u32) -> String {
        if let Some(sort) = params.sort.as_deref() {
            match normalize_filter_value(sort).as_str() {
                "trending" | "popularity" => return format!("{}/home", BASE_URL),
                "latest" => return format!("{}/updated?page={}", BASE_URL, page),
                "alphabetical" => {
                    return format!(
                        "{}/filter?keyword=&sort_by=title_az&page={}",
                        BASE_URL, page
                    )
                }
                "score" => {
                    return format!("{}/filter?keyword=&sort_by=score&page={}", BASE_URL, page)
                }
                _ => {}
            }
        }

        if let Some(typ) = params.r#type.as_deref().and_then(type_slug) {
            return format!("{}/type/{}?page={}", BASE_URL, typ, page);
        }
        if let Some(status) = params.status.as_deref().and_then(status_path) {
            return format!("{}/{}?page={}", BASE_URL, status, page);
        }
        if let Some(genre) = params.genre.as_deref() {
            return format!("{}/genre/{}?page={}", BASE_URL, slugify(genre), page);
        }
        if let Some(language) = params.language.as_deref() {
            if normalize_filter_value(language) == "chinese" {
                return format!(
                    "{}/filter?keyword=&country%5B%5D=2&sort_by=last_updated&page={}",
                    BASE_URL, page
                );
            }
        }
        format!("{}/updated?page={}", BASE_URL, page)
    }
}

// ── JSON response shapes ────────────────────────────────────────────────────

#[derive(Deserialize)]
struct AniwaveListResponse {
    status: u32,
    result: String,
}

#[derive(Deserialize)]
struct AniwaveSourcesResponse {
    status: u32,
    result: AniwaveSourcesResult,
}

#[derive(Deserialize)]
struct AniwaveSourcesResult {
    url: String,
    #[serde(default)]
    skip_data: Option<AniwaveSkipData>,
    #[serde(default)]
    sources: Option<serde_json::Value>,
    #[serde(default)]
    tracks: Option<serde_json::Value>,
    #[serde(default)]
    html_guide: Option<String>,
    #[serde(default)]
    server: Option<u32>,
}

#[derive(Deserialize)]
struct AniwaveSkipData {
    #[serde(default)]
    intro: Option<AniwaveIntroOutro>,
    #[serde(default)]
    outro: Option<AniwaveIntroOutro>,
}

#[derive(Deserialize)]
struct AniwaveIntroOutro {
    #[serde(default)]
    start: f64,
    #[serde(default)]
    end: f64,
}

#[derive(Deserialize)]
struct EmbedSourcesResponse {
    #[serde(default)]
    sources: String,
    #[serde(default)]
    intro: Option<AniwaveIntroOutro>,
    #[serde(default)]
    outro: Option<AniwaveIntroOutro>,
}

async fn curl_fetch_text(url: &str, referer: &str, ajax: bool) -> Result<String> {
    let mut cmd = Command::new("/usr/bin/curl");
    cmd.arg("-fsSL")
        .arg("--compressed")
        .arg("--http1.1")
        .arg("--max-time")
        .arg("20")
        .arg("--write-out")
        .arg("%{stderr}curl_meta http=%{http_code} size=%{size_download} url=%{url_effective}\n")
        .arg("-A")
        .arg(UA_DESKTOP)
        .arg("-H")
        .arg("Accept: application/json, text/javascript, */*; q=0.01")
        .arg("-H")
        .arg("Accept-Language: en-US,en;q=0.9")
        .arg("-H")
        .arg(format!("Referer: {}", referer));

    if ajax {
        cmd.arg("-H").arg("X-Requested-With: XMLHttpRequest");
    }

    let output = cmd.arg(url).output().await?;
    if !output.status.success() {
        return Err(anyhow!(
            "aniwave: curl fallback failed status={} stderr={}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let text = String::from_utf8(output.stdout)
        .map_err(|e| anyhow!("aniwave: curl fallback returned non-utf8 body: {}", e))?;
    if text.trim().is_empty() {
        return Err(anyhow!(
            "aniwave: curl fallback returned empty body url={} stderr={}",
            url,
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(text)
}

// ── HTML parsing ───────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
struct AniwaveServerLink {
    server_id: u32,
    name: String,
    encoded_id: String,
}

fn parse_episode_list(show_id: &str, html: &str) -> Vec<AniwaveEpisode> {
    // HTML structure (relevant slice):
    //   <ul class="ep-range" data-range="...">
    //     <li title="Episode 1" ...>
    //       <a href="#" data-ids="82510&eps=1" data-num="1" data-sub="1" data-dub="0" ...>1</a>
    //     </li>
    //   </ul>
    let mut out = Vec::new();
    // Split on `<li ...>` to find each episode block
    for li in html.split("<li").skip(1) {
        let li_end = match li.find("</li>") {
            Some(i) => &li[..i],
            None => li,
        };
        // Anchor is the first <a ...>...</a> inside
        let a_start = match li_end.find("<a") {
            Some(i) => i,
            None => continue,
        };
        let a_rest = &li_end[a_start..];
        let a_end = match a_rest.find("</a>") {
            Some(i) => i,
            None => continue,
        };
        let a_block = &a_rest[..a_end];
        let ep_ids = extract_attr(a_block, "data-ids").unwrap_or_default();
        let num_str = extract_attr(a_block, "data-num").unwrap_or_default();
        let num: u32 = num_str.parse().unwrap_or(0);
        let sub: u8 = extract_attr(a_block, "data-sub")
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        let dub: u8 = extract_attr(a_block, "data-dub")
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        if ep_ids.is_empty() || num == 0 {
            continue;
        }
        // ep_ids is "<show_id>&eps=<n>" — extract the show id
        let show = ep_ids.split('&').next().unwrap_or(show_id).to_string();
        out.push(AniwaveEpisode {
            number: num,
            ep_ids,
            has_sub: sub > 0,
            has_dub: dub > 0,
            show_id: show,
        });
    }
    out
}

fn parse_server_list(html: &str, audio_type: &str) -> ServerList {
    let links = parse_server_links(html, audio_type);
    let to_entries = |links: &[AniwaveServerLink]| -> Vec<ServerEntry> {
        links
            .iter()
            .map(|l| ServerEntry {
                id: l.server_id.to_string(),
                name: l.name.clone(),
                audio_type: audio_type.to_string(),
                format: "json".to_string(),
            })
            .collect()
    };
    if audio_type == "dub" {
        ServerList {
            sub: vec![],
            dub: to_entries(&links),
        }
    } else {
        ServerList {
            sub: to_entries(&links),
            dub: vec![],
        }
    }
}

fn parse_server_links(html: &str, audio_type: &str) -> Vec<AniwaveServerLink> {
    let mut out = Vec::new();
    // Each <li data-ep-id="..." data-sv-id="N" data-link-id="<base64>">SERVERNAME</li>
    // The audio type wrapper is `<div class="type" data-type="sub"> ... </div>`
    let sections = server_sections_for_audio(html, audio_type);
    let scan_blocks: Vec<&str> = if sections.is_empty() {
        vec![html]
    } else {
        sections
    };
    for block in scan_blocks {
        parse_server_links_block(block, &mut out);
    }
    out
}

fn parse_server_links_block(block: &str, out: &mut Vec<AniwaveServerLink>) {
    for li in block.split("<li").skip(1) {
        let li_end = match li.find("</li>") {
            Some(i) => &li[..i],
            None => li,
        };
        let sv_str = match extract_attr(li_end, "data-sv-id") {
            Some(s) => s,
            None => continue,
        };
        let sv_id: u32 = sv_str.parse().unwrap_or(0);
        let link_id = match extract_attr(li_end, "data-link-id") {
            Some(s) => s,
            None => continue,
        };
        // Server name is the text after the last `>` and before `</li>`
        let name = li_end.rsplit('>').next().unwrap_or("").trim().to_string();
        if sv_id == 0 || link_id.is_empty() || name.is_empty() {
            continue;
        }
        out.push(AniwaveServerLink {
            server_id: sv_id,
            name,
            encoded_id: link_id,
        });
    }
}

fn server_sections_for_audio<'a>(html: &'a str, audio_type: &str) -> Vec<&'a str> {
    let target = if audio_type == "dub" { "dub" } else { "sub" };
    let mut markers = Vec::new();
    for needle in ["data-type=\"", "data-type='"] {
        let mut offset = 0;
        while let Some(pos) = html[offset..].find(needle) {
            let absolute = offset + pos;
            let value_start = absolute + needle.len();
            let quote = if needle.ends_with('"') { '"' } else { '\'' };
            let value_end = html[value_start..]
                .find(quote)
                .map(|i| value_start + i)
                .unwrap_or(html.len());
            markers.push((absolute, &html[value_start..value_end]));
            offset = value_end.saturating_add(1);
        }
    }
    markers.sort_by_key(|(pos, _)| *pos);
    markers
        .iter()
        .enumerate()
        .filter_map(|(idx, (start, value))| {
            if *value != target {
                return None;
            }
            let end = markers
                .get(idx + 1)
                .map(|(next, _)| *next)
                .unwrap_or(html.len());
            Some(&html[*start..end])
        })
        .collect()
}

fn extract_embed_endpoint(embed_url: &str) -> Option<(String, String)> {
    let parsed = url::Url::parse(embed_url).ok()?;
    let segments: Vec<_> = parsed.path_segments()?.collect();
    let embed_idx = segments
        .iter()
        .position(|segment| segment.starts_with("embed-"))?;
    let embed_segment = *segments.get(embed_idx)?;
    let embed_id = segments.get(embed_idx + 1)?.to_string();
    if embed_id.is_empty() {
        return None;
    }
    let origin = parsed.origin().ascii_serialization();
    Some((format!("{}/{}/", origin, embed_segment), embed_id))
}

fn parse_watch_page(html: &str, slug: &str) -> Result<AnimeDetail> {
    let json_ld = extract_json_ld_series(html);
    let title = json_ld
        .as_ref()
        .and_then(|v| json_string(v, "name"))
        .or_else(|| {
            extract_between(html, "<h1", "</h1>")
                .or_else(|| extract_between(html, "<h2", "</h2>"))
                .map(|s| strip_tags(&s).trim().to_string())
        })
        .map(|s| decode_html_entities(&s))
        .filter(|s| !s.is_empty() && s != "404 Not Found")
        .unwrap_or_else(|| slug.to_string());
    let japanese_title = json_ld
        .as_ref()
        .and_then(|v| json_string(v, "alternateName"))
        .map(|s| decode_html_entities(&s));
    let synopsis = json_ld
        .as_ref()
        .and_then(|v| json_string(v, "description"))
        .or_else(|| meta_content(html, "description"))
        .map(|s| decode_html_entities(&s));
    let image = json_ld
        .as_ref()
        .and_then(|v| json_string(v, "image"))
        .or_else(|| meta_property(html, "og:image"))
        .or_else(|| extract_attr(html, "data-src"))
        .map(|s| absolutize_url(&decode_html_entities(&s)));
    let banner_image = meta_property(html, "og:image")
        .map(|s| absolutize_url(&decode_html_entities(&s)))
        .or_else(|| image.clone());
    let anime_type = title_from_meta_title(html)
        .or_else(|| json_ld.as_ref().and_then(|v| json_string(v, "@type")))
        .or_else(|| Some("TV".to_string()));
    let rating = json_ld
        .as_ref()
        .and_then(|v| json_string(v, "contentRating"));
    let genres = json_ld.as_ref().and_then(json_genres);
    let total_episodes = json_ld
        .as_ref()
        .and_then(|v| json_u32(v, "numberOfEpisodes"))
        .or_else(|| {
            json_ld
                .as_ref()
                .and_then(|v| json_additional_u32(v, "Subbed episodes released"))
        });
    let score = json_ld
        .as_ref()
        .and_then(|v| v.get("aggregateRating"))
        .and_then(|v| json_f64(v, "ratingValue"))
        .map(trim_float);
    let aired = json_ld
        .as_ref()
        .and_then(|v| json_string(v, "datePublished"))
        .or_else(|| json_ld.as_ref().and_then(|v| json_string(v, "startDate")));
    let producers = json_ld.as_ref().and_then(json_producers);
    let show_id = AniwaveClient::show_id_from_slug(slug).unwrap_or_default();
    let card = AnimeCard {
        id: show_id,
        title,
        japanese_title,
        synopsis,
        image,
        banner_image,
        anime_type,
        status: None,
        score,
        rating,
        genres,
        total_episodes,
        total_subbed: None,
        total_dubbed: None,
        slug: Some(slug.to_string()),
        mal_id: None,
        view_count: None,
        aired,
        duration: None,
        has_sub: true,
        has_dub: true,
    };
    let detail = AnimeDetail {
        card,
        broadcast: None,
        premiered: None,
        producers,
        licensors: None,
        source: None,
        members: None,
        favorites: None,
        popularity: None,
        ranked: None,
        episodes: None,
    };
    Ok(detail)
}

fn parse_card_list(html: &str) -> Vec<AnimeCard> {
    let mut cards = Vec::new();
    for item in html.split("<div class=\"item").skip(1) {
        if let Some(card) = parse_card_block(item) {
            cards.push(card);
        }
    }
    cards
}

fn parse_card_block(block: &str) -> Option<AnimeCard> {
    let href = extract_attr(block, "href")?;
    if !href.contains("/watch/") {
        return None;
    }
    let slug = href
        .trim_start_matches('/')
        .strip_prefix("watch/")
        .unwrap_or(&href)
        .trim_matches('/')
        .to_string();
    let id = extract_attr(block, "data-tip").or_else(|| AniwaveClient::show_id_from_slug(&slug))?;
    let image = extract_attr(block, "src").map(|s| absolutize_url(&s));
    let title_block = extract_named_class_block(block, "name d-title")
        .or_else(|| extract_between(block, "<a", "</a>"))
        .unwrap_or_default();
    let title = decode_html_entities(strip_tags(&title_block).trim());
    if title.is_empty() {
        return None;
    }
    let japanese_title = extract_attr(block, "data-jp").map(|s| decode_html_entities(&s));
    let genres = parse_genre_links(block);
    let total_subbed = parse_ep_status(block, "sub");
    let total_dubbed = parse_ep_status(block, "dub");
    let total_episodes = parse_ep_status(block, "total")
        .or(total_subbed)
        .or(total_dubbed);
    let anime_type = extract_card_type(block);
    let score = extract_card_score(block);
    let view_count = extract_card_users(block);
    Some(AnimeCard {
        id,
        title,
        japanese_title,
        synopsis: None,
        image,
        banner_image: None,
        anime_type,
        status: None,
        score,
        rating: None,
        genres,
        total_episodes,
        total_subbed,
        total_dubbed,
        slug: Some(slug),
        mal_id: None,
        view_count,
        aired: None,
        duration: None,
        has_sub: total_subbed.unwrap_or(0) > 0,
        has_dub: total_dubbed.unwrap_or(0) > 0,
    })
}

fn extract_json_ld_series(html: &str) -> Option<serde_json::Value> {
    let mut offset = 0;
    while let Some(pos) = html[offset..].find(r#"<script type="application/ld+json">"#) {
        let start = offset + pos + r#"<script type="application/ld+json">"#.len();
        let end = html[start..].find("</script>").map(|i| start + i)?;
        let text = html[start..end].trim();
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(text) {
            if is_media_json_ld(&value) {
                return Some(value);
            }
            if let Some(found) = value
                .get("@graph")
                .and_then(|g| g.as_array())
                .and_then(|arr| arr.iter().find(|v| is_media_json_ld(v)))
            {
                return Some(found.clone());
            }
        }
        offset = end + "</script>".len();
    }
    None
}

fn is_media_json_ld(v: &serde_json::Value) -> bool {
    matches!(
        v.get("@type").and_then(|t| t.as_str()),
        Some("TVSeries") | Some("Movie") | Some("VideoObject")
    )
}

fn json_string(v: &serde_json::Value, key: &str) -> Option<String> {
    v.get(key)?.as_str().map(ToString::to_string)
}

fn json_u32(v: &serde_json::Value, key: &str) -> Option<u32> {
    v.get(key)?.as_u64().map(|n| n as u32)
}

fn json_f64(v: &serde_json::Value, key: &str) -> Option<f64> {
    v.get(key)?.as_f64()
}

fn json_genres(v: &serde_json::Value) -> Option<Vec<String>> {
    let genres = match v.get("genre")? {
        serde_json::Value::Array(arr) => arr
            .iter()
            .filter_map(|g| g.as_str().map(decode_html_entities))
            .collect::<Vec<_>>(),
        serde_json::Value::String(s) => vec![decode_html_entities(s)],
        _ => vec![],
    };
    non_empty(genres)
}

fn json_producers(v: &serde_json::Value) -> Option<String> {
    let arr = v.get("productionCompany")?.as_array()?;
    let names = arr
        .iter()
        .filter_map(|p| p.get("name").and_then(|n| n.as_str()))
        .map(decode_html_entities)
        .collect::<Vec<_>>();
    non_empty(names).map(|v| v.join(", "))
}

fn json_additional_u32(v: &serde_json::Value, name: &str) -> Option<u32> {
    let arr = v.get("additionalProperty")?.as_array()?;
    arr.iter().find_map(|entry| {
        let entry_name = entry.get("name")?.as_str()?;
        if entry_name.eq_ignore_ascii_case(name) {
            entry.get("value")?.as_u64().map(|n| n as u32)
        } else {
            None
        }
    })
}

fn meta_property(html: &str, property: &str) -> Option<String> {
    extract_meta_content(html, "property", property)
}

fn meta_content(html: &str, name: &str) -> Option<String> {
    extract_meta_content(html, "name", name)
}

fn extract_meta_content(html: &str, attr: &str, value: &str) -> Option<String> {
    let needle_a = format!(r#"{attr}="{value}""#);
    let needle_b = format!(r#"{attr}='{value}'"#);
    for meta in html.split("<meta").skip(1) {
        let meta = meta.split('>').next().unwrap_or(meta);
        if meta.contains(&needle_a) || meta.contains(&needle_b) {
            return extract_attr(meta, "content");
        }
    }
    None
}

fn title_from_meta_title(html: &str) -> Option<String> {
    let title = strip_tags(&extract_between(html, "<title", "</title>")?);
    if title.contains(" Movie ") {
        Some("Movie".to_string())
    } else if title.contains(" ONA ") {
        Some("ONA".to_string())
    } else if title.contains(" OVA ") {
        Some("OVA".to_string())
    } else if title.contains(" Special ") {
        Some("Special".to_string())
    } else {
        Some("TV".to_string())
    }
}

fn parse_genre_links(block: &str) -> Option<Vec<String>> {
    let mut out = Vec::new();
    for part in block.split(r#"/genre/"#).skip(1) {
        let anchor = match part.find('>') {
            Some(i) => &part[i + 1..],
            None => continue,
        };
        let text = anchor.split("</a>").next().unwrap_or("").trim();
        if !text.is_empty() {
            out.push(decode_html_entities(text));
        }
    }
    non_empty(out)
}

fn parse_ep_status(block: &str, class_name: &str) -> Option<u32> {
    let needle = format!(r#"ep-status {}"#, class_name);
    let idx = block.find(&needle)?;
    let span = &block[idx..block.len().min(idx + 180)];
    parse_u32_text(&strip_tags(span))
}

fn extract_card_type(block: &str) -> Option<String> {
    let idx = block.find(r#"<div class="right">"#)?;
    let tail = &block[idx + r#"<div class="right">"#.len()..];
    let value = tail.split("</div>").next().unwrap_or("").trim();
    if value.is_empty() {
        None
    } else {
        Some(decode_html_entities(value))
    }
}

fn extract_card_score(block: &str) -> Option<String> {
    let idx = block.find(r#"m-item rated"#)?;
    let tail = &block[idx..block.len().min(idx + 160)];
    parse_u32_text(&strip_tags(tail)).map(|n| n.to_string())
}

fn extract_card_users(block: &str) -> Option<u32> {
    let idx = block.find(r#"m-item favourited"#)?;
    let tail = &block[idx..block.len().min(idx + 180)];
    parse_compact_count(&strip_tags(tail))
}

fn extract_named_class_block(html: &str, class_name: &str) -> Option<String> {
    let idx = html.find(class_name)?;
    let start = html[..idx].rfind('<').unwrap_or(idx);
    let rest = &html[start..];
    let tag = rest
        .trim_start_matches('<')
        .split(|c: char| c.is_whitespace() || c == '>')
        .next()
        .unwrap_or("a");
    let close = format!("</{}>", tag);
    extract_between(rest, &format!("<{}", tag), &close)
}

fn parse_u32_text(text: &str) -> Option<u32> {
    let digits = text
        .chars()
        .skip_while(|c| !c.is_ascii_digit())
        .take_while(|c| c.is_ascii_digit())
        .collect::<String>();
    if digits.is_empty() {
        None
    } else {
        digits.parse().ok()
    }
}

fn parse_compact_count(text: &str) -> Option<u32> {
    let mut token = String::new();
    let mut started = false;
    for c in text.chars() {
        if c.is_ascii_digit() || (started && c == '.') {
            started = true;
            token.push(c);
        } else if started && matches!(c, 'K' | 'M' | 'k' | 'm') {
            token.push(c.to_ascii_uppercase());
            break;
        } else if started {
            break;
        }
    }
    if token.is_empty() {
        return None;
    }
    let multiplier = if token.ends_with('M') {
        1_000_000.0
    } else if token.ends_with('K') {
        1_000.0
    } else {
        1.0
    };
    let numeric = token.trim_end_matches(['M', 'K']).parse::<f64>().ok()?;
    Some((numeric * multiplier) as u32)
}

fn trim_float(value: f64) -> String {
    let rounded = format!("{:.1}", value);
    rounded.trim_end_matches(".0").to_string()
}

fn non_empty<T>(value: Vec<T>) -> Option<Vec<T>> {
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

fn type_slug(value: &str) -> Option<&'static str> {
    match normalize_filter_value(value).as_str() {
        "tv" | "tvseries" | "series" => Some("tv-series"),
        "movie" | "movies" => Some("movies"),
        "ova" => Some("ova"),
        "ona" => Some("ona"),
        "special" | "specials" => Some("special"),
        "music" => Some("music"),
        _ => None,
    }
}

fn status_path(value: &str) -> Option<&'static str> {
    match normalize_filter_value(value).as_str() {
        "airing" | "ongoing" | "currentlyairing" => Some("ongoing"),
        "finished" | "completed" => Some("completed"),
        "upcoming" | "notyetaired" => Some("upcoming"),
        _ => None,
    }
}

fn normalize_filter_value(value: &str) -> String {
    value
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

fn slugify(value: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for c in value.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    out.trim_matches('-').to_string()
}

fn absolutize_url(url: &str) -> String {
    if url.starts_with("http://") || url.starts_with("https://") {
        url.to_string()
    } else if url.starts_with("//") {
        format!("https:{}", url)
    } else if url.starts_with('/') {
        format!("{}{}", BASE_URL, url)
    } else {
        url.to_string()
    }
}

fn decode_html_entities(input: &str) -> String {
    input
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#039;", "'")
        .replace("&#x27;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
}

fn to_episode_summaries(episodes: Vec<AniwaveEpisode>) -> Vec<EpisodeSummary> {
    episodes
        .into_iter()
        .map(|e| EpisodeSummary {
            id: e.ep_ids.clone(),
            number: e.number,
            title: None,
            sub_count: if e.has_sub { 1 } else { 0 },
            dub_count: if e.has_dub { 1 } else { 0 },
            slug: Some(format!("{}-ep{}", e.show_id, e.number)),
            series_slug: Some(e.show_id.clone()),
            slugs: Some(vec![format!("{}-ep{}", e.show_id, e.number)]),
        })
        .collect()
}

fn stream_to_payload(s: &AniwaveStream, _audio: &str) -> StreamPayload {
    let is_hls = s.url.contains(".m3u8");
    let subtitles = if s.tracks.is_empty() {
        None
    } else {
        Some(s.tracks.clone())
    };
    StreamPayload {
        url: s.url.clone(),
        stream_type: if is_hls {
            "hls".to_string()
        } else {
            "mp4".to_string()
        },
        subtitles,
        intro: s.intro.clone(),
        outro: s.outro.clone(),
        server: Some(format!("aniwave-{}", s.server_name)),
        referer: Some(s.referer.clone()),
    }
}

// ── tiny HTML helpers ───────────────────────────────────────────────────────

fn extract_attr(haystack: &str, name: &str) -> Option<String> {
    // `data-ids="..."` style attribute. Tolerant of single/double quotes.
    for prefix in [format!("{}=\"", name), format!("{}='", name)] {
        if let Some(idx) = haystack.find(&prefix) {
            let rest = &haystack[idx + prefix.len()..];
            let quote = prefix.chars().last().unwrap();
            let end = rest.find(quote).unwrap_or(rest.len());
            return Some(rest[..end].to_string());
        }
    }
    None
}

fn extract_between(haystack: &str, open_tag: &str, close_tag: &str) -> Option<String> {
    let start = haystack.find(open_tag)? + open_tag.len();
    // skip past the opening tag's attributes (> or whitespace)
    let body_start = haystack[start..]
        .find('>')
        .map(|i| start + i + 1)
        .unwrap_or(start);
    let body_end = haystack[body_start..].find(close_tag)? + body_start;
    Some(haystack[body_start..body_end].to_string())
}

fn strip_tags(s: &str) -> String {
    let mut out = String::new();
    let mut inside = false;
    for c in s.chars() {
        match c {
            '<' => inside = true,
            '>' => inside = false,
            _ if !inside => out.push(c),
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_echovideo_embed_endpoint_from_path_id() {
        let (base, id) = extract_embed_endpoint(
            "https://play.echovideo.ru/embed-1/clwcEG3J9yTpXHxLHJXp?v=1&asi=0&autoPlay=0&ao=0",
        )
        .expect("embed endpoint");

        assert_eq!(base, "https://play.echovideo.ru/embed-1/");
        assert_eq!(id, "clwcEG3J9yTpXHxLHJXp");
    }

    #[test]
    fn filters_server_links_by_audio_type_section() {
        let html = r#"
            <div class="type" data-type="sub">
                <ul>
                    <li data-ep-id="1" data-sv-id="4" data-link-id="sub-link">Vidplay</li>
                </ul>
            </div>
            <div class="type" data-type="dub">
                <ul>
                    <li data-ep-id="1" data-sv-id="4" data-link-id="dub-link">Vidplay</li>
                    <li data-ep-id="1" data-sv-id="1" data-link-id="dub-b">BYFMS</li>
                </ul>
            </div>
        "#;

        let sub = parse_server_links(html, "sub");
        let dub = parse_server_links(html, "dub");

        assert_eq!(sub.len(), 1);
        assert_eq!(sub[0].encoded_id, "sub-link");
        assert_eq!(dub.len(), 2);
        assert_eq!(dub[0].encoded_id, "dub-link");
        assert_eq!(dub[1].encoded_id, "dub-b");
    }

    #[test]
    fn parses_episode_list_with_padded_tags() {
        let html = r##"
            <ul class="ep-range" data-range="001-003" style="display: block">
                <li        title="Episode 1" data-html="true">
                    <a href="#" data-ids="82510&eps=1" data-num="1" data-slug="1" data-sub="1" data-dub="0" data-ids="" class="">1</a>
                </li>
                <li        title="Episode 2" data-html="true">
                    <a        href="#" data-ids="82510&eps=2" data-num="2" data-slug="2" data-sub="1" data-dub="1" class="">2</a>
                </li>
            </ul>
        "##;

        let episodes = parse_episode_list("82510", html);

        assert_eq!(episodes.len(), 2);
        assert_eq!(episodes[0].number, 1);
        assert_eq!(episodes[0].ep_ids, "82510&eps=1");
        assert!(episodes[0].has_sub);
        assert!(!episodes[0].has_dub);
        assert_eq!(episodes[1].number, 2);
        assert!(episodes[1].has_dub);
    }

    #[test]
    fn parses_watch_page_json_ld_detail() {
        let html = r#"
            <html><head>
            <title>Aniwave - Martial God Asura 2 (2026) – Watch ONA Online in HD</title>
            <meta property="og:image" content="https://static.aniwaves.ru/resources/cover/1200x675/100/c.jpg">
            <script type="application/ld+json">{
                "@context": "https://schema.org",
                "@type": "TVSeries",
                "name": "Martial God Asura 2",
                "alternateName": "修罗武神 第2季",
                "description": "Chu Feng &amp; friends",
                "image": "https://static.aniwaves.ru/resources/cover/1200x675/100/c.jpg",
                "contentRating": "PG-13 - Teens 13 or older",
                "genre": ["Action", "Fantasy"],
                "numberOfEpisodes": 16,
                "datePublished": "2026-02-24",
                "productionCompany": [{"@type":"Organization","name":"Tang Kirin Culture"}],
                "aggregateRating": {"@type":"AggregateRating","ratingValue":3.5}
            }</script>
            </head></html>
        "#;

        let detail = parse_watch_page(html, "xiuluo-wushen-2-82510").expect("detail");

        assert_eq!(detail.card.id, "82510");
        assert_eq!(detail.card.title, "Martial God Asura 2");
        assert_eq!(
            detail.card.japanese_title.as_deref(),
            Some("修罗武神 第2季")
        );
        assert_eq!(detail.card.synopsis.as_deref(), Some("Chu Feng & friends"));
        assert_eq!(detail.card.total_episodes, Some(16));
        assert_eq!(detail.card.score.as_deref(), Some("3.5"));
        assert_eq!(
            detail.card.genres.as_ref().unwrap(),
            &vec!["Action".to_string(), "Fantasy".to_string()]
        );
        assert_eq!(detail.producers.as_deref(), Some("Tang Kirin Culture"));
    }

    #[test]
    fn parses_aniwave_card_items() {
        let html = r#"
            <div class="ani items"><div class="item "><div class="inner">
                <div class="ani poster tip tooltipstered" data-tip="81553">
                    <a href="/watch/one-piece-81553">
                        <img src="https://static.aniwaves.ru/resources/thumbnails/200x280/100/op.jpg" alt="One Piece">
                        <div class="meta"><div class="inner"><div class="left">
                            <span class="ep-status sub"><span> 1166 </span></span>
                            <span class="ep-status dub"><span> 1155</span></span>
                            <span class="ep-status total"><span>?</span></span>
                        </div><div class="right">TV</div></div></div>
                    </a>
                </div>
                <div class="info"><div class="b1">
                    <a class="name d-title" href="/watch/one-piece-81553" data-jp="One Piece">One Piece</a>
                    <div class="genre"><a href="/genre/action">Action</a>, <a href="/genre/adventure">Adventure</a></div>
                </div><div class="meta">
                    <div class="m-item favourited"><span>1.5M</span><label> Users</label></div>
                    <div class="m-item rated"><span> 9</span><label></label></div>
                </div></div>
            </div></div></div>
        "#;

        let cards = parse_card_list(html);

        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0].id, "81553");
        assert_eq!(cards[0].slug.as_deref(), Some("one-piece-81553"));
        assert_eq!(
            cards[0].image.as_deref(),
            Some("https://static.aniwaves.ru/resources/thumbnails/200x280/100/op.jpg")
        );
        assert_eq!(cards[0].total_subbed, Some(1166));
        assert_eq!(cards[0].total_dubbed, Some(1155));
        assert_eq!(cards[0].anime_type.as_deref(), Some("TV"));
        assert_eq!(cards[0].score.as_deref(), Some("9"));
        assert_eq!(cards[0].view_count, Some(1_500_000));
    }
}
