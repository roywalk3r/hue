# Anime pipeline

Anime is its own vertical. The reasons it is split from the
main aggregator:

- Most stream providers don't cover anime well.
- Anime metadata (episodes, seasons, OVA ordering) is
  modeled differently in TMDB vs AniList.
- Japanese / Chinese / Korean sources dominate this catalog,
  and they have their own scraping quirks.

## Anime-specific routes

```
/api/anime/home                — homepage rails
/api/anime/search?q=           — search by title
/api/anime/discover?            — discover with filters
/api/anime/discover (POST)      — discover with body
/api/anime/filters              — available filter values
/api/anime/schedule             — weekly schedule
/api/anime/detail/:slug         — show detail
/api/anime/episodes/:id         — episode list
/api/anime/servers?id=&ep=&type=— per-episode servers
/api/anime/stream?id=&ep=&type=&server=
                               — extract stream from a server
```

Deprecated aliases (still work, log warn):

```
/api/anime/category   → /api/anime/discover
/api/anime/filter     → /api/anime/discover
/api/anime/latest     → /api/anime/discover?sort=latest
```

## AnimeAggregator

`src/clients/anime_agg.rs::AnimeAggregator` is the single entry
point for anime list / discover / filter / schedule metadata.
It fans out to three backends in parallel and dedupes by
`mal_id`:

- **AniList** — the filter spine. AniList has the best
  filter model (genres, year, season, status, type) and
  the best catalog coverage.
- **AniWatch** — backfill for titles that AniList doesn't
  have full episode lists for.
- **HiAnime** — backfill for the same; HiAnime is also a
  stream source (see "AniWave" below).

The aggregator returns `AnimeCard` / `AnimeDetail` /
`EpisodeSummary` / `AnimeRail` shapes that are normalized
across backends.

## AniWave as a stream source

`AniWave` is the primary anime provider for JP/CN/KR
content. Streams come back as HLS playlists with the same
`EXT-X-KEY` AES-128 pattern as Vixsrc. The proxy handles
AniWave's CDN Referer requirements via the implicit
header injection in [docs/proxy.md](proxy.md).

Animexin is **excluded** from the `discover` API because
it returns iframe-only streams that the mobile player
can't play (the `playwright-agent` fallback is for the
web player only).

## EpisodeSummary

`EpisodeSummary` carries both a per-episode `slug` and the
parent `seriesSlug`. The flow:

1. Detail page calls `/api/anime/detail/:slug` → returns
   `AnimeDetail` with `EpisodeSummary[]`.
2. Player picks an episode, calls
   `/api/anime/servers?id=<seriesSlug>&episode=<ep>&type=...`
   → returns the list of available servers for that
   episode.
3. Player picks a server, calls
   `/api/anime/stream?id=...&ep=...&type=...&server=...`
   → returns an `HLS` URL (AniWave) or an iframe URL
   (Animexin — web only).

The `seriesSlug` is the routing key for steps 2 and 3;
the per-episode `slug` is sometimes used by HiAnime
for direct episode lookup.

## Stream encryption (kkey)

Like the movie/TV streams, anime streams are encrypted into
`h_token` for the proxy to replay. The Player decrypts
the `kkey` separately (client-side, not the Rust proxy's
job — see the global memory note on Kkey).

## Anime mode toggle (UI)

The Flutter app exposes a toggle on the anime home page
that switches between "subbed" and "donghua" modes.
Donghua (Chinese animation) is a separate rails feed
sourced from AnimeKill's homepage. The
`/api/anime/donghua/rails` and `/api/anime/donghua/hero`
endpoints are not currently exposed in the public API
surface; they're internal to the Flutter `AnimeModeNotifier`.

AnimeKill is parsed via `/api/data/parser/{anime_id}/{episode}`
for live mirrors — the homepage's episode links point to
dead `gotaku1.com` embeds, so the parser is the only
working path. See the global memory note on
AnimeKill donghua parsing.

## Cache & rate limit

The anime aggregator uses the same moka cache as the
main stream cache (`CACHE_TTL_SECS`, default 4 h). Rate
limits are the same IP-based 120/60s.

## Operations

- AniList is rate-limited upstream. The aggregator
  uses key rotation across multiple AniList tokens
  (configured in `.env` as `ANILIST_KEYS=...`).
- AnimeKill's parser is fragile — if AnimeKill changes
  its embed structure, the parser silently returns
  empty results and the donghua home page falls back
  to a generic "no results" state.
- AniWave's CDN rotates Referer requirements. The
  proxy's implicit header injection handles this
  automatically; if AniWave changes CDN host, update
  the injection table in `src/proxy/mod.rs`.
