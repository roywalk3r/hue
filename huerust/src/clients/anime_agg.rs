use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use moka::future::Cache;
use tokio::task::JoinSet;
use tokio::time::timeout;
use tracing::warn;

use chrono::NaiveDate;

use crate::clients::anilist::AniListClient;
use crate::clients::animexin::AnimexinClient;
use crate::clients::aniwatch::AniWatchClient;
use crate::clients::aniwave::AniwaveClient;
use crate::clients::hianime::HianimeClient;
use crate::models::anime::{
    AnimeCard, AnimeDetail, AnimeHomeFeed, AnimeSearchResponse, EpisodeListResponse, FilterChoice,
    FilterOptions, FilterParams, PaginatedResponse, ScheduleEntry, ServerList, SkipRange,
    StreamPayload, SubtitleTrack,
};

/// Aggregates anime results from multiple providers.
/// Search fans out to all providers and merges results.
/// Detail/servers/stream try providers in order until one succeeds.
#[derive(Clone)]
pub struct AnimeAggregator {
    aniwave: Arc<AniwaveClient>,
    hianime: Arc<HianimeClient>,
    aniwatch: Arc<AniWatchClient>,
    anilist: Arc<AniListClient>,
    animexin: Arc<AnimexinClient>,
    /// Slug → mal_id mapping (30d TTL). Once matched, skip the
    /// hiAnime proxy search on future requests.
    mal_id_cache: Cache<String, Option<u32>>,
    /// Slug → () for unmatched slugs (1h TTL). Prevents re-searching
    /// every request without permanently blocking retries.
    mal_miss_cache: Cache<String, ()>,
}

impl AnimeAggregator {
    pub fn new(
        aniwave: Arc<AniwaveClient>,
        hianime: Arc<HianimeClient>,
        aniwatch: Arc<AniWatchClient>,
        anilist: Arc<AniListClient>,
        animexin: Arc<AnimexinClient>,
    ) -> Self {
        Self {
            aniwave,
            hianime,
            aniwatch,
            anilist,
            animexin,
            mal_id_cache: Cache::builder()
                .max_capacity(2000)
                .time_to_live(Duration::from_secs(30 * 24 * 60 * 60))
                .build(),
            mal_miss_cache: Cache::builder()
                .max_capacity(2000)
                .time_to_live(Duration::from_secs(60 * 60))
                .build(),
        }
    }

    /// Search: AniWave is the single working source.
    pub async fn search(&self, keyword: &str, page: u32, per_page: u32) -> AnimeSearchResponse {
        match self.aniwave.search(keyword, page, per_page).await {
            Ok(response) => response,
            Err(e) => {
                warn!("[AnimeAgg] AniWave search error: {}", e);
                AnimeSearchResponse {
                    query: keyword.to_string(),
                    total: 0,
                    page,
                    limit: per_page,
                    results: Vec::new(),
                }
            }
        }
    }

    /// Try each provider for detail until one succeeds.
    pub async fn get_detail(&self, id: &str) -> Option<AnimeDetail> {
        // Try Aniwave first (most reliable as of June 2026)
        if id.contains('-') && id.chars().last().unwrap_or(' ').is_ascii_digit() {
            match self.aniwave.get_detail(id).await {
                Ok(mut detail) => {
                    if !detail.card.id.is_empty() {
                        // Fire all enrichment calls in parallel with a
                        // shared deadline so the slowest doesn't hold up
                        // the response. Each call is optional; misses
                        // silently degrade.
                        let en_deadline = Duration::from_secs(5);

                        let info_fut = async {
                            if self.aniwave.mirror_base().is_some() {
                                self.aniwave.fetch_hianime_info_meta(id).await.ok()
                            } else {
                                None
                            }
                        };

                        let show_id =
                            crate::clients::aniwave::AniwaveClient::show_id_from_slug(id);
                        let ep_fut = async {
                            if self.aniwave.mirror_base().is_some() {
                                if let Some(ref sid) = show_id {
                                    return self
                                        .aniwave
                                        .fetch_hianime_episode_details(sid)
                                        .await
                                        .ok();
                                }
                            }
                            None
                        };

                        // Skip the search if we've already matched (hit
                        // cache) or recently failed (miss cache, 1h TTL).
                        let need_mal_search = detail.card.mal_id.is_none()
                            && self.mal_id_cache.get(id).await.is_none()
                            && self.mal_miss_cache.get(id).await.is_none();

                        let mal_fut = async {
                            if !need_mal_search {
                                return None;
                            }
                            let raw = self.hianime.search(&detail.card.title).await.ok()?;
                            let results =
                                crate::clients::hianime::transform_search_results(
                                    raw,
                                    &detail.card.title,
                                );
                            let title_lower = detail.card.title.to_lowercase();
                            let jp_lower = detail
                                .card
                                .japanese_title
                                .as_deref()
                                .unwrap_or("")
                                .to_lowercase();
                            results.results.into_iter().find(|c| {
                                let c_lower = c.title.to_lowercase();
                                c_lower == title_lower
                                    || c_lower.starts_with(&title_lower)
                                    || title_lower.starts_with(&c_lower)
                                    || (!jp_lower.is_empty()
                                        && (c_lower == jp_lower
                                            || c_lower.starts_with(&jp_lower)))
                                    || token_overlap(&c_lower, &title_lower) >= 3
                            })
                        };

                        let (meta, rich_eps, mal_match) = tokio::join!(
                            async { timeout(en_deadline, info_fut).await.unwrap_or(None) },
                            async { timeout(en_deadline, ep_fut).await.unwrap_or(None) },
                            async { timeout(en_deadline, mal_fut).await.unwrap_or(None) },
                        );

                        if let Some(meta) = meta {
                            detail.card.synopsis =
                                detail.card.synopsis.or(meta.description);
                            let og = meta.og_image.clone();
                            detail.card.banner_image =
                                detail.card.banner_image.or(og);
                            if detail.card.image.is_none() {
                                detail.card.image = meta.og_image;
                            }
                            if detail.card.genres.is_none()
                                && !meta.keywords.is_empty()
                            {
                                detail.card.genres = Some(meta.keywords);
                            }
                        }
                        if let Some(rich) = rich_eps.as_ref() {
                            if let Some(episodes) = detail.episodes.as_mut() {
                                crate::clients::aniwave::AniwaveClient::enrich_episodes_v2(rich, episodes);
                            }
                        }
                        // Apply cached mal_id even when we skipped the search.
                        if detail.card.mal_id.is_none() {
                            if let Some(Some(cached)) =
                                self.mal_id_cache.get(id).await
                            {
                                detail.card.mal_id = Some(cached);
                            }
                        }
                        if let Some(best) = mal_match {
                            detail.card.mal_id = best.mal_id;
                            detail.card.score = detail.card.score.or(best.score);
                            detail.card.rating =
                                detail.card.rating.or(best.rating);
                            detail.card.aired = detail.card.aired.or(best.aired);
                            detail.card.duration =
                                detail.card.duration.or(best.duration);
                            if detail.card.genres.is_none() {
                                detail.card.genres = best.genres;
                            }
                            // Cache the mal_id for this slug so subsequent
                            // detail requests skip the search entirely.
                            self.mal_id_cache
                                .insert(id.to_string(), best.mal_id)
                                .await;
                        } else if need_mal_search {
                            // Short-TTL negative cache — re-search in 1h.
                            self.mal_miss_cache
                                .insert(id.to_string(), ())
                                .await;
                        }
                        return Some(detail);
                    }
                }
                Err(e) => warn!("[AnimeAgg] Aniwave detail error for {}: {}", id, e),
            }
        }

        // Try HiAnime
        if let Ok(info) = self.hianime.get_anime_info(id).await {
            let detail = crate::clients::hianime::transform_anime_detail(info);
            if !detail.card.id.is_empty() {
                return Some(detail);
            }
        }

        // Try Animexin for WordPress slug-based series pages
        if id.contains('-') && !id.contains('/') {
            if let Ok(detail) = self.animexin.get_detail(id).await {
                if !detail.card.id.is_empty() {
                    return Some(detail);
                }
            }
        }

        None
    }

    /// Get episodes: prefer Aniwave, then HiAnime, then AniList (synthetic)
    pub async fn get_episodes(
        &self,
        id: &str,
        start: u32,
    ) -> Option<Vec<crate::models::anime::EpisodeSummary>> {
        // Try Aniwave first (slugs end in numeric id like one-piece-81553)
        if id.contains('-') && id.chars().last().unwrap_or(' ').is_ascii_digit() {
            match self.aniwave.get_episodes(id).await {
                Ok(eps) => {
                    if !eps.is_empty() {
                        return Some(eps);
                    }
                }
                Err(e) => warn!("[AnimeAgg] Aniwave episodes error for {}: {}", id, e),
            }
        }

        // Try HiAnime
        if let Ok(ep_data) = self.hianime.get_episodes(id, start).await {
            let episodes: Vec<crate::models::anime::EpisodeSummary> = ep_data
                .episodes
                .unwrap_or_default()
                .into_iter()
                .map(|ep| crate::models::anime::EpisodeSummary {
                    id: ep._id,
                    number: ep.episodeNumber.unwrap_or(0),
                    title: None,
                    sub_count: ep
                        .link
                        .as_ref()
                        .and_then(|l| l.sub.as_ref())
                        .map(|v| v.len() as u32)
                        .unwrap_or(0),
                    dub_count: ep
                        .link
                        .as_ref()
                        .and_then(|l| l.dub.as_ref())
                        .map(|v| v.len() as u32)
                        .unwrap_or(0),
                    slug: ep.slugs.as_ref().and_then(|s| s.first().cloned()),
                    series_slug: ep.slugs.as_ref().and_then(|s| s.first().cloned()),
                    slugs: ep.slugs,
                })
                .collect();
            if !episodes.is_empty() {
                return Some(episodes);
            }
        }

        // AniList synthetic episodes
        if let Ok(detail) = self.anilist.get_detail(id).await {
            if let Some(eps) = detail.episodes {
                if !eps.is_empty() {
                    return Some(eps);
                }
            }
        }

        // Animexin donghua scraper (season page → episode list)
        if let Ok(eps) = self.animexin.get_episodes(id).await {
            if !eps.is_empty() {
                return Some(eps);
            }
        }

        None
    }

    /// Get servers: try Aniwave, then HiAnime, then Animexin
    pub async fn get_servers(
        &self,
        id: &str,
        episode: Option<u32>,
        audio_type: Option<&str>,
    ) -> Option<ServerList> {
        if id.contains('-') && id.chars().last().unwrap_or(' ').is_ascii_digit() {
            let ep = episode.unwrap_or(1);
            let audio = audio_type.unwrap_or("");
            if audio == "sub" || audio == "dub" {
                match self.aniwave.get_servers(id, ep, audio).await {
                    Ok(servers) if !servers.sub.is_empty() || !servers.dub.is_empty() => {
                        return Some(servers);
                    }
                    Ok(_) => {}
                    Err(e) => warn!(
                        "[AnimeAgg] Aniwave servers error for {} ep={} type={}: {}",
                        id, ep, audio, e
                    ),
                }
            } else {
                let sub = self.aniwave.get_servers(id, ep, "sub").await.ok();
                let dub = self.aniwave.get_servers(id, ep, "dub").await.ok();
                let servers = ServerList {
                    sub: sub.map(|s| s.sub).unwrap_or_default(),
                    dub: dub.map(|s| s.dub).unwrap_or_default(),
                };
                if !servers.sub.is_empty() || !servers.dub.is_empty() {
                    return Some(servers);
                }
            }
        }

        // HiAnime
        if let Ok(raw) = self.hianime.get_servers(id).await {
            let structured = crate::clients::hianime::transform_server_list(&raw);
            if !structured.sub.is_empty() || !structured.dub.is_empty() {
                return Some(structured);
            }
        }

        // Animexin (donghua WordPress scraper)
        if let Ok(servers) = self.animexin.get_servers(id).await {
            if !servers.sub.is_empty() || !servers.dub.is_empty() {
                return Some(servers);
            }
        }

        None
    }

    /// Get stream: try Aniwave, then HiAnime, then Animexin
    pub async fn get_stream(
        &self,
        id: &str,
        server: Option<&str>,
        link_type: &str,
        episode: u32,
    ) -> Option<StreamPayload> {
        // Aniwave: id is the series slug (e.g. "one-piece-81553"),
        // episode is the episode number. Skip if id doesn't look like a slug.
        if id.contains('-') && id.chars().last().unwrap_or(' ').is_ascii_digit() {
            let server_id = server.and_then(|s| s.parse::<u32>().ok());
            match self
                .aniwave
                .get_stream(id, episode, server_id, link_type)
                .await
            {
                Ok(stream) => {
                    if !stream.url.is_empty() {
                        return Some(stream);
                    }
                }
                Err(e) => warn!(
                    "[AnimeAgg] Aniwave stream error for {} ep={} type={}: {}",
                    id, episode, link_type, e
                ),
            }
        }

        // HiAnime fallback when the slug is HiAnime-shaped (24-hex id or path slug).
        if let Ok(data) = self
            .hianime
            .get_stream(id, server, link_type, episode)
            .await
        {
            if !data.link.file.is_empty() {
                return Some(stream_data_to_payload(data));
            }
        }

        // Do not fall back to Animexin: it only returns iframe embeds
        // (ok.ru, fembed, dood) that the mobile player cannot play.
        None
    }

    /// Home feed: use AniWave as the single source of truth.
    pub async fn get_home(&self) -> AnimeHomeFeed {
        let mut feed = AnimeHomeFeed {
            trending: None,
            currently_airing: None,
            featured: None,
            latest: None,
            popular: None,
            finished_airing: None,
        };

        if let Ok(aniwave_feed) = self.aniwave.get_home().await {
            feed.trending = merge_optional_vecs(feed.trending, aniwave_feed.trending);
            feed.latest = merge_optional_vecs(feed.latest, aniwave_feed.latest);
            feed.popular = merge_optional_vecs(feed.popular, aniwave_feed.popular);
            feed.featured = merge_optional_vecs(feed.featured, aniwave_feed.featured);
            feed.currently_airing =
                merge_optional_vecs(feed.currently_airing, aniwave_feed.currently_airing);
        }

        // Fallback: use AniWave discover pipeline to backfill any empty section.
        if feed.trending.is_none() {
            let resp = self
                .discover(&FilterParams {
                    sort: Some("trending".into()),
                    limit: Some(12),
                    ..Default::default()
                })
                .await;
            if !resp.items.is_empty() {
                feed.trending = Some(resp.items);
            }
        }
        if feed.popular.is_none() {
            let resp = self
                .discover(&FilterParams {
                    sort: Some("popularity".into()),
                    limit: Some(12),
                    ..Default::default()
                })
                .await;
            if !resp.items.is_empty() {
                feed.popular = Some(resp.items);
            }
        }
        if feed.finished_airing.is_none() {
            let resp = self
                .discover(&FilterParams {
                    status: Some("finished".into()),
                    sort: Some("score".into()),
                    limit: Some(12),
                    ..Default::default()
                })
                .await;
            if !resp.items.is_empty() {
                feed.finished_airing = Some(resp.items);
            }
        }

        feed
    }

    /// Discover / browse: AniWave is the single source.
    /// No AniList/AniWatch/HiAnime fan-out, so every returned card has a
    /// source-backed slug and is playable through the AniWave stream path.
    pub async fn discover(&self, params: &FilterParams) -> PaginatedResponse<AnimeCard> {
        match self.aniwave.discover(params).await {
            Ok(resp) => PaginatedResponse {
                items: resp.items,
                total: resp.total,
                page: resp.page,
                limit: resp.limit,
                has_more: resp.has_more,
            },
            Err(e) => {
                warn!("[AnimeAgg] AniWave discover error: {}", e);
                PaginatedResponse {
                    items: Vec::new(),
                    total: 0,
                    page: params.page.unwrap_or(1),
                    limit: params.limit.unwrap_or(20),
                    has_more: false,
                }
            }
        }
    }

    /// Static metadata for the discover screen. Years are generated at
    /// call time so the current year is always present.
    pub async fn filter_options(&self) -> FilterOptions {
        let current_year = chrono::Utc::now()
            .format("%Y")
            .to_string()
            .parse::<u32>()
            .unwrap_or(2026);
        let years: Vec<FilterChoice> = (1960..=current_year)
            .rev()
            .map(|y| FilterChoice {
                value: y.to_string(),
                label: y.to_string(),
            })
            .collect();
        FilterOptions {
            types: vec![
                FilterChoice {
                    value: "tv".into(),
                    label: "TV".into(),
                },
                FilterChoice {
                    value: "movie".into(),
                    label: "Movie".into(),
                },
                FilterChoice {
                    value: "ova".into(),
                    label: "OVA".into(),
                },
                FilterChoice {
                    value: "ona".into(),
                    label: "ONA".into(),
                },
                FilterChoice {
                    value: "special".into(),
                    label: "Special".into(),
                },
                FilterChoice {
                    value: "music".into(),
                    label: "Music".into(),
                },
            ],
            statuses: vec![
                FilterChoice {
                    value: "airing".into(),
                    label: "Currently Airing".into(),
                },
                FilterChoice {
                    value: "finished".into(),
                    label: "Finished Airing".into(),
                },
                FilterChoice {
                    value: "upcoming".into(),
                    label: "Not Yet Aired".into(),
                },
            ],
            sorts: vec![
                FilterChoice {
                    value: "popularity".into(),
                    label: "Popularity".into(),
                },
                FilterChoice {
                    value: "score".into(),
                    label: "Score".into(),
                },
                FilterChoice {
                    value: "trending".into(),
                    label: "Trending".into(),
                },
                FilterChoice {
                    value: "latest".into(),
                    label: "Latest".into(),
                },
                FilterChoice {
                    value: "alphabetical".into(),
                    label: "A–Z".into(),
                },
            ],
            genres: ANILIST_GENRES
                .iter()
                .map(|(v, l)| FilterChoice {
                    value: (*v).into(),
                    label: (*l).into(),
                })
                .collect(),
            years,
            seasons: vec![
                FilterChoice {
                    value: "winter".into(),
                    label: "Winter".into(),
                },
                FilterChoice {
                    value: "spring".into(),
                    label: "Spring".into(),
                },
                FilterChoice {
                    value: "summer".into(),
                    label: "Summer".into(),
                },
                FilterChoice {
                    value: "fall".into(),
                    label: "Fall".into(),
                },
            ],
            languages: vec![
                FilterChoice {
                    value: "japanese".into(),
                    label: "Japanese".into(),
                },
                FilterChoice {
                    value: "english".into(),
                    label: "English".into(),
                },
                FilterChoice {
                    value: "chinese".into(),
                    label: "Chinese".into(),
                },
                FilterChoice {
                    value: "korean".into(),
                    label: "Korean".into(),
                },
            ],
            countries: vec![
                FilterChoice {
                    value: "CN".into(),
                    label: "China".into(),
                },
                FilterChoice {
                    value: "JP".into(),
                    label: "Japan".into(),
                },
                FilterChoice {
                    value: "KR".into(),
                    label: "Korea".into(),
                },
            ],
            scores: vec![
                FilterChoice {
                    value: "all".into(),
                    label: "All".into(),
                },
                FilterChoice {
                    value: "8".into(),
                    label: "8+".into(),
                },
                FilterChoice {
                    value: "7".into(),
                    label: "7+".into(),
                },
                FilterChoice {
                    value: "6".into(),
                    label: "6+".into(),
                },
                FilterChoice {
                    value: "5".into(),
                    label: "5+".into(),
                },
            ],
            ratings: vec![
                FilterChoice {
                    value: "g".into(),
                    label: "G".into(),
                },
                FilterChoice {
                    value: "pg".into(),
                    label: "PG".into(),
                },
                FilterChoice {
                    value: "pg13".into(),
                    label: "PG-13".into(),
                },
                FilterChoice {
                    value: "r".into(),
                    label: "R".into(),
                },
                FilterChoice {
                    value: "rplus".into(),
                    label: "R+".into(),
                },
                FilterChoice {
                    value: "rx".into(),
                    label: "Rx".into(),
                },
            ],
        }
    }

    /// Airings for a single date. Currently AniList only; other providers
    /// don't expose a schedule API.
    pub async fn schedule(&self, date: Option<NaiveDate>, _tz: Option<&str>) -> Vec<ScheduleEntry> {
        match self.anilist.schedule(date).await {
            Ok(v) => v,
            Err(e) => {
                warn!("[AnimeAgg] schedule error: {}", e);
                Vec::new()
            }
        }
    }

    /// Build a typed episode list response for the route handler.
    pub async fn episodes_response(&self, id: &str, start: u32) -> Option<EpisodeListResponse> {
        let episodes = self.get_episodes(id, start).await?;
        let total = episodes.len() as u32;
        Some(EpisodeListResponse {
            anime_id: id.to_string(),
            start,
            total,
            episodes,
        })
    }
}

/// Canonical AniList genre slugs. Source: https://anilist.co/genre
const ANILIST_GENRES: &[(&str, &str)] = &[
    ("Action", "Action"),
    ("Adventure", "Adventure"),
    ("Cars", "Cars"),
    ("Comedy", "Comedy"),
    ("Dementia", "Dementia"),
    ("Demons", "Demons"),
    ("Drama", "Drama"),
    ("Ecchi", "Ecchi"),
    ("Fantasy", "Fantasy"),
    ("Game", "Game"),
    ("Harem", "Harem"),
    ("Historical", "Historical"),
    ("Horror", "Horror"),
    ("Isekai", "Isekai"),
    ("Josei", "Josei"),
    ("Kids", "Kids"),
    ("Magic", "Magic"),
    ("Martial Arts", "Martial Arts"),
    ("Mecha", "Mecha"),
    ("Military", "Military"),
    ("Music", "Music"),
    ("Mystery", "Mystery"),
    ("Parody", "Parody"),
    ("Police", "Police"),
    ("Psychological", "Psychological"),
    ("Romance", "Romance"),
    ("Samurai", "Samurai"),
    ("School", "School"),
    ("Sci-Fi", "Sci-Fi"),
    ("Seinen", "Seinen"),
    ("Shoujo", "Shoujo"),
    ("Shoujo Ai", "Shoujo Ai"),
    ("Shounen", "Shounen"),
    ("Shounen Ai", "Shounen Ai"),
    ("Slice of Life", "Slice of Life"),
    ("Space", "Space"),
    ("Sports", "Sports"),
    ("Super Power", "Super Power"),
    ("Supernatural", "Supernatural"),
    ("Thriller", "Thriller"),
    ("Vampire", "Vampire"),
];

fn normalize_title(title: &str) -> String {
    title.to_lowercase().trim().to_string()
}

fn token_overlap(a: &str, b: &str) -> usize {
    let a_tokens: std::collections::HashSet<&str> =
        a.split_whitespace().collect();
    b.split_whitespace()
        .filter(|t| a_tokens.contains(t))
        .count()
}

fn search_match_rank(title: &str, normalized_query: &str) -> u8 {
    let title = normalize_title(title);
    if title == normalized_query {
        return 0;
    }
    if title.starts_with(normalized_query) {
        return 1;
    }
    if title.contains(normalized_query) {
        return 2;
    }
    3
}

fn is_aniwave_slug(slug: &str) -> bool {
    slug.rsplit('-')
        .next()
        .map(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_digit()))
        .unwrap_or(false)
}

fn source_rank(card: &AnimeCard) -> u8 {
    if card.slug.as_deref().map(is_aniwave_slug).unwrap_or(false) {
        return 3;
    }
    if card.has_sub || card.has_dub {
        return 2;
    }
    card.has_sub
        .then_some(1)
        .or_else(|| {
            card.slug
                .as_deref()
                .filter(|slug| !slug.starts_with("mal-"))
                .map(|_| 1)
        })
        .unwrap_or(0)
}

fn should_prefer_card(existing: &AnimeCard, candidate: &AnimeCard) -> bool {
    source_rank(candidate) > source_rank(existing)
}

fn merge_cards(mut primary: AnimeCard, secondary: AnimeCard) -> AnimeCard {
    if primary.japanese_title.is_none() {
        primary.japanese_title = secondary.japanese_title;
    }
    if primary.synopsis.is_none() {
        primary.synopsis = secondary.synopsis;
    }
    if primary.image.is_none() {
        primary.image = secondary.image;
    }
    if primary.banner_image.is_none() {
        primary.banner_image = secondary.banner_image;
    }
    if primary.anime_type.is_none() {
        primary.anime_type = secondary.anime_type;
    }
    if primary.status.is_none() {
        primary.status = secondary.status;
    }
    if primary.score.is_none() {
        primary.score = secondary.score;
    }
    if primary.rating.is_none() {
        primary.rating = secondary.rating;
    }
    if primary.genres.is_none() {
        primary.genres = secondary.genres;
    }
    if primary.total_episodes.is_none() {
        primary.total_episodes = secondary.total_episodes;
    }
    if primary.total_subbed.is_none() {
        primary.total_subbed = secondary.total_subbed;
    }
    if primary.total_dubbed.is_none() {
        primary.total_dubbed = secondary.total_dubbed;
    }
    let primary_metadata_slug = primary
        .slug
        .as_deref()
        .map(|slug| slug.starts_with("mal-"))
        .unwrap_or(true);
    let secondary_playable_slug = secondary
        .slug
        .as_deref()
        .map(|slug| !slug.starts_with("mal-"))
        .unwrap_or(false);
    if primary_metadata_slug && secondary_playable_slug {
        primary.slug = secondary.slug;
        primary.id = secondary.id;
    }
    if primary.mal_id.is_none() {
        primary.mal_id = secondary.mal_id;
    }
    if primary.view_count.is_none() {
        primary.view_count = secondary.view_count;
    }
    if primary.aired.is_none() {
        primary.aired = secondary.aired;
    }
    if primary.duration.is_none() {
        primary.duration = secondary.duration;
    }
    primary.has_sub = primary.has_sub || secondary.has_sub;
    primary.has_dub = primary.has_dub || secondary.has_dub;
    primary
}

fn merge_optional_vecs(
    a: Option<Vec<AnimeCard>>,
    b: Option<Vec<AnimeCard>>,
) -> Option<Vec<AnimeCard>> {
    match (a, b) {
        (Some(mut av), Some(bv)) => {
            let mut seen = std::collections::HashSet::new();
            for card in &av {
                seen.insert(normalize_title(&card.title));
            }
            for card in bv {
                let key = normalize_title(&card.title);
                if seen.insert(key.clone()) {
                    av.push(card);
                } else if let Some(existing) =
                    av.iter_mut().find(|c| normalize_title(&c.title) == key)
                {
                    if should_prefer_card(existing, &card) {
                        *existing = merge_cards(card, existing.clone());
                    }
                }
            }
            Some(av)
        }
        (a, None) => a,
        (None, b) => b,
    }
}

/// Convert HiAnime StreamData into the shared StreamPayload format.
fn stream_data_to_payload(data: crate::clients::hianime::StreamData) -> StreamPayload {
    StreamPayload {
        url: data.link.file,
        stream_type: data.link.link_type,
        subtitles: data.tracks.map(|tracks| {
            tracks
                .into_iter()
                .map(|t| SubtitleTrack {
                    file: t.file,
                    label: t.label,
                    kind: t.kind,
                    default: t.default,
                })
                .collect()
        }),
        intro: data.intro.map(|i| SkipRange {
            start: i.start,
            end: i.end,
        }),
        outro: data.outro.map(|o| SkipRange {
            start: o.start,
            end: o.end,
        }),
        server: data.server,
        referer: data.referer,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_card(id: &str, title: &str, slug: &str) -> AnimeCard {
        AnimeCard {
            id: id.to_string(),
            title: title.to_string(),
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
            slug: Some(slug.to_string()),
            mal_id: None,
            view_count: None,
            aired: None,
            duration: None,
            country: None,
            has_sub: false,
            has_dub: false,
        }
    }

    #[test]
    fn merge_keeps_aniwave_slug_over_mal() {
        let mut a = test_card("1", "One Piece", "mal-21");
        a.has_sub = false;
        a.has_dub = false;
        let b = test_card("2", "One Piece", "one-piece-81553");
        let merged = merge_cards(a, b);
        assert_eq!(merged.slug.as_deref(), Some("one-piece-81553"));
        assert_eq!(merged.id, "2");
    }

    #[test]
    fn source_rank_prefers_aniwave() {
        let wave = test_card("1", "A", "a-1");
        let mal = test_card("2", "A", "mal-1");
        assert!(source_rank(&wave) > source_rank(&mal));
    }
}
