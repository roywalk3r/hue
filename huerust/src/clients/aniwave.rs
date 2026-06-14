use anyhow::{anyhow, Result};
use moka::future::Cache;
use reqwest::header::{HeaderMap, HeaderValue};
use reqwest_middleware::ClientWithMiddleware;
use serde::Deserialize;
use std::time::Duration;

use crate::models::anime::{AnimeCard, AnimeDetail, EpisodeSummary, ServerEntry, ServerList, SkipRange, StreamPayload, SubtitleTrack};

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
    client: ClientWithMiddleware,
    episode_cache: Cache<String, Vec<AniwaveEpisode>>,
    stream_cache: Cache<String, AniwaveStream>,
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
            client: crate::http_client::wrap_with_retry(client),
            episode_cache: Cache::builder()
                .max_capacity(500)
                .time_to_live(Duration::from_secs(60 * 60))
                .build(),
            stream_cache: Cache::builder()
                .max_capacity(500)
                .time_to_live(Duration::from_secs(15 * 60))
                .build(),
        }
    }

    fn base_headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert("User-Agent", HeaderValue::from_static(UA_DESKTOP));
        headers.insert("X-Requested-With", HeaderValue::from_static("XMLHttpRequest"));
        headers
    }

    /// Resolve a slug (e.g. "one-piece-81553") to the numeric show id.
    /// Slugs follow `<name>-<id>`; the id is the trailing numeric.
    fn show_id_from_slug(slug: &str) -> Option<String> {
        slug.rsplit('-')
            .next()
            .filter(|s| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit()))
            .map(|s| s.to_string())
    }

    /// Fetch the episode list for a series. `slug_or_id` may be either a
    /// series slug (`one-piece-81553`) or a numeric show id (`81553`).
    pub async fn get_episodes(
        &self,
        slug_or_id: &str,
    ) -> Result<Vec<EpisodeSummary>> {
        let show_id = AniwaveClient::show_id_from_slug(slug_or_id)
            .unwrap_or_else(|| slug_or_id.to_string());
        if let Some(cached) = self.episode_cache.get(&show_id).await {
            return Ok(to_episode_summaries(cached));
        }

        let url = format!("{}/ajax/episode/list/{}?vrf=", AJAX, show_id);
        let resp = self
            .client
            .get(&url)
            .header("Referer", BASE_URL)
            .headers(self.base_headers())
            .send()
            .await?;
        let status = resp.status();
        let text = resp.text().await?;
        if !status.is_success() {
            return Err(anyhow!("aniwave: episode list status {} body={}", status, &text[..text.len().min(200)]));
        }
        let parsed: AniwaveListResponse = serde_json::from_str(&text)
            .map_err(|e| anyhow!("aniwave: parse episode list: {} body={}", e, &text[..text.len().min(200)]))?;
        if parsed.status != 200 {
            return Err(anyhow!("aniwave: episode list status={} body={}", parsed.status, &parsed.result));
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
        let url = format!("{}/ajax/server/list?servers={}&eps={}", AJAX, ep.show_id, ep.number);
        let resp = self
            .client
            .get(&url)
            .header("Referer", BASE_URL)
            .headers(self.base_headers())
            .send()
            .await?;
        let status = resp.status();
        let text = resp.text().await?;
        if !status.is_success() {
            return Err(anyhow!("aniwave: server list status {} body={}", status, &text[..text.len().min(200)]));
        }
        let parsed: AniwaveListResponse = serde_json::from_str(&text)
            .map_err(|e| anyhow!("aniwave: parse server list: {} body={}", e, &text[..text.len().min(200)]))?;
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
        let cache_key = format!("{}:{}:{}:{}", show_id, episode, server_id.unwrap_or(0), audio);
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
        let url = format!("{}/ajax/server/list?servers={}&eps={}", AJAX, ep.show_id, ep.number);
        let resp = self
            .client
            .get(&url)
            .header("Referer", BASE_URL)
            .headers(self.base_headers())
            .send()
            .await?;
        let text = resp.text().await?;
        let parsed: AniwaveListResponse = serde_json::from_str(&text)?;
        let links = parse_server_links(&parsed.result, audio);
        let link = match server_id {
            Some(sid) => links.iter().find(|l| l.server_id == sid).cloned(),
            None => links.first().cloned(),
        }
        .ok_or_else(|| anyhow!("aniwave: no {} servers for ep {}", audio, episode))?;

        // 2. /ajax/sources?id=<encoded>&asi=0&autoPlay=0 returns the iframe URL
        let sources_url = format!(
            "{}/sources?id={}&asi=0&autoPlay=0",
            AJAX, link.encoded_id
        );
        let resp = self
            .client
            .get(&sources_url)
            .header("Referer", BASE_URL)
            .headers(self.base_headers())
            .send()
            .await?;
        let text = resp.text().await?;
        let parsed: AniwaveSourcesResponse = serde_json::from_str(&text)?;
        if parsed.status != 200 {
            return Err(anyhow!("aniwave: sources status={}", parsed.status));
        }
        let url = parsed.result.url;
        if url.is_empty() {
            return Err(anyhow!("aniwave: empty stream url"));
        }

        // 3. Extract `id=` query param from the embed URL and call getSources
        let embed_id = url
            .split('?')
            .next()
            .and_then(|_| url.split("id=").nth(1))
            .and_then(|s| s.split('&').next())
            .unwrap_or("");
        if embed_id.is_empty() {
            return Err(anyhow!("aniwave: could not extract embed id from {}", url));
        }
        // Derive the embed host (e.g. https://play.echovideo.ru/embed-1/)
        let embed_base = url
            .split("/embed-")
            .next()
            .map(|s| format!("{}/embed-1/", s))
            .unwrap_or_else(|| url.clone());
        let get_sources_url = format!("{}getSources?id={}", embed_base, embed_id);
        let resp = self
            .client
            .get(&get_sources_url)
            .header("Referer", &url)
            .headers(self.base_headers())
            .send()
            .await?;
        let text = resp.text().await?;
        let parsed: EmbedSourcesResponse = serde_json::from_str(&text)
            .map_err(|e| anyhow!("aniwave: parse embed sources: {} body={}", e, &text[..text.len().min(200)]))?;
        let stream_url = parsed.sources;
        if stream_url.is_empty() {
            return Err(anyhow!("aniwave: empty embed sources url"));
        }
        let intro = parsed.intro.and_then(|i| {
            if i.end > i.start {
                Some(SkipRange { start: i.start as u32, end: i.end as u32 })
            } else {
                None
            }
        });
        let outro = parsed.outro.and_then(|o| {
            if o.end > o.start {
                Some(SkipRange { start: o.start as u32, end: o.end as u32 })
            } else {
                None
            }
        });

        let stream = AniwaveStream {
            url: stream_url.clone(),
            server_id: link.server_id,
            intro: intro.clone(),
            outro: outro.clone(),
            tracks: vec![],
        };
        self.stream_cache.insert(cache_key, stream.clone()).await;
        let is_hls = stream.url.contains(".m3u8");
        let subtitles = if stream.tracks.is_empty() { None } else { Some(stream.tracks) };
        Ok(StreamPayload {
            url: stream.url,
            stream_type: if is_hls { "hls".to_string() } else { "mp4".to_string() },
            subtitles,
            intro: stream.intro,
            outro: stream.outro,
            server: Some(format!("aniwave-{}", link.name)),
            referer: Some(embed_base.trim_end_matches('/').to_string()),
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
        parse_watch_page(&text, slug)
    }

    // ── helpers ───────────────────────────────────────────────────────────

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
    for li in html.split("<li ").skip(1) {
        let li_end = match li.find("</li>") {
            Some(i) => &li[..i],
            None => li,
        };
        // Anchor is the first <a ...>...</a> inside
        let a_start = match li_end.find("<a ") {
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
        ServerList { sub: vec![], dub: to_entries(&links) }
    } else {
        ServerList { sub: to_entries(&links), dub: vec![] }
    }
}

fn parse_server_links(html: &str, audio_type: &str) -> Vec<AniwaveServerLink> {
    let mut out = Vec::new();
    // Each <li data-ep-id="..." data-sv-id="N" data-link-id="<base64>">SERVERNAME</li>
    // The audio type wrapper is `<div class="type" data-type="sub"> ... </div>`
    // We capture all <li> and rely on the requested audio_type filter
    // (the wrapper would be more robust but the page returns both anyway).
    for li in html.split("<li ").skip(1) {
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
        let name = li_end
            .rsplit('>')
            .next()
            .unwrap_or("")
            .trim()
            .to_string();
        if sv_id == 0 || link_id.is_empty() || name.is_empty() {
            continue;
        }
        out.push(AniwaveServerLink {
            server_id: sv_id,
            name,
            encoded_id: link_id,
        });
    }
    out
}

fn parse_watch_page(html: &str, slug: &str) -> Result<AnimeDetail> {
    // Title: <div class="watch_title"> ... <h2 ...>TITLE</h2>
    let title = extract_between(html, "<h2", "</h2>")
        .map(|s| strip_tags(&s).trim().to_string())
        .unwrap_or_else(|| slug.to_string());
    // Poster: <img ... data-src="URL" or src="URL" ...>
    let poster = extract_attr(html, "data-src")
        .or_else(|| extract_attr(html, "src=\"https://static.aniwaves.ru"))
        .map(|s| s.trim().to_string());
    let show_id = AniwaveClient::show_id_from_slug(slug).unwrap_or_default();
    let card = AnimeCard {
        id: show_id,
        title,
        japanese_title: None,
        synopsis: None,
        image: poster,
        banner_image: None,
        anime_type: None,
        status: None,
        score: None,
        rating: None,
        genres: None,
        total_episodes: None,
        total_subbed: None,
        total_dubbed: None,
        slug: Some(slug.to_string()),
        mal_id: None,
        view_count: None,
        aired: None,
        duration: None,
        has_sub: true,
        has_dub: true,
    };
    let mut detail = AnimeDetail {
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
        episodes: None,
    };
    Ok(detail)
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
            slug: None,
            slugs: Some(vec![format!("{}-ep{}", e.show_id, e.number)]),
        })
        .collect()
}

fn stream_to_payload(s: &AniwaveStream, audio: &str) -> StreamPayload {
    let is_hls = s.url.contains(".m3u8");
    let subtitles = if s.tracks.is_empty() { None } else { Some(s.tracks.clone()) };
    StreamPayload {
        url: s.url.clone(),
        stream_type: if is_hls { "hls".to_string() } else { "mp4".to_string() },
        subtitles,
        intro: s.intro.clone(),
        outro: s.outro.clone(),
        server: Some(format!("aniwave-{}", audio)),
        referer: None,
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

