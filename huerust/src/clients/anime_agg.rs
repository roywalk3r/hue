use std::collections::HashMap;
use std::sync::Arc;
use tokio::task::JoinSet;
use tracing::warn;

use crate::clients::hianime::HianimeClient;
use crate::clients::aniwatch::AniWatchClient;
use crate::clients::anilist::AniListClient;
use crate::clients::animexin::AnimexinClient;
use crate::models::anime::{AnimeCard, AnimeDetail, AnimeHomeFeed, AnimeSearchResponse, ServerList, StreamPayload, SubtitleTrack, SkipRange};

/// Aggregates anime results from multiple providers.
/// Search fans out to all providers and merges results.
/// Detail/servers/stream try providers in order until one succeeds.
#[derive(Clone)]
pub struct AnimeAggregator {
    hianime: Arc<HianimeClient>,
    aniwatch: Arc<AniWatchClient>,
    anilist: Arc<AniListClient>,
    animexin: Arc<AnimexinClient>,
}

impl AnimeAggregator {
    pub fn new(
        hianime: Arc<HianimeClient>,
        aniwatch: Arc<AniWatchClient>,
        anilist: Arc<AniListClient>,
        animexin: Arc<AnimexinClient>,
    ) -> Self {
        Self { hianime, aniwatch, anilist, animexin }
    }

    /// Search across all providers in parallel, merge and deduplicate by title.
    pub async fn search(&self, keyword: &str, page: u32, per_page: u32) -> AnimeSearchResponse {
        let mut tasks = JoinSet::new();

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
            match aw.search(&kw).await {
                Ok(r) => Some(r),
                Err(e) => {
                    warn!("[AnimeAgg] AniWatch search error: {}", e);
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
        // Sort by title
        results.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));

        AnimeSearchResponse {
            query: keyword.to_string(),
            total: results.len(),
            page,
            limit: per_page,
            results,
        }
    }

    /// Try each provider for detail until one succeeds.
    pub async fn get_detail(&self, id: &str) -> Option<AnimeDetail> {
        // Try HiAnime first (has most episode data)
        if let Ok(info) = self.hianime.get_anime_info(id).await {
            let detail = crate::clients::hianime::transform_anime_detail(info);
            if !detail.card.id.is_empty() {
                return Some(detail);
            }
        }

        // Try AniWatch
        if let Ok(detail) = self.aniwatch.get_detail(id).await {
            return Some(detail);
        }

        // Try AniList (metadata only)
        if let Ok(detail) = self.anilist.get_detail(id).await {
            return Some(detail);
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

    /// Get episodes: prefer HiAnime, fallback to AniWatch, then AniList (synthetic)
    pub async fn get_episodes(&self, id: &str, start: u32,
    ) -> Option<Vec<crate::models::anime::EpisodeSummary>> {
        // Try HiAnime first
        if let Ok(ep_data) = self.hianime.get_episodes(id, start).await {
            let episodes: Vec<crate::models::anime::EpisodeSummary> = ep_data.episodes
                .unwrap_or_default()
                .into_iter()
                .map(|ep| crate::models::anime::EpisodeSummary {
                    id: ep._id,
                    number: ep.episodeNumber.unwrap_or(0),
                    title: None,
                    sub_count: ep.link.as_ref().and_then(|l| l.sub.as_ref()).map(|v| v.len() as u32).unwrap_or(0),
                    dub_count: ep.link.as_ref().and_then(|l| l.dub.as_ref()).map(|v| v.len() as u32).unwrap_or(0),
                    slug: ep.slugs.as_ref().and_then(|s| s.first().cloned()),
                    slugs: ep.slugs,
                })
                .collect();
            if !episodes.is_empty() {
                return Some(episodes);
            }
        }

        // Try AniWatch detail which includes episodes
        if let Ok(detail) = self.aniwatch.get_detail(id).await {
            if let Some(eps) = detail.episodes {
                if !eps.is_empty() {
                    return Some(eps);
                }
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

    /// Get servers: try AniWatch, then HiAnime, then Animexin
    pub async fn get_servers(&self, id: &str
    ) -> Option<ServerList> {
        // AniWatch uses episode ID directly
        if let Ok(servers) = self.aniwatch.get_servers(id).await {
            if !servers.sub.is_empty() || !servers.dub.is_empty() {
                return Some(servers);
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

    /// Get stream: try HiAnime, then AniWatch, then Animexin
    pub async fn get_stream(
        &self,
        id: &str,
        server: Option<&str>,
        link_type: &str,
        episode: u32,
    ) -> Option<StreamPayload> {
        // HiAnime first (most reliable streams)
        if let Ok(data) = self.hianime.get_stream(id, server, link_type, episode).await {
            return Some(stream_data_to_payload(data));
        }

        // AniWatch
        let server_name = server.unwrap_or("hd-1");
        let category = if link_type == "dub" { "dub" } else { "sub" };
        if let Ok(stream) = self.aniwatch.get_stream(id, server_name, category).await {
            return Some(stream);
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

        // HiAnime home
        if let Ok(raw) = self.hianime.get_home().await {
            let hi_feed = crate::clients::hianime::transform_home_feed(raw);
            feed.trending = hi_feed.trending;
            feed.currently_airing = hi_feed.currently_airing;
            feed.featured = hi_feed.featured;
            feed.latest = hi_feed.latest;
            feed.popular = hi_feed.popular;
            feed.finished_airing = hi_feed.finished_airing;
        }

        // AniWatch home (merge into same fields)
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

        feed
    }
}

fn normalize_title(title: &str) -> String {
    title.to_lowercase().trim().to_string()
}

fn merge_optional_vecs(a: Option<Vec<AnimeCard>>, b: Option<Vec<AnimeCard>>) -> Option<Vec<AnimeCard>> {
    match (a, b) {
        (Some(mut av), Some(bv)) => {
            let mut seen = std::collections::HashSet::new();
            for card in &av {
                seen.insert(normalize_title(&card.title));
            }
            for card in bv {
                if seen.insert(normalize_title(&card.title)) {
                    av.push(card);
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
            tracks.into_iter().map(|t| SubtitleTrack {
                file: t.file,
                label: t.label,
                kind: t.kind,
                default: t.default,
            }).collect()
        }),
        intro: data.intro.map(|i| SkipRange { start: i.start, end: i.end }),
        outro: data.outro.map(|o| SkipRange { start: o.start, end: o.end }),
        server: data.server,
        referer: data.referer,
    }
}
