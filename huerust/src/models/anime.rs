/*
 * HUE ENGINE ❄
 * System Designation: The Architecture of Aggregation
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// A lightweight anime card for lists, grids, carousels.
#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AnimeCard {
    /// MongoDB hex ID
    pub id: String,
    /// Primary display title (English)
    pub title: String,
    /// Original Japanese title
    #[serde(skip_serializing_if = "Option::is_none")]
    pub japanese_title: Option<String>,
    /// Short synopsis / description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub synopsis: Option<String>,
    /// Poster image URL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    /// Wide banner / landscape image URL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub banner_image: Option<String>,
    /// Anime type: TV, Movie, OVA, ONA, Special
    #[serde(skip_serializing_if = "Option::is_none")]
    pub anime_type: Option<String>,
    /// Airing status: Currently Airing, Finished Airing, Not yet aired
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    /// MAL score (e.g. "8.73")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score: Option<String>,
    /// Content rating (e.g. "PG-13", "R - 17+")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rating: Option<String>,
    /// Genre tags
    #[serde(skip_serializing_if = "Option::is_none")]
    pub genres: Option<Vec<String>>,
    /// Total number of episodes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_episodes: Option<u32>,
    /// Number of subbed episodes available
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_subbed: Option<u32>,
    /// Number of dubbed episodes available
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_dubbed: Option<u32>,
    /// URL-safe slug for routing
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slug: Option<String>,
    /// MAL ID for external links
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mal_id: Option<u32>,
    /// View count for popularity ranking
    #[serde(skip_serializing_if = "Option::is_none")]
    pub view_count: Option<u32>,
    /// Aired date range
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aired: Option<String>,
    /// Episode duration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<String>,
    /// Country of origin: "JP" | "CN" | "KR" | "US"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
    /// Source/provider slug: "aniwave" | "animexin" | "hianime" | "aniwatch" | "anilist"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// Whether this anime has at least one episode with sub servers
    pub has_sub: bool,
    /// Whether this anime has at least one episode with dub servers
    pub has_dub: bool,
}

/// Full anime detail for a dedicated anime page.
#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AnimeDetail {
    #[serde(flatten)]
    pub card: AnimeCard,
    /// Broadcast schedule
    #[serde(skip_serializing_if = "Option::is_none")]
    pub broadcast: Option<String>,
    /// Season premiered
    #[serde(skip_serializing_if = "Option::is_none")]
    pub premiered: Option<String>,
    /// Production studios / producers
    #[serde(skip_serializing_if = "Option::is_none")]
    pub producers: Option<String>,
    /// Licensing companies
    #[serde(skip_serializing_if = "Option::is_none")]
    pub licensors: Option<String>,
    /// Source material (Manga, Light Novel, Original, etc)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// Members count on MAL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub members: Option<String>,
    /// Favorites count on MAL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub favorites: Option<String>,
    /// MAL popularity rank
    #[serde(skip_serializing_if = "Option::is_none")]
    pub popularity: Option<String>,
    /// MAL ranked position
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ranked: Option<String>,
    /// Episode list
    #[serde(skip_serializing_if = "Option::is_none")]
    pub episodes: Option<Vec<EpisodeSummary>>,
}

/// Lightweight episode summary for detail pages.
#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct EpisodeSummary {
    /// Episode hex ID
    pub id: String,
    /// Episode number
    pub number: u32,
    /// Episode title
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Number of available sub servers
    pub sub_count: u32,
    /// Number of available dub servers
    pub dub_count: u32,
    /// Per-episode slug used to resolve the stream (e.g. "81553-ep1").
    /// Pass this as the `id` query param to `/api/anime/stream`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slug: Option<String>,
    /// Series slug (e.g. "one-piece-81553" or "81553"). Use this to fetch
    /// the full episode list or open the detail page; do NOT use it to
    /// resolve an individual stream.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub series_slug: Option<String>,
    /// All available slug variants for fallback
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slugs: Option<Vec<String>>,
}

/// Server entry for a specific episode.
#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ServerEntry {
    /// Server ID (numeric for JSON servers, URL for embeds)
    pub id: String,
    /// Display name: HD-1, HD-2, VIDMOLY, etc
    pub name: String,
    /// Audio track: sub | dub
    pub audio_type: String,
    /// Server format: json (extractable) | embed (iframe)
    pub format: String,
}

/// Clean server list response.
#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ServerList {
    pub sub: Vec<ServerEntry>,
    pub dub: Vec<ServerEntry>,
}

/// Stream payload for video players.
#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StreamPayload {
    /// Direct or proxied stream URL
    pub url: String,
    /// Stream type: hls | dash | mp4 | iframe
    pub stream_type: String,
    /// Optional subtitle tracks
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subtitles: Option<Vec<SubtitleTrack>>,
    /// Skip intro start/end seconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intro: Option<SkipRange>,
    /// Skip outro start/end seconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outro: Option<SkipRange>,
    /// Server name that served this stream
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server: Option<String>,
    /// Required Referer header for the stream
    #[serde(skip_serializing_if = "Option::is_none")]
    pub referer: Option<String>,
}

/// Subtitle / caption track.
#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SubtitleTrack {
    pub file: String,
    pub label: String,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<bool>,
}

/// Intro / outro skip range in seconds.
#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SkipRange {
    pub start: u32,
    pub end: u32,
}

/// Structured home feed for dashboard / landing page.
#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AnimeHomeFeed {
    /// Trending / most viewed anime
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trending: Option<Vec<AnimeCard>>,
    /// Currently airing this season
    #[serde(skip_serializing_if = "Option::is_none")]
    pub currently_airing: Option<Vec<AnimeCard>>,
    /// Featured / spotlight picks
    #[serde(skip_serializing_if = "Option::is_none")]
    pub featured: Option<Vec<AnimeCard>>,
    /// Recently added anime
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest: Option<Vec<AnimeCard>>,
    /// Popular / most watched anime
    #[serde(skip_serializing_if = "Option::is_none")]
    pub popular: Option<Vec<AnimeCard>>,
    /// Finished airing / completed series
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finished_airing: Option<Vec<AnimeCard>>,
}

/// Paginated search response.
#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AnimeSearchResponse {
    pub query: String,
    pub total: usize,
    pub page: u32,
    pub limit: u32,
    pub results: Vec<AnimeCard>,
}

// ── Unified paginated envelope ──────────────────────────────────────────────
//
// All NEW list endpoints (discover, schedule-as-list, future browse feeds)
// return this shape. `AnimeSearchResponse` above keeps its existing shape
// to avoid breaking the search route; documented in OpenAPI.

/// Standard pagination wrapper. Use this for any new list endpoint.
///
/// `ToSchema` is implemented manually (via [`PaginatedResponse::schema_name`])
/// rather than via derive because generic `T: ToSchema` isn't supported by
/// utoipa's derive macro. The runtime type at the route level is concrete
/// (e.g. `PaginatedResponse<AnimeCard>`), and the OpenAPI spec references
/// the concrete schema.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PaginatedResponse<T> {
    pub items: Vec<T>,
    pub total: u32,
    pub page: u32,
    pub limit: u32,
    pub has_more: bool,
}

// ── Filter discovery ────────────────────────────────────────────────────────

/// A single (value, label) pair in a `FilterOptions` dropdown. The frontend
/// renders these in the order they appear.
#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FilterChoice {
    pub value: String,
    pub label: String,
}

/// Static metadata for the discover screen. Years are generated at call
/// time so they always include the current year. The genre list is the
/// canonical AniList list; the others are also stable across providers.
#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FilterOptions {
    pub types: Vec<FilterChoice>,
    pub statuses: Vec<FilterChoice>,
    pub sorts: Vec<FilterChoice>,
    pub genres: Vec<FilterChoice>,
    pub years: Vec<FilterChoice>,
    pub seasons: Vec<FilterChoice>,
    pub languages: Vec<FilterChoice>,
    /// Country of origin filter: "JP" | "CN" | "KR" | "US"
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub countries: Vec<FilterChoice>,
    pub scores: Vec<FilterChoice>,
    pub ratings: Vec<FilterChoice>,
}

/// Query/body params accepted by `/api/anime/discover`. Every field is
/// optional. Unknown fields cause a 400 (replaces the previous permissive
/// `serde_json::Value` passthrough).
#[derive(Debug, Default, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct FilterParams {
    /// TV, movie, ova, ona, special, music
    #[serde(default)]
    pub r#type: Option<String>,
    /// Single genre slug (e.g. "Action", "Sci-Fi"). Multi-genre is a
    /// future extension.
    #[serde(default)]
    pub genre: Option<String>,
    /// airing | finished | upcoming
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub year: Option<u32>,
    /// winter | spring | summer | fall
    #[serde(default)]
    pub season: Option<String>,
    /// popularity | score | trending | latest | alphabetical
    #[serde(default)]
    pub sort: Option<String>,
    /// japanese | english | chinese | korean
    #[serde(default)]
    pub language: Option<String>,
    /// CN | JP | KR | US
    #[serde(default)]
    pub country: Option<String>,
    /// "1".."10" or "all"
    #[serde(default)]
    pub score: Option<String>,
    /// G | PG | PG-13 | R | R+ | Rx
    #[serde(default)]
    pub rating: Option<String>,
    #[serde(default)]
    pub page: Option<u32>,
    #[serde(default)]
    pub limit: Option<u32>,
}

// ── Schedule ────────────────────────────────────────────────────────────────

/// A single airing in the daily schedule.
#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleEntry {
    pub anime_id: String,
    pub anime_title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slug: Option<String>,
    pub episode: u32,
    /// sub | dub if known
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio_type: Option<String>,
    /// Unix seconds, UTC
    pub airs_at: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub countdown_seconds: Option<u64>,
}

// ── Episode list wrapper ────────────────────────────────────────────────────

/// Response shape for `/api/anime/episodes/{id}`. Replaces the inline
/// `json!({ animeId, start, count, episodes })` previously assembled in
/// the route handler.
#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct EpisodeListResponse {
    pub anime_id: String,
    pub start: u32,
    pub total: u32,
    pub episodes: Vec<EpisodeSummary>,
}
