use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
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
        }
    }

    /// Search across all providers in parallel, merge and deduplicate by title.
    pub async fn search(&self, keyword: &str, page: u32, per_page: u32) -> AnimeSearchResponse {
        let mut tasks = JoinSet::new();

        let aniwave = self.aniwave.clone();
        let kw = keyword.to_string();
        let pp = per_page;
        tasks.spawn(async move {
            match aniwave.search(&kw, page, pp).await {
                Ok(response) => Some(response),
                Err(e) => {
                    warn!("[AnimeAgg] Aniwave search error: {}", e);
                    None
                }
            }
        });

        let hi = self.hianime.clone();
        let kw = keyword.to_string();
        tasks.spawn(async move {
            match hi.search(&kw).await {
                Ok(raw) => {
                    let structured = crate::clients::hianime::transform_search_results(raw, &kw);
                    Some(structured)
                }
                Err(e) => {
                    warn!("[AnimeAgg] HiAnime search error: {}", e);
                    None
                }
            }
        });

        let aw = self.aniwatch.clone();
        let kw = keyword.to_string();
        tasks.spawn(async move {
            match timeout(Duration::from_secs(8), aw.search(&kw)).await {
                Ok(Ok(r)) => Some(r),
                Ok(Err(e)) => {
                    warn!("[AnimeAgg] AniWatch search error: {}", e);
                    None
                }
                Err(_) => {
                    warn!("[AnimeAgg] AniWatch search timed out");
                    None
                }
            }
        });

        let al = self.anilist.clone();
        let kw = keyword.to_string();
        let pp = per_page;
        tasks.spawn(async move {
            match al.search(&kw, page, pp).await {
                Ok(r) => Some(r),
                Err(e) => {
                    warn!("[AnimeAgg] AniList search error: {}", e);
                    None
                }
            }
        });

        let ax = self.animexin.clone();
        let kw = keyword.to_string();
        tasks.spawn(async move {
            match ax.search(&kw).await {
                Ok(r) => Some(r),
                Err(e) => {
                    warn!("[AnimeAgg] Animexin search error: {}", e);
                    None
                }
            }
        });

        let mut seen = HashMap::<String, AnimeCard>::new();
        while let Some(Ok(opt)) = tasks.join_next().await {
            if let Some(response) = opt {
                for card in response.results {
                    let key = normalize_title(&card.title);
                    // Prefer results with images and episode counts
                    if let Some(existing) = seen.get_mut(&key) {
                        if should_prefer_card(existing, &card) {
                            *existing = merge_cards(card, existing.clone());
                            continue;
                        }
                        if existing.image.is_none() && card.image.is_some() {
                            existing.image = card.image.clone();
                        }
                        if existing.total_episodes.is_none() && card.total_episodes.is_some() {
                            existing.total_episodes = card.total_episodes;
                        }
                        if !existing.has_sub && card.has_sub {
                            existing.has_sub = true;
                        }
                        if !existing.has_dub && card.has_dub {
                            existing.has_dub = true;
                        }
                        // Merge slugs for fallback lookup
                        if existing.slug.is_none() && card.slug.is_some() {
                            existing.slug = card.slug.clone();
                        }
                    } else {
                        seen.insert(key, card);
                    }
                }
            }
        }

        let mut results: Vec<AnimeCard> = seen.into_values().collect();
        let normalized_query = normalize_title(keyword);
        results.sort_by(|a, b| {
            search_match_rank(&a.title, &normalized_query)
                .cmp(&search_match_rank(&b.title, &normalized_query))
                .then_with(|| source_rank(b).cmp(&source_rank(a)))
                .then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase()))
        });
        let total = results.len();
        let start = page.saturating_sub(1).saturating_mul(per_page) as usize;
        let results = results
            .into_iter()
            .skip(start)
            .take(per_page as usize)
            .collect();

        AnimeSearchResponse {
            query: keyword.to_string(),
            total,
            page,
            limit: per_page,
            results,
        }
    }

    /// Try each provider for detail until one succeeds.
    pub async fn get_detail(&self, id: &str) -> Option<AnimeDetail> {
        // Try Aniwave first (most reliable as of June 2026)
        if id.contains('-') && id.chars().last().unwrap_or(' ').is_ascii_digit() {
            match self.aniwave.get_detail(id).await {
                Ok(detail) => {
                    if !detail.card.id.is_empty() {
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

        // HiAnime first (most reliable streams)
        if let Ok(data) = self
            .hianime
            .get_stream(id, server, link_type, episode)
            .await
        {
            return Some(stream_data_to_payload(data));
        }

        // Animexin fallback (iframe embeds)
        if let Ok(stream) = self.animexin.get_stream(id, server).await {
            return Some(stream);
        }

        None
    }

    /// Home feed: merge HiAnime + AniWatch trending/latest/popular
    pub async fn get_home(&self) -> AnimeHomeFeed {
        let mut feed = AnimeHomeFeed {
            trending: None,
            currently_airing: None,
            featured: None,
            latest: None,
            popular: None,
            finished_airing: None,
        };

        // AniWatch home (merge into same fields) — fast, no upstream scrape
        if let Ok(aw_feed) = self.aniwatch.get_home().await {
            feed.trending = merge_optional_vecs(feed.trending, aw_feed.trending);
            feed.latest = merge_optional_vecs(feed.latest, aw_feed.latest);
            feed.popular = merge_optional_vecs(feed.popular, aw_feed.popular);
        }

        // Animexin home (latest episodes + trending series)
        if let Ok(ax_feed) = self.animexin.get_home().await {
            feed.latest = merge_optional_vecs(feed.latest, ax_feed.latest);
            feed.trending = merge_optional_vecs(feed.trending, ax_feed.trending);
        }

        // Aniwave home/list widgets provide stronger poster/sub/dub counts.
        if let Ok(aniwave_feed) = self.aniwave.get_home().await {
            feed.trending = merge_optional_vecs(feed.trending, aniwave_feed.trending);
            feed.latest = merge_optional_vecs(feed.latest, aniwave_feed.latest);
            feed.popular = merge_optional_vecs(feed.popular, aniwave_feed.popular);
            feed.featured = merge_optional_vecs(feed.featured, aniwave_feed.featured);
            feed.currently_airing =
                merge_optional_vecs(feed.currently_airing, aniwave_feed.currently_airing);
        }

        feed
    }

    /// Discover / browse using the typed `FilterParams`. Fans out across
    /// providers that support the requested filter. AniList is the spine
    /// (supports the most filters); AniWatch and HiAnime backfill when
    /// their individual filters match. The result is normalized, deduped
    /// by title, and paginated locally.
    pub async fn discover(&self, params: &FilterParams) -> PaginatedResponse<AnimeCard> {
        let page = params.page.unwrap_or(1).max(1) as usize;
        let limit = params.limit.unwrap_or(20).clamp(1, 50) as usize;
        let want_more = (page * limit) + 1; // over-fetch to know if there's another page
        let want_more_u32 = want_more as u32;

        let mut tasks: JoinSet<Vec<AnimeCard>> = JoinSet::new();

        // AniList: always try, supports the most filters
        let al = self.anilist.clone();
        let p = params.clone();
        tasks.spawn(async move { al.discover(&p).await.unwrap_or_default() });

        // Aniwave: useful for frontend-ready browse pages and source-backed slugs.
        let aniwave = self.aniwave.clone();
        let p = params.clone();
        tasks.spawn(async move {
            aniwave
                .discover(&p)
                .await
                .map(|resp| resp.items)
                .unwrap_or_default()
        });

        // AniWatch: only when type/genre/status is set
        if params.r#type.is_some() || params.genre.is_some() || params.status.is_some() {
            let aw = self.aniwatch.clone();
            let p = params.clone();
            tasks.spawn(async move {
                match timeout(Duration::from_secs(8), aw.discover(&p)).await {
                    Ok(Ok(cards)) => cards,
                    Ok(Err(e)) => {
                        warn!("[AnimeAgg] AniWatch discover error: {}", e);
                        Vec::new()
                    }
                    Err(_) => {
                        warn!("[AnimeAgg] AniWatch discover timed out");
                        Vec::new()
                    }
                }
            });
        }

        // HiAnime: only for type filter (mapped to category)
        if let Some(typ) = params.r#type.clone() {
            let hi = self.hianime.clone();
            tasks.spawn(async move {
                hi.get_by_category(&typ, 1, want_more_u32)
                    .await
                    .ok()
                    .map(crate::clients::hianime::transform_category_results)
                    .unwrap_or_default()
            });
        }

        // Animexin: Chinese (donghua) source. Always included because it's
        // the only upstream that populates the `country` field.
        let ax = self.animexin.clone();
        tasks.spawn(async move {
            ax.get_home()
                .await
                .ok()
                .map(|feed| {
                    feed.latest
                        .unwrap_or_default()
                        .into_iter()
                        .chain(feed.featured.unwrap_or_default())
                        .chain(feed.popular.unwrap_or_default())
                        .chain(feed.trending.unwrap_or_default())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        });

        let mut seen: HashMap<String, AnimeCard> = HashMap::new();
        while let Some(Ok(cards)) = tasks.join_next().await {
            for card in cards {
                let key = normalize_title(&card.title);
                if key.is_empty() {
                    continue;
                }
                if let Some(existing) = seen.get_mut(&key) {
                    if should_prefer_card(existing, &card) {
                        *existing = merge_cards(card, existing.clone());
                        continue;
                    }
                    // Prefer items with images + episode counts
                    if existing.image.is_none() && card.image.is_some() {
                        existing.image = card.image.clone();
                    }
                    if existing.total_episodes.is_none() && card.total_episodes.is_some() {
                        existing.total_episodes = card.total_episodes;
                    }
                    if !existing.has_sub && card.has_sub {
                        existing.has_sub = true;
                    }
                    if !existing.has_dub && card.has_dub {
                        existing.has_dub = true;
                    }
                } else {
                    seen.insert(key, card);
                }
            }
        }

        let mut items: Vec<AnimeCard> = seen.into_values().collect();

        // Country is a post-merge filter because most upstreams don't expose it.
        if let Some(want) = params.country.as_deref() {
            let want_upper = want.to_ascii_uppercase();
            items.retain(|c| {
                c.country
                    .as_deref()
                    .map(|v| v.to_ascii_uppercase() == want_upper)
                    .unwrap_or(false)
            });
        }

        items.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
        let total = items.len() as u32;
        let start = ((page - 1) * limit).min(items.len());
        let end = (start + limit).min(items.len());
        let page_items = items[start..end].to_vec();
        let has_more = end < items.len();
        PaginatedResponse {
            items: page_items,
            total,
            page: page as u32,
            limit: limit as u32,
            has_more,
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
    fn ranks_exact_search_match_before_partial_matches() {
        let query = normalize_title("one piece");

        assert_eq!(search_match_rank("One Piece", &query), 0);
        assert_eq!(
            search_match_rank("One Piece: Episode of East Blue", &query),
            1
        );
        assert_eq!(search_match_rank("Koisuru ONE PIECE", &query), 2);
        assert_eq!(search_match_rank("Naruto", &query), 3);
    }

    #[test]
    fn merge_prefers_playable_slug_over_mal_metadata_slug() {
        let metadata = test_card("21", "ONE PIECE", "mal-21");
        let playable = test_card("81553", "One Piece", "one-piece-81553");

        let merged = merge_cards(metadata, playable);

        assert_eq!(merged.id, "81553");
        assert_eq!(merged.slug.as_deref(), Some("one-piece-81553"));
    }
}
