# Drama pipeline

Drama (Asian drama, K-drama, J-drama, C-drama) is treated as a
distinct vertical from movies and TV. The reasons:

- The main aggregator is movie/TV-first; its providers return
  poor matches for drama titles because TMDB metadata is
  sparse for non-Western content.
- The sources that *do* have good drama catalogs (KissKH,
  KissAsian, MovieBox) are not TMDB-keyed — they're slug-keyed or
  detail-path-keyed.

## Drama-specific routes

```
/api/drama/moviebox/browse                  — MovieBox drama rails
/api/drama/moviebox/search?q=               — MovieBox drama search
/api/drama/moviebox/detail-rec?q=           — MovieBox drama detail
/api/drama/moviebox/streams/:title/:episode — MovieBox drama streams
/api/drama/moviebox/streams-direct?         — MovieBox drama direct
   detail_path=...&episode=...
/api/drama/kisskh/search?q=                 — KissKH (Zenith) search
/api/drama/kisskh/show?id=                  — KissKH show detail
/api/drama/kisskh/browse                    — KissKH browse rails
/api/drama/kisskh/upcoming                  — KissKH upcoming
/api/drama/kisskh/top-rating                — KissKH top-rated
/api/drama/kisskh/streams/:id/:episode      — KissKH streams + subs
/api/drama/kissasian/search?q=              — KissAsian (Pearl) search
/api/drama/kissasian/streams/:id/:episode   — KissAsian streams
/api/drama/kissasian_top/search?q=          — KissAsian.top (Rose)
/api/drama/kissasian_top/browse             — KissAsian.top browse
/api/drama/kissasian_top/info/:slug         — KissAsian.top detail
/api/drama/kissasian_top/streams/:id/:ep    — KissAsian.top streams
```

The Xyra-backed drama routes (`/api/drama/dramacool/*`,
`/api/drama/kisskh_xyra/*`) are **disabled** in the route table.
xyra upstream was returning wrong-match / duplicate results for
these titles.

## MovieBox drama path (the canonical one)

MovieBox is the primary drama source for both the Nuxt web app
and the Flutter mobile app. The Player always routes
drama playback through MovieBox even when other providers are
also available.

Why MovieBox-first:

1. MovieBox has the largest drama catalog.
2. The CDN MP4 streams play directly without any decryption
   layer (no AES, no HLS — just `.mp4`).
3. The CDN IP-blocks datacenters, so the streams are returned
   **raw** to the client (not proxied) — the user agent
   fetches them from the device, not the server.

Flow:

```
Client ──► /api/drama/moviebox/streams/:title/:episode
              │
              ▼
   detail lookup (search → first match → detail_path)
              │
              ▼
   call MovieBox /api/stream/streams?detailPath=...&episode=...
              │
              ▼
   normalize to StreamResult[]
              │
              ▼
   wrap_with_proxy (mostly a no-op for MovieBox MP4 — host whitelisted)
              │
              ▼
   return StreamsResponse
```

The detail-path-keyed `streams-direct` endpoint is a
shortcut when the client already has the detail_path from a
prior search/browse call. Skips the search round-trip.

## KissKH (Zenith)

Zenith is a TMDB-keyed drama provider, but it requires
`kkey`-style decryption for its CDN URLs. The Rust provider
gates Zenith on `ENABLE_KISSKH=true` (default: off) because the
upstream decryption was rotating frequently and breaking
playback.

KissKH is the second-fallback drama source after MovieBox.
The Player's auto-picker prefers it for Asian content but
falls back to MovieBox on any 403/timeout.

## KissAsian (Pearl) and KissAsian.top (Rose)

Pearl is the older KissAsian (Oriental) site. Slug-keyed,
works fine but the catalog overlaps with MovieBox. Movie/TV
requests are short-circuited at the provider level.

Rose is KissAsian.top, a different site. Direct MP4 streams
(no scraping step), so it's a fast provider with a 6 s
timeout. The dispatcher can't yet route to it via TMDB
because the catalog mapping is incomplete; the Flutter app
calls it directly via `/api/drama/kissasian_top/*`.

## Subtitle handling

All three drama sources return VTT/SRT subtitle URLs alongside
the video stream. The Player auto-attaches them; the API
exposes them as a `subtitles: [...]` field on each
`StreamResult`. Subtitles are proxied through
`/sub-proxy?h_token=...` so the upstream `Referer` requirement
is satisfied.

See [docs/subtitles.md](docs/subtitles.md) for the full
subtitle pipeline including the Vidrock and Xyra subtitle
fallbacks.

## Cache & rate limit

Drama routes are subject to the same IP rate limit as the rest
of the API (120 req/60s). They are **not** cached at the
`StreamCache` layer — the URLs are too short-lived.

## What does NOT change

- The MovieBox client (`src/clients/moviebox.rs`) and its
  provider (`src/providers/moviebox_provider.rs`) are the
  same code paths used by the main aggregator. Drama routing
  is a route-level specialization, not a separate
  provider pipeline.
- The Player's stream picker for movies/TV is unchanged.
  Drama is opt-in from the search / detail page.

## Operations

- MovieBox upstream rotates detail paths every few days.
  The drama pipeline re-resolves on every request rather
  than caching, so a stale path never reaches the user.
- KissKH is a fragile source. The `ENABLE_KISSKH` flag
  exists to let us kill it instantly in production without
  a deploy.
- KissAsian and KissAsian.top have similar flavors and
  similar failure modes. The Pearl provider short-circuits
  movie requests because its match rate is too low.
