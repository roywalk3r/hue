/*
 * HUE ENGINE ❄
 * System Designation: The Architecture of Aggregation
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */

use anyhow::{anyhow, Result};
use moka::future::Cache;
use reqwest::header::{HeaderMap, HeaderValue};
use reqwest_middleware::ClientWithMiddleware;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::time::Duration;
use tracing::warn;

use crate::constants::UA_DESKTOP;
use crate::models::anime::{
    AnimeCard, AnimeDetail, AnimeHomeFeed, AnimeSearchResponse, EpisodeSummary, ServerEntry,
    ServerList,
};

const API_BASE: &str = "https://animedata.cfd/api";

const STREAM_DOMAINS: &[&str] = &[
    "https://megaplay.buzz",
    "https://animeplay.video",
    "https://embed.anicdn.to",
];

#[derive(Debug, Serialize, Deserialize, Clone)]
#[allow(non_snake_case)]
pub struct Episode {
    pub _id: String,
    pub episodeNumber: Option<u32>,
    pub slugs: Option<Vec<String>>,
    pub link: Option<EpisodeLinks>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EpisodeLinks {
    pub sub: Option<Vec<String>>,
    pub dub: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EpisodeResponse {
    pub episodes: Option<Vec<Episode>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ServerLink {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub link_type: String,
    pub format: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embed_url: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ServerResponse {
    pub sub: Vec<ServerLink>,
    pub dub: Vec<ServerLink>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Track {
    pub file: String,
    pub label: String,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct IntroOutro {
    pub start: u32,
    pub end: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StreamLink {
    pub file: String,
    #[serde(rename = "type")]
    pub link_type: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StreamData {
    pub link: StreamLink,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tracks: Option<Vec<Track>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intro: Option<IntroOutro>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outro: Option<IntroOutro>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub referer: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MegaplaySource {
    pub file: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MegaplayResponse {
    pub sources: MegaplaySource,
    #[serde(default)]
    pub tracks: Vec<Track>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intro: Option<IntroOutro>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outro: Option<IntroOutro>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server: Option<u32>,
}

pub struct HianimeClient {
    client: ClientWithMiddleware,
    episode_cache: Cache<String, EpisodeResponse>,
    stream_cache: Cache<String, StreamData>,
}

impl HianimeClient {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .connect_timeout(std::time::Duration::from_secs(8))
            .tcp_keepalive(std::time::Duration::from_secs(60))
            .pool_max_idle_per_host(2)
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
                .time_to_live(Duration::from_secs(5 * 60))
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

    pub async fn search(&self, keyword: &str) -> Result<serde_json::Value> {
        let url = format!("{}/search", API_BASE);
        let resp = self
            .client
            .post(&url)
            .headers(self.base_headers())
            .json(&json!({ "title": keyword }))
            .send()
            .await?;
        let status = resp.status();
        let text = resp.text().await?;
        tracing::debug!("[HiAnime] search status={} body={}", status, text);
        if !status.is_success() {
            return Err(anyhow!("Upstream error: {}", status));
        }
        serde_json::from_str(&text).map_err(|e| anyhow!("JSON parse error: {} body={}", e, text))
    }

    pub async fn get_latest(&self, _page: u32) -> Result<serde_json::Value> {
        let url = format!("{}/latest/anime", API_BASE);
        let resp = self
            .client
            .get(&url)
            .headers(self.base_headers())
            .send()
            .await?;
        Ok(resp.json().await?)
    }

    pub async fn get_by_category(
        &self,
        typ: &str,
        page: u32,
        limit: u32,
    ) -> Result<serde_json::Value> {
        let url = format!(
            "{}/category?type={}&page={}&limit={}",
            API_BASE, typ, page, limit
        );
        let resp = self
            .client
            .get(&url)
            .headers(self.base_headers())
            .send()
            .await?;
        Ok(resp.json().await?)
    }

    pub async fn filter(&self, body: serde_json::Value) -> Result<serde_json::Value> {
        let url = format!("{}/filter", API_BASE);
        let resp = self
            .client
            .post(&url)
            .headers(self.base_headers())
            .json(&body)
            .send()
            .await?;
        Ok(resp.json().await?)
    }

    pub async fn get_home(&self) -> Result<serde_json::Value> {
        let url = format!("{}/home?slugNth=12&includeSlugs=false", API_BASE);
        let resp = self
            .client
            .get(&url)
            .headers(self.base_headers())
            .send()
            .await?;
        Ok(resp.json().await?)
    }

    pub async fn get_anime_info(&self, slug: &str) -> Result<serde_json::Value> {
        let url = format!("{}/anime/{}", API_BASE, slug);
        let resp = self
            .client
            .get(&url)
            .headers(self.base_headers())
            .send()
            .await?;
        Ok(resp.json().await?)
    }

    pub async fn get_episodes(
        &self,
        anime_id: &str,
        start_episode: u32,
    ) -> Result<EpisodeResponse> {
        let hex_pattern = regex::Regex::new(r"^[0-9a-fA-F]{24}$").unwrap();
        let resolved_id = if hex_pattern.is_match(anime_id) {
            anime_id.to_string()
        } else {
            // Resolve episode slug → anime _id via detail endpoint
            let detail = self.get_anime_info(anime_id).await?;
            detail
                .get("anime")
                .and_then(|a| a.get("_id"))
                .and_then(|i| i.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| anime_id.to_string())
        };

        let cache_key = format!("{}:{}", resolved_id, start_episode);
        if let Some(cached) = self.episode_cache.get(&cache_key).await {
            return Ok(cached);
        }
        let url = if start_episode > 0 {
            format!(
                "{}/episodes/{}?start={}",
                API_BASE, resolved_id, start_episode
            )
        } else {
            format!("{}/episodes/{}", API_BASE, resolved_id)
        };
        let resp = self
            .client
            .get(&url)
            .headers(self.base_headers())
            .send()
            .await?;
        let data: EpisodeResponse = resp.json().await?;
        self.episode_cache.insert(cache_key, data.clone()).await;
        Ok(data)
    }

    pub async fn get_servers(&self, slug: &str) -> Result<ServerResponse> {
        let url = format!("{}/episode/{}", API_BASE, slug);
        let resp = self
            .client
            .get(&url)
            .headers(self.base_headers())
            .send()
            .await?;
        let status = resp.status();
        let text = resp.text().await?;
        tracing::debug!("[HiAnime] servers status={} body={}", status, text);
        if !status.is_success() {
            return Err(anyhow!("Upstream error: {}", status));
        }
        let data: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| anyhow!("JSON parse error: {} body={}", e, text))?;
        if let Some(episode) = data.get("episode").and_then(|e| e.get("link")) {
            let sub = parse_server_links(episode.get("sub").cloned(), "sub");
            let dub = parse_server_links(episode.get("dub").cloned(), "dub");
            Ok(ServerResponse { sub, dub })
        } else {
            Ok(ServerResponse {
                sub: vec![],
                dub: vec![],
            })
        }
    }

    pub async fn get_stream(
        &self,
        id: &str,
        server_name: Option<&str>,
        link_type: &str,
        episode: u32,
    ) -> Result<StreamData> {
        let cache_key = format!(
            "{}:{}:{}:{}",
            id,
            server_name.unwrap_or(""),
            link_type,
            episode
        );
        if let Some(cached) = self.stream_cache.get(&cache_key).await {
            return Ok(cached);
        }

        // 1. Resolve non-hex ID (episode slug) to hex anime _id via detail endpoint
        let hex_pattern = regex::Regex::new(r"^[0-9a-fA-F]{24}$").unwrap();
        let mut target_id = id.to_string();
        let mut pre_resolved_servers: Option<ServerResponse> = None;

        if !hex_pattern.is_match(id) {
            let detail = self.get_anime_info(id).await?;
            if let Some(hex_id) = detail
                .get("anime")
                .and_then(|a| a.get("_id"))
                .and_then(|i| i.as_str())
            {
                target_id = hex_id.to_string();
            }
        }

        // 2. Resolve hex Anime ID to Episode slug
        if hex_pattern.is_match(&target_id) {
            let ep_data = self.get_episodes(&target_id, episode).await?;
            if let Some(eps) = ep_data.episodes {
                let target_ep = eps
                    .iter()
                    .find(|e| e.episodeNumber == Some(episode))
                    .or_else(|| eps.first());
                if let Some(ep) = target_ep {
                    target_id = ep
                        .slugs
                        .as_ref()
                        .and_then(|s| s.first().cloned())
                        .unwrap_or_else(|| ep._id.clone());
                    if ep.link.as_ref().is_some_and(|l| {
                        l.sub.as_ref().is_some_and(|v| !v.is_empty())
                            || l.dub.as_ref().is_some_and(|v| !v.is_empty())
                    }) {
                        let sub = parse_server_links(
                            ep.link.as_ref().and_then(|l| {
                                l.sub.as_ref().map(|v| {
                                    serde_json::Value::Array(
                                        v.iter()
                                            .map(|s| serde_json::Value::String(s.clone()))
                                            .collect(),
                                    )
                                })
                            }),
                            "sub",
                        );
                        let dub = parse_server_links(
                            ep.link.as_ref().and_then(|l| {
                                l.dub.as_ref().map(|v| {
                                    serde_json::Value::Array(
                                        v.iter()
                                            .map(|s| serde_json::Value::String(s.clone()))
                                            .collect(),
                                    )
                                })
                            }),
                            "dub",
                        );
                        pre_resolved_servers = Some(ServerResponse { sub, dub });
                    }
                }
            }
        }

        // 2. Get Servers
        let servers = if let Some(srv) = pre_resolved_servers {
            srv
        } else {
            self.get_servers(&target_id).await?
        };

        let list = if link_type == "dub" {
            &servers.dub
        } else {
            &servers.sub
        };
        if list.is_empty() {
            return Err(anyhow!("SERVERS_NOT_FOUND"));
        }

        let target = server_name
            .and_then(|sn| list.iter().find(|s| s.name == sn))
            .or_else(|| list.iter().find(|s| s.format == "json"))
            .or_else(|| list.first())
            .cloned()
            .ok_or_else(|| anyhow!("SERVER_NOT_FOUND"))?;

        // 3. Early return for embed servers — their id IS the iframe URL
        if target.format == "embed" {
            let result = StreamData {
                link: StreamLink {
                    file: target.id.clone(),
                    link_type: "iframe".to_string(),
                },
                tracks: None,
                intro: None,
                outro: None,
                server: Some(target.name.clone()),
                referer: None,
            };
            self.stream_cache.insert(cache_key, result.clone()).await;
            return Ok(result);
        }

        // 4. For JSON servers, resolve the real data-id from the iframe page
        let mut resolved_source_id = target.id.clone();
        if let Some(url) = target.embed_url.as_ref() {
            if let Some(data_id) = extract_data_id_from_iframe(&self.client, url).await {
                resolved_source_id = data_id;
            }
        }

        // 5. Attempt getSources extraction
        for stream_dom in STREAM_DOMAINS {
            let url = format!("{}/stream/getSources?id={}", stream_dom, resolved_source_id);
            let mut headers = HeaderMap::new();
            headers.insert("User-Agent", HeaderValue::from_static(UA_DESKTOP));
            headers.insert(
                "X-Requested-With",
                HeaderValue::from_static("XMLHttpRequest"),
            );
            headers.insert("Referer", HeaderValue::from_static(stream_dom));

            match self.client.get(&url).headers(headers).send().await {
                Ok(resp) if resp.status().is_success() => {
                    if let Ok(data) = resp.json::<MegaplayResponse>().await {
                        if !data.sources.file.is_empty() {
                            let result = StreamData {
                                link: StreamLink {
                                    file: data.sources.file,
                                    link_type: "hls".to_string(),
                                },
                                tracks: Some(data.tracks),
                                intro: data.intro,
                                outro: data.outro,
                                server: Some(target.name.clone()),
                                referer: Some(format!("{}/", stream_dom)),
                            };
                            self.stream_cache.insert(cache_key, result.clone()).await;
                            return Ok(result);
                        }
                    }
                }
                Ok(resp) => {
                    warn!("[HiAnime] {} returned status {}", stream_dom, resp.status());
                }
                Err(e) => {
                    warn!("[HiAnime] {} fetch error: {}", stream_dom, e);
                }
            }
        }

        Err(anyhow!("EXTRACTION_FAILED"))
    }
}

/// Extract the real `data-id` from a megaplay/animeplay iframe page HTML.
/// The iframe page contains `<div data-id="36396" data-realid="2142">`;
/// the site uses `data-id` for `getSources`, not the server ID from the URL.
async fn extract_data_id_from_iframe(
    client: &ClientWithMiddleware,
    iframe_url: &str,
) -> Option<String> {
    let mut headers = HeaderMap::new();
    headers.insert("User-Agent", HeaderValue::from_static(UA_DESKTOP));
    headers.insert("Referer", HeaderValue::from_static("https://hianimes.se/"));

    match client.get(iframe_url).headers(headers).send().await {
        Ok(resp) if resp.status().is_success() => {
            if let Ok(html) = resp.text().await {
                let re = regex::Regex::new(r#"data-id\s*=\s*"(\d+)""#).ok()?;
                if let Some(caps) = re.captures(&html) {
                    let data_id = caps.get(1)?.as_str().to_string();
                    tracing::debug!("[HiAnime] iframe {} => data-id {}", iframe_url, data_id);
                    return Some(data_id);
                }
            }
        }
        Ok(resp) => {
            tracing::warn!(
                "[HiAnime] iframe {} returned status {}",
                iframe_url,
                resp.status()
            );
        }
        Err(e) => {
            tracing::warn!("[HiAnime] iframe {} fetch error: {}", iframe_url, e);
        }
    }
    None
}

fn parse_server_links(links: Option<serde_json::Value>, link_type: &str) -> Vec<ServerLink> {
    let arr = match links {
        Some(serde_json::Value::Array(a)) => a,
        _ => return vec![],
    };

    arr.iter()
        .filter_map(|link| {
            let url_str = link.as_str()?;

            // Case 1: Internal JSON Server (megaplay/animeplay)
            let re = regex::Regex::new(r"/s-2/(\d+)").ok()?;
            if let Some(caps) = re.captures(url_str) {
                let server_id = caps.get(1)?.as_str().to_string();
                let name = if url_str.contains("megaplay") {
                    "HD-1"
                } else {
                    "HD-2"
                };
                return Some(ServerLink {
                    id: server_id,
                    name: name.to_string(),
                    link_type: link_type.to_string(),
                    format: "json".to_string(),
                    embed_url: Some(url_str.to_string()),
                });
            }

            // Case 2: Direct Embed (Vidmoly, Vidwish, etc)
            if let Ok(parsed) = url::Url::parse(url_str) {
                let host = parsed.host_str()?;
                let clean = host.replace("www.", "");
                let name = clean.split('.').next()?.to_uppercase();
                return Some(ServerLink {
                    id: url_str.to_string(),
                    name,
                    link_type: link_type.to_string(),
                    format: "embed".to_string(),
                    embed_url: Some(url_str.to_string()),
                });
            }

            None
        })
        .collect()
}

// ─── Data Transformers: Raw Upstream → Clean Models ───────────────────────────

fn opt_str(v: &serde_json::Value, key: &str) -> Option<String> {
    v.get(key).and_then(|f| f.as_str()).map(|s| s.to_string())
}

fn opt_u32(v: &serde_json::Value, key: &str) -> Option<u32> {
    v.get(key).and_then(|f| {
        f.as_u64()
            .map(|n| n as u32)
            .or_else(|| f.as_str().and_then(|s| s.parse().ok()))
    })
}

fn opt_str_arr(v: &serde_json::Value, key: &str) -> Option<Vec<String>> {
    v.get(key).and_then(|f| f.as_array()).map(|arr| {
        arr.iter()
            .filter_map(|i| i.as_str().map(|s| s.to_string()))
            .collect()
    })
}

fn has_servers(ep: &serde_json::Value, track: &str) -> bool {
    ep.get("link")
        .and_then(|l| l.get(track))
        .and_then(|l| l.as_array())
        .map(|a| !a.is_empty())
        .unwrap_or(false)
}

/// Build a slug from the first available slug source.
fn pick_slug(v: &serde_json::Value) -> Option<String> {
    v.get("slug")
        .and_then(|s| s.as_str().map(|x| x.to_string()))
        .or_else(|| {
            v.get("slugs")
                .and_then(|p| p.as_array())
                .and_then(|a| a.first())
                .and_then(|f| f.as_str().map(|s| s.to_string()))
        })
        .or_else(|| {
            v.get("public_id")
                .and_then(|p| p.as_array())
                .and_then(|a| a.first())
                .and_then(|f| f.as_str().map(|s| s.to_string()))
        })
        .or_else(|| v.get("_id").and_then(|i| i.as_str().map(|s| s.to_string())))
}

/// Extract totalSub / totalDub from either top-level fields or episode data.
fn count_sub_dub(v: &serde_json::Value) -> (Option<u32>, Option<u32>) {
    let ts = opt_u32(v, "totalSub").or_else(|| opt_u32(v, "totalSubbed"));
    let td = opt_u32(v, "totalDub").or_else(|| opt_u32(v, "totalDubbed"));
    (ts, td)
}

/// Does this anime have at least one episode with sub/dub links?
fn anime_has_sub_dub(v: &serde_json::Value) -> (bool, bool) {
    let eps = v.get("episodes").and_then(|e| e.as_array());
    if let Some(arr) = eps {
        let sub = arr.iter().any(|ep| has_servers(ep, "sub"));
        let dub = arr.iter().any(|ep| has_servers(ep, "dub"));
        (sub, dub)
    } else {
        (false, false)
    }
}

pub fn transform_anime_card(v: &serde_json::Value) -> AnimeCard {
    let title = opt_str(v, "English")
        .or_else(|| opt_str(v, "title"))
        .or_else(|| opt_str(v, "Japanese"))
        .unwrap_or_default();

    let (has_sub, has_dub) = anime_has_sub_dub(v);
    let (total_subbed, total_dubbed) = count_sub_dub(v);

    let id = pick_slug(v).unwrap_or_default();

    AnimeCard {
        id,
        title,
        japanese_title: opt_str(v, "Japanese"),
        synopsis: opt_str(v, "synopsis"),
        image: opt_str(v, "image"),
        banner_image: opt_str(v, "landScapeImage"),
        anime_type: opt_str(v, "Type"),
        status: opt_str(v, "Status"),
        score: opt_str(v, "Score"),
        rating: opt_str(v, "Rating"),
        genres: {
            // Try multiple possible keys the API might return
            let arr = opt_str_arr(v, "genres")
                .or_else(|| opt_str_arr(v, "Genres"))
                .or_else(|| opt_str_arr(v, "genreList"))
                .or_else(|| {
                    // Fallback: comma-separated string under "genre"
                    v.get("genre")
                        .and_then(|g| g.as_str())
                        .map(|s| s.split(',').map(|g| g.trim().to_string()).collect())
                });
            arr
        },
        total_episodes: opt_u32(v, "totalEpisodes"),
        total_subbed,
        total_dubbed,
        slug: pick_slug(v),
        mal_id: opt_u32(v, "mal_id"),
        view_count: opt_u32(v, "recentViewsCount"),
        aired: opt_str(v, "Aired"),
        duration: opt_str(v, "Duration"),
        country: None,
        has_sub,
        has_dub,
    }
}

/// Transform a raw upstream anime info response into a rich AnimeDetail.
pub fn transform_anime_detail(raw: serde_json::Value) -> AnimeDetail {
    let anime = raw.get("anime").unwrap_or(&raw);
    let card = transform_anime_card(anime);

    let episodes: Option<Vec<EpisodeSummary>> =
        anime.get("episodes").and_then(|e| e.as_array()).map(|arr| {
            arr.iter()
                .map(|ep| {
                    let slug = opt_str(ep, "slug");
                    EpisodeSummary {
                        id: opt_str(ep, "_id").unwrap_or_default(),
                        number: opt_u32(ep, "episodeNumber").unwrap_or(0),
                        title: opt_str(ep, "title"),
                        sub_count: ep
                            .get("link")
                            .and_then(|l| l.get("sub"))
                            .and_then(|s| s.as_array())
                            .map(|a| a.len() as u32)
                            .unwrap_or(0),
                        dub_count: ep
                            .get("link")
                            .and_then(|l| l.get("dub"))
                            .and_then(|s| s.as_array())
                            .map(|a| a.len() as u32)
                            .unwrap_or(0),
                        slug: slug.clone(),
                        series_slug: slug,
                        slugs: opt_str_arr(ep, "slugs"),
                    }
                })
                .collect()
        });

    AnimeDetail {
        card,
        broadcast: opt_str(anime, "Broadcast"),
        premiered: opt_str(anime, "Premiered"),
        producers: opt_str(anime, "Producers"),
        licensors: opt_str(anime, "Licensors"),
        source: opt_str(anime, "Source"),
        members: opt_str(anime, "Members"),
        favorites: opt_str(anime, "Favorites"),
        popularity: opt_str(anime, "Popularity"),
        ranked: opt_str(anime, "Ranked"),
        episodes,
    }
}

/// Transform raw search results into a clean AnimeSearchResponse.
pub fn transform_search_results(raw: serde_json::Value, query: &str) -> AnimeSearchResponse {
    let items = match raw.as_array() {
        Some(arr) => arr.iter().map(transform_anime_card).collect(),
        None => vec![],
    };
    AnimeSearchResponse {
        query: query.to_string(),
        total: items.len(),
        page: 1,
        limit: items.len() as u32,
        results: items,
    }
}

/// Extract a vector of AnimeCard from a raw JSON array or a nested "animes" field.
fn extract_anime_list(v: &serde_json::Value) -> Vec<AnimeCard> {
    let empty: Vec<serde_json::Value> = vec![];
    let arr = v
        .as_array()
        .or_else(|| v.get("animes").and_then(|a| a.as_array()))
        .unwrap_or(&empty);
    arr.iter().map(transform_anime_card).collect()
}

/// Transform raw upstream home response into a structured AnimeHomeFeed.
pub fn transform_home_feed(raw: serde_json::Value) -> AnimeHomeFeed {
    AnimeHomeFeed {
        trending: raw.get("trending").map(extract_anime_list),
        currently_airing: raw.get("currentlyAiring").map(extract_anime_list),
        featured: raw.get("featured").map(extract_anime_list),
        latest: raw.get("latestAnime").map(extract_anime_list),
        popular: raw.get("popular").map(extract_anime_list),
        finished_airing: raw.get("finishedAiring").map(extract_anime_list),
    }
}

/// Transform raw category / latest results into a clean list.
pub fn transform_category_results(raw: serde_json::Value) -> Vec<AnimeCard> {
    extract_anime_list(&raw)
}

/// Transform our internal ServerResponse into the clean ServerList model.
pub fn transform_server_list(resp: &ServerResponse) -> ServerList {
    fn to_entries(list: &[ServerLink]) -> Vec<ServerEntry> {
        list.iter()
            .map(|s| ServerEntry {
                id: s.id.clone(),
                name: s.name.clone(),
                audio_type: s.link_type.clone(),
                format: s.format.clone(),
            })
            .collect()
    }
    ServerList {
        sub: to_entries(&resp.sub),
        dub: to_entries(&resp.dub),
    }
}
