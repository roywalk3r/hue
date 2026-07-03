use anyhow::{anyhow, Result};
use regex::Regex;
use reqwest_middleware::ClientWithMiddleware;
use tracing::warn;

use crate::models::anime::{
    AnimeCard, AnimeDetail, AnimeHomeFeed, AnimeSearchResponse, EpisodeSummary, FilterParams,
    PaginatedResponse, ServerEntry, ServerList, StreamPayload,
};

const BASE_URL: &str = "https://animexin.dev";
const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36";

#[derive(Clone)]
pub struct AnimexinClient {
    client: ClientWithMiddleware,
}

impl AnimexinClient {
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

    /// Search WordPress posts by query.
    pub async fn search(&self, keyword: &str) -> Result<AnimeSearchResponse> {
        let url = format!("{}/?s={}", BASE_URL, urlencoding::encode(keyword));
        let resp = self.client.get(&url).send().await?;
        let html = resp.text().await?;

        let mut results = Vec::new();
        // (?s) makes . match newlines — article content spans multiple lines
        let article_re = Regex::new(r#"(?s)<article[^>]*class="bs"[^>]*>(.*?)</article>"#)?;
        let link_re =
            Regex::new(r#"<a\s+href="https://animexin\.dev/([^"]+)/"[^>]*title="([^"]+)""#)?;
        let img_re = Regex::new(r#"<img[^>]+src="([^"]+)""#)?;
        let type_re = Regex::new(r#"<div\s+class="typez\s+([^"]+)">"#)?;
        let status_re = Regex::new(r#"<div\s+class="status\s+([^"]+)">"#)?;

        for article in article_re.captures_iter(&html) {
            let block = article.get(1).map(|m| m.as_str()).unwrap_or("");
            if let Some(cap) = link_re.captures(block) {
                let slug = cap
                    .get(1)
                    .map(|m| m.as_str().to_string())
                    .unwrap_or_default();
                let title = cap
                    .get(2)
                    .map(|m| m.as_str().trim().to_string())
                    .unwrap_or_default();
                if slug.is_empty() || title.is_empty() || is_utility_page(&slug) {
                    continue;
                }
                if results.iter().any(|c: &AnimeCard| c.id == slug) {
                    continue;
                }
                let image = img_re
                    .captures(block)
                    .and_then(|c| c.get(1).map(|m| m.as_str().to_string()));
                let anime_type = type_re
                    .captures(block)
                    .and_then(|c| c.get(1).map(|m| m.as_str().trim().to_string()));
                let status = status_re
                    .captures(block)
                    .and_then(|c| c.get(1).map(|m| m.as_str().trim().to_string()));

                results.push(AnimeCard {
                    id: slug.clone(),
                    title: clean_title(&title),
                    japanese_title: None,
                    synopsis: None,
                    image,
                    banner_image: None,
                    anime_type: anime_type.or_else(|| Some("TV".to_string())),
                    status,
                    score: None,
                    rating: None,
                    genres: None,
                    total_episodes: None,
                    total_subbed: None,
                    total_dubbed: None,
                    slug: Some(slug),
                    mal_id: None,
                    view_count: None,
                    aired: None,
                    duration: None,
                    country: Some("CN".into()),
                    source: Some("animexin".to_string()),
                    has_sub: true,
                    has_dub: false,
                });
            }
        }

        let total = results.len();
        Ok(AnimeSearchResponse {
            query: keyword.to_string(),
            total,
            page: 1,
            limit: total as u32,
            results,
        })
    }

    /// Scrape a season/series page for full detail + episode list.
    pub async fn get_detail(&self, season_slug: &str) -> Result<AnimeDetail> {
        let url = format!("{}/{}/", BASE_URL, season_slug.trim_end_matches('/'));
        let resp = self.client.get(&url).send().await?;
        let status = resp.status();
        let html = resp.text().await?;
        if status.as_u16() == 404 || html.contains("404 Not Found") {
            return Err(anyhow!("Animexin series page not found ({})", season_slug));
        }

        let title = extract_meta_content(&html, r#"property="og:title"\s+content="([^"]+)""#)
            .or_else(|| extract_meta_content(&html, r#"<title>([^<]+)</title>"#))
            .map(|t| clean_title(&t))
            .unwrap_or_else(|| clean_title(season_slug));

        let synopsis =
            extract_meta_content(&html, r#"property="og:description"\s+content="([^"]+)""#)
                .or_else(|| {
                    extract_meta_content(&html, r#"name="description"\s+content="([^"]+)""#)
                });

        let image = extract_meta_content(&html, r#"property="og:image"\s+content="([^"]+)""#);

        let episodes = self.extract_episodes_from_html(&html, season_slug).await?;
        let total_eps = if episodes.is_empty() {
            None
        } else {
            Some(episodes.len() as u32)
        };

        let card = AnimeCard {
            id: season_slug.to_string(),
            title: title.clone(),
            japanese_title: None,
            synopsis: synopsis.clone(),
            image: image.clone(),
            banner_image: image.clone(),
            anime_type: Some("TV".to_string()),
            status: None,
            score: None,
            rating: None,
            genres: None,
            total_episodes: total_eps,
            total_subbed: total_eps,
            total_dubbed: None,
            slug: Some(season_slug.to_string()),
            mal_id: None,
            view_count: None,
            aired: None,
            duration: None,
            country: Some("CN".into()),
            source: Some("animexin".to_string()),
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

    /// Scrape a season/series page for episode links.
    pub async fn get_episodes(&self, season_slug: &str) -> Result<Vec<EpisodeSummary>> {
        let url = format!("{}/{}/", BASE_URL, season_slug.trim_end_matches('/'));
        let resp = self.client.get(&url).send().await?;
        let html = resp.text().await?;
        self.extract_episodes_from_html(&html, season_slug).await
    }

    async fn extract_episodes_from_html(
        &self,
        html: &str,
        series_slug: &str,
    ) -> Result<Vec<EpisodeSummary>> {
        let mut episodes = Vec::new();
        let series_prefix = series_slug.split('-').take(3).collect::<Vec<_>>().join("-");

        // Broad episode link regex
        let re = Regex::new(r#"href="https://animexin\.dev/([^"]*episode[^"]*)/""#)?;
        for cap in re.captures_iter(html) {
            let slug = cap
                .get(1)
                .map(|m| m.as_str().to_string())
                .unwrap_or_default();
            if slug.is_empty() {
                continue;
            }
            // Only keep episodes that belong to this series (prefix match)
            if !slug.starts_with(&series_prefix) && !slug.contains(series_slug) {
                continue;
            }
            let ep_num = extract_episode_number(&slug);
            if ep_num == 0 {
                continue; // likely not a real episode
            }
            episodes.push(EpisodeSummary {
                id: slug.clone(),
                number: ep_num,
                title: None,
                sub_count: 1,
                dub_count: 0,
                slug: Some(slug.clone()),
                series_slug: Some(slug),
                slugs: None,
            });
        }

        episodes.sort_by_key(|e| e.number);
        episodes.dedup_by(|a, b| a.number == b.number);
        Ok(episodes)
    }

    /// Scrape an episode page for available stream servers and download links.
    pub async fn get_servers(&self, episode_slug: &str) -> Result<ServerList> {
        let url = format!("{}/{}/", BASE_URL, episode_slug.trim_end_matches('/'));
        let resp = self.client.get(&url).send().await?;
        let html = resp.text().await?;

        let mut sub = Vec::new();

        // 1. Base64-encoded iframe embeds (primary sources)
        // The dropdown may use <li> buttons instead of <option> tags.
        let mut seen_sources: std::collections::HashSet<String> = std::collections::HashSet::new();
        let option_re = Regex::new(r#"<option\s+value="([A-Za-z0-9+/=]+)"(?:\s+data-index="[^"]*")?\s*>([^<]+)</option>"#).ok();
        let li_re = Regex::new(r#"(?s)<li[^>]*>.*?value="([A-Za-z0-9+/=]+)".*?</li>"#).ok();
        let li_label_re = Regex::new(r#"<li[^>]*>.*?<span[^>]*>([^<]+)</span>.*?</li>"#)
            .ok()
            .map(|re| std::sync::Arc::new(re));
        let mut extract_embed = |b64: &str, label: &str| {
            if b64.len() < 20 || seen_sources.contains(b64) {
                return;
            }
            if let Ok(decoded_bytes) = base64::decode(b64) {
                if let Ok(decoded_html) = String::from_utf8(decoded_bytes) {
                    if let Some(src) = extract_iframe_src(&decoded_html) {
                        seen_sources.insert(b64.to_string());
                        let host_name = extract_server_name_from_url(&src);
                        let label = label.trim();
                        let name = if label.is_empty() {
                            host_name
                        } else {
                            format!("{} - {}", host_name, label)
                        };
                        sub.push(ServerEntry {
                            id: src,
                            name,
                            audio_type: "sub".to_string(),
                            format: "iframe".to_string(),
                        });
                    }
                }
            }
        };

        for cap in option_re.as_ref().unwrap().captures_iter(&html) {
            let b64 = cap.get(1).map(|m| m.as_str()).unwrap_or_default();
            let label = cap.get(2).map(|m| m.as_str()).unwrap_or_default();
            extract_embed(b64, label);
        }

        if let Some(re) = li_re.as_ref() {
            for cap in re.captures_iter(&html) {
                let b64 = cap.get(1).map(|m| m.as_str()).unwrap_or_default();
                // Try to find a matching label in the same <li>
                let full_li = cap.get(0).map(|m| m.as_str()).unwrap_or("");
                let label = li_label_re
                    .as_ref()
                    .and_then(|re| re.captures(full_li))
                    .and_then(|c| c.get(1).map(|m| m.as_str()))
                    .unwrap_or("")
                    .trim();
                extract_embed(b64, label);
            }
        }

        // 2. Direct download links (Terabox, Mediafire, Mirrored.to, Streamwish)
        let dl_re = Regex::new(
            r#"<a\s+href="(https://(?:1024terabox\.com|www\.mediafire\.com|mirrored\.to|streamwish\.com|filelions\.to|wolfstream\.tv|speedostream\.com)[^"]*)"[^>]*>([^<]+)</a>"#,
        )?;
        for cap in dl_re.captures_iter(&html) {
            let link = cap
                .get(1)
                .map(|m| m.as_str().to_string())
                .unwrap_or_default();
            let label = cap
                .get(2)
                .map(|m| m.as_str().trim().to_string())
                .unwrap_or_default();
            if link.is_empty() {
                continue;
            }
            let name = if label.is_empty() {
                extract_server_name_from_url(&link)
            } else {
                format!("{}-DL", label.to_uppercase())
            };
            sub.push(ServerEntry {
                id: link,
                name,
                audio_type: "sub".to_string(),
                format: "iframe".to_string(), // Treat as iframe/embed for now
            });
        }

        Ok(ServerList { sub, dub: vec![] })
    }

    /// Get the first available stream as a StreamPayload.
    pub async fn get_stream(
        &self,
        episode_slug: &str,
        server: Option<&str>,
    ) -> Result<StreamPayload> {
        let servers = self.get_servers(episode_slug).await?;

        // Prefer non-download sources first when no server is explicitly picked.
        let playable: Vec<_> = servers
            .sub
            .iter()
            .filter(|s| !s.name.to_uppercase().ends_with("-DL"))
            .collect();
        let fallback: Vec<_> = servers.sub.iter().collect();
        let pool = if playable.is_empty() { &fallback } else { &playable };

        let target = if let Some(srv) = server {
            pool.iter()
                .find(|s| s.name.eq_ignore_ascii_case(srv))
                .or_else(|| pool.first())
                .copied()
        } else {
            pool.first().copied()
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

    /// Scrape homepage sections into AnimeHomeFeed.
    /// Maps:
    ///   trending       → Popular Today
    ///   latest         → Latest Release
    ///   popular        → Popular Donghua (weekly)
    ///   featured       → New Movie
    pub async fn get_home(&self) -> Result<AnimeHomeFeed> {
        let html = self.client.get(BASE_URL).send().await?.text().await?;

        let popular_today = extract_section_html(
            &html,
            r#"<div class="releases hothome"><h2>Popular Today</h2></div>"#,
            &[r#"<div class="releases latesthome">"#],
        )
        .map(parse_article_cards)
        .filter(|v| !v.is_empty());

        let latest_release = extract_section_html(
            &html,
            r#"<div class="releases latesthome"><h3>Latest Release</h3>"#,
            &[r#"<div class="section"><div class="releases"><h3>Popular Donghua</h3></div>"#],
        )
        .map(parse_article_cards)
        .filter(|v| !v.is_empty());

        let popular_donghua = extract_section_html(
            &html,
            r#"<div class="section"><div class="releases"><h3>Popular Donghua</h3></div>"#,
            &[r#"<script>ts_popular_widget.run"#],
        )
        .map(parse_serieslist_cards)
        .filter(|v| !v.is_empty());

        let new_movie = extract_section_html(
            &html,
            r#"<div class="section"><div class="releases"><h3><span>New Movie</span></h3>"#,
            &[r#"<div class="widget_text section">"#],
        )
        .map(parse_serieslist_cards)
        .filter(|v| !v.is_empty());

        Ok(AnimeHomeFeed {
            trending: popular_today,
            currently_airing: None,
            featured: new_movie,
            latest: latest_release,
            popular: popular_donghua,
            finished_airing: None,
        })
    }

    pub async fn discover(&self, params: &FilterParams) -> Result<PaginatedResponse<AnimeCard>> {
        let start_page = params.page.unwrap_or(1).max(1) as usize;
        let limit = params.limit.unwrap_or(20).clamp(1, 200) as usize;

        // Map generic aggregator sort names to Animexin's order parameter.
        let order = match params.sort.as_deref() {
            Some("popularity") | Some("trending") => "popular",
            Some("rating") | Some("score") => "rating",
            Some("alphabetical") | Some("az") | Some("a-z") => "title",
            Some("title_reverse") | Some("title-reverse") | Some("z-a") | Some("za") => {
                "titlereverse"
            }
            Some("added") | Some("latest_added") => "added",
            _ => "update",
        };
        let status = params.status.as_deref().unwrap_or("");
        let anime_type = params.r#type.as_deref().unwrap_or("");
        let genre = params.genre.as_deref().unwrap_or("");

        let article_re = Regex::new(r#"(?s)<article[^>]*class="bs"[^>]*>(.*?)</article>"#)?;
        let link_re =
            Regex::new(r#"<a\s+href="https://animexin\.dev/([^"]+)/"[^>]*title="([^"]+)""#)?;
        let img_re = Regex::new(r#"<img[^>]+src="([^"]+)""#)?;
        let type_re = Regex::new(r#"<div\s+class="typez\s+([^"]+)">"#)?;
        let status_re = Regex::new(r#"<div\s+class="status\s+([^"]+)">"#)?;

        let mut items = Vec::new();
        let mut current_page = start_page;
        // Animexin's series listing shows 30 items per page; keep fetching
        // pages until we satisfy the requested limit (up to 10 pages to
        // avoid runaway requests).
        loop {
            let url = format!(
                "{}/anime/?status={}&type={}&order={}&page={}",
                BASE_URL,
                urlencoding::encode(status),
                urlencoding::encode(anime_type),
                order,
                current_page
            );
            let url = if genre.is_empty() {
                url
            } else {
                format!("{}&genre={}", url, urlencoding::encode(genre))
            };
            let html = match self.client.get(&url).send().await {
                Ok(resp) => resp.text().await?,
                Err(e) => {
                    warn!("[Animexin] discover page {} fetch failed: {}", current_page, e);
                    break;
                }
            };

            let mut page_items = 0usize;
            for cap in article_re.captures_iter(&html) {
                let block = cap.get(1).map(|m| m.as_str()).unwrap_or("");
                let Some(link_cap) = link_re.captures(block) else { continue };
                let slug = link_cap
                    .get(1)
                    .map(|m| m.as_str().to_string())
                    .unwrap_or_default();
                let title = link_cap
                    .get(2)
                    .map(|m| m.as_str().trim().to_string())
                    .unwrap_or_default();
                if slug.is_empty() || title.is_empty() || is_utility_page(&slug) {
                    continue;
                }
                if items.iter().any(|c: &AnimeCard| c.id == slug) {
                    continue;
                }

                let image = img_re
                    .captures(block)
                    .and_then(|c| c.get(1).map(|m| m.as_str().to_string()));
                let anime_type = type_re
                    .captures(block)
                    .and_then(|c| c.get(1).map(|m| m.as_str().trim().to_string()));
                let status = status_re
                    .captures(block)
                    .and_then(|c| c.get(1).map(|m| m.as_str().trim().to_string()));

                items.push(AnimeCard {
                    id: slug.clone(),
                    title: clean_title(&title),
                    japanese_title: None,
                    synopsis: None,
                    image,
                    banner_image: None,
                    anime_type: anime_type.or_else(|| Some("TV".to_string())),
                    status,
                    score: None,
                    rating: None,
                    genres: None,
                    total_episodes: None,
                    total_subbed: None,
                    total_dubbed: None,
                    slug: Some(slug),
                    mal_id: None,
                    view_count: None,
                    aired: None,
                    duration: None,
                    country: Some("CN".into()),
                    source: Some("animexin".to_string()),
                    has_sub: true,
                    has_dub: false,
                });
                page_items += 1;

                if items.len() >= limit {
                    break;
                }
            }

            if items.len() >= limit || page_items == 0 {
                break;
            }
            current_page += 1;
            if current_page > start_page + 9 {
                break;
            }
        }

        let total = items.len() as u32;
        let has_more = items.len() >= limit;
        Ok(PaginatedResponse {
            items,
            total,
            page: start_page as u32,
            limit: limit as u32,
            has_more,
        })
    }
}

/// Slice the HTML between a start marker and the nearest end marker.
fn extract_section_html<'a>(html: &'a str, start_marker: &str, end_markers: &[&str]) -> Option<&'a str> {
    let start = html.find(start_marker)? + start_marker.len();
    let end = end_markers
        .iter()
        .filter_map(|m| html[start..].find(m).map(|p| start + p))
        .min()
        .unwrap_or(html.len());
    Some(&html[start..end])
}

/// Parse `<article class="bs">` blocks into cards.
fn parse_article_cards(html: &str) -> Vec<AnimeCard> {
    let Some(article_re) = Regex::new(r#"(?s)<article[^>]*class="bs"[^>]*>(.*?)</article>"#).ok() else {
        return Vec::new();
    };
    let Some(link_re) =
        Regex::new(r#"<a\s+href="https://animexin\.dev/([^"]+)/"[^>]*title="([^"]+)""#).ok()
    else {
        return Vec::new();
    };
    let Some(img_re) = Regex::new(r#"<img[^>]+src="([^"]+)""#).ok() else {
        return Vec::new();
    };
    let type_re = Regex::new(r#"<div\s+class="typez\s+([^"]+)">"#).ok();
    let status_re = Regex::new(r#"<div\s+class="status\s+([^"]+)">"#).ok();

    let mut cards = Vec::new();
    for cap in article_re.captures_iter(html) {
        let block = cap.get(1).map(|m| m.as_str()).unwrap_or("");
        let Some(link_cap) = link_re.captures(block) else { continue };
        let slug = link_cap
            .get(1)
            .map(|m| m.as_str().to_string())
            .unwrap_or_default();
        let title = link_cap
            .get(2)
            .map(|m| m.as_str().trim().to_string())
            .unwrap_or_default();
        if slug.is_empty() || title.is_empty() || is_utility_page(&slug) {
            continue;
        }

        let image = img_re
            .captures(block)
            .and_then(|c| c.get(1).map(|m| m.as_str().to_string()));
        let anime_type = type_re
            .as_ref()
            .and_then(|re| re.captures(block))
            .and_then(|c| c.get(1).map(|m| m.as_str().trim().to_string()));
        let status = status_re
            .as_ref()
            .and_then(|re| re.captures(block))
            .and_then(|c| c.get(1).map(|m| m.as_str().trim().to_string()));

        cards.push(AnimeCard {
            id: slug.clone(),
            title: clean_title(&title),
            japanese_title: None,
            synopsis: None,
            image,
            banner_image: None,
            anime_type: anime_type.or_else(|| Some("TV".to_string())),
            status,
            score: None,
            rating: None,
            genres: None,
            total_episodes: None,
            total_subbed: None,
            total_dubbed: None,
            slug: Some(slug),
            mal_id: None,
            view_count: None,
            aired: None,
            duration: None,
            country: Some("CN".into()),
            source: Some("animexin".to_string()),
            has_sub: true,
            has_dub: false,
        });
    }
    cards
}

/// Parse `<li>` items from a `serieslist` block into cards.
fn parse_serieslist_cards(html: &str) -> Vec<AnimeCard> {
    let Some(li_re) = Regex::new(r#"(?s)<li>(.*?)</li>"#).ok() else {
        return Vec::new();
    };
    let Some(link_re) = Regex::new(r#"<a\s+class="series"\s+href="https://animexin\.dev/([^"]+)/"[^>]*>"#).ok()
    else {
        return Vec::new();
    };
    let Some(img_re) = Regex::new(r#"<img[^>]+src="([^"]+)""#).ok() else {
        return Vec::new();
    };
    let Some(title_re) = Regex::new(r#"<h4>[^<]*<a\s+class="series"[^>]*>([^<]+)</a>"#).ok()
    else {
        return Vec::new();
    };
    let score_re = Regex::new(r#"<div\s+class="numscore">([^<]+)</div>"#).ok();

    let mut cards = Vec::new();
    for cap in li_re.captures_iter(html) {
        let block = cap.get(1).map(|m| m.as_str()).unwrap_or("");
        let Some(link_cap) = link_re.captures(block) else { continue };
        let slug = link_cap
            .get(1)
            .map(|m| m.as_str().to_string())
            .unwrap_or_default();
        if slug.is_empty() || is_utility_page(&slug) {
            continue;
        }
        let title = title_re
            .captures(block)
            .and_then(|c| c.get(1).map(|m| m.as_str().trim().to_string()))
            .unwrap_or_else(|| clean_title(&slug));
        let image = img_re
            .captures(block)
            .and_then(|c| c.get(1).map(|m| m.as_str().to_string()));
        let score = score_re
            .as_ref()
            .and_then(|re| re.captures(block))
            .and_then(|c| c.get(1).map(|m| m.as_str().trim().to_string()))
            .and_then(|s| s.parse().ok());

        cards.push(AnimeCard {
            id: slug.clone(),
            title: clean_title(&title),
            japanese_title: None,
            synopsis: None,
            image,
            banner_image: None,
            anime_type: Some("TV".to_string()),
            status: None,
            score,
            rating: None,
            genres: None,
            total_episodes: None,
            total_subbed: None,
            total_dubbed: None,
            slug: Some(slug),
            mal_id: None,
            view_count: None,
            aired: None,
            duration: None,
            country: Some("CN".into()),
            source: Some("animexin".to_string()),
            has_sub: true,
            has_dub: false,
        });
    }
    cards
}

fn extract_iframe_src(html: &str) -> Option<String> {
    let re = Regex::new(r#"src\s*=\s*"([^"]+)""#).ok()?;
    re.captures(html)?.get(1).map(|m| m.as_str().to_string())
}

fn extract_meta_content(html: &str, pattern: &str) -> Option<String> {
    let re = Regex::new(pattern).ok()?;
    re.captures(html)?.get(1).map(|m| m.as_str().to_string())
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
    match name.to_lowercase().as_str() {
        "geo" => "DAILYMOTION".to_string(),
        "odysee" => "ODYSEE".to_string(),
        "ok" => "OKRU".to_string(),
        "play" => "DTUBE".to_string(),
        "mega" => "MEGA".to_string(),
        "rumble" => "RUMBLE".to_string(),
        "1024terabox" | "terabox" => "TERABOX-DL".to_string(),
        "mediafire" => "MEDIAFIRE-DL".to_string(),
        "mirrored" => "MIRRORED-DL".to_string(),
        "animexinfansub" => "SEEKPLAYER".to_string(),
        "streamwish" => "STREAMWISH".to_string(),
        "filelions" => "FILELIONS".to_string(),
        "wolfstream" => "WOLFSTREAM".to_string(),
        "speedostream" => "SPEEDOSTREAM".to_string(),
        _ => name.to_uppercase(),
    }
}

/// Extract episode number from slug with multiple fallback strategies.
/// Supports: `-episode-{N}-...`, `-episode-{local}-{global}-...`
fn extract_episode_number(slug: &str) -> u32 {
    // Strategy 1: dual number pattern like -episode-14-222-
    let re = Regex::new(r#"-episode-(\d+)-(\d+)[-\/]"#).ok();
    if let Some(re) = re {
        if let Some(cap) = re.captures_iter(slug).next() {
            return cap
                .get(2)
                .and_then(|m| m.as_str().parse().ok())
                .unwrap_or(0);
        }
    }

    // Strategy 2: single number pattern like -episode-100-
    let re = Regex::new(r#"-episode-(\d+)(?:-subtitle|-indonesia|-english|-sub)?"#).ok();
    if let Some(re) = re {
        if let Some(cap) = re.captures_iter(slug).next() {
            return cap
                .get(1)
                .and_then(|m| m.as_str().parse().ok())
                .unwrap_or(0);
        }
    }

    0
}

/// Clean "- AnimeXin" suffix and episode-specific junk from titles.
/// Animexin episode pages include the current episode in the page title,
/// e.g. "Shrouding the Heavens Episode 10 Indonesia, English Sub - AnimeXin".
/// Strip everything from " Episode N" onward so the series title is clean.
fn clean_title(raw: &str) -> String {
    let trimmed = raw.trim();
    // Remove trailing site suffix first.
    let without_suffix = trimmed
        .trim_end_matches("- AnimeXin")
        .trim_end_matches("– AnimeXin")
        .trim();
    // Strip " Episode N ..." tail. Case-insensitive, captures any trailing
    // language/subtitle text Animexin appends after the episode number.
    let re = Regex::new(r"(?i)\s+Episode\s+\d+.*").unwrap();
    let cleaned = re.replace(without_suffix, "").trim().to_string();
    if cleaned.is_empty() {
        trimmed.to_string()
    } else {
        cleaned
    }
}

/// Filter out non-anime utility pages.
fn is_utility_page(slug: &str) -> bool {
    let lower = slug.to_lowercase();
    lower.starts_with("dmca")
        || lower.starts_with("bookmark")
        || lower.starts_with("az-lists")
        || lower.starts_with("blog")
        || lower.starts_with("contact")
        || lower.starts_with("privacy")
        || lower.starts_with("terms")
        || lower.starts_with("login")
        || lower.starts_with("register")
        || lower.starts_with("feed")
        || lower.contains("wp-json")
        || lower.contains("wp-content")
}

/// Try to extract episode count from search result snippet.
fn extract_episode_count_from_search(snippet: &str) -> Option<u32> {
    let re = Regex::new(r#"(?i)(\d+)\s*(?:episodes?|eps?)"#).ok()?;
    re.captures(snippet)?.get(1)?.as_str().parse().ok()
}
