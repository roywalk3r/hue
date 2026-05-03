# Hue Stream API — Architecture Cleanup Design

## Current State

Hue has two streaming engines sharing zero code:
- **Node.js Express API** — Client-facing, serves all routes, runs Playwright providers
- **Rust Axum API (huerust)** — Runs on port 24609, handles HTTP-based providers + discovery

They connect via a circular bridge:
- Node → Rust via `providers/huerust.js` (HTTP call to `/api/streams/...`)
- Rust → Node via `huerust/src/routes/play.rs` (HTTP call to `/api/anime/stream`)

## Target Architecture

Client → Rust API (port 24609)
  ├── HTTP providers (SFlix, Vixsrc, KissKH, Dramacool, MovieBox)
  ├── M3U8/TS/sub proxy + playlist rewriting
  ├── Discovery (HiTV, MovieBox, Jikan)
  ├── Cache (Redis-backed)
  ├── Circuit breaker
  ├── Rate limiting
  ├── Auth (API key)
  ├── OpenAPI docs
  └── Calls Node scraper internally when needed:
       POST /internal/scrape/browser
       GET  /internal/cookies/:provider
       GET  /internal/health

Node Scraper (port 24610, internal only)
  ├── Playwright-based providers (Showbox, MP4Hydra, VidZee, vidsrccc-pw, moviesmod, uhdmovies, 4khdhub)
  ├── Cookie rotation (FebBox, SFlix, MovieBox)
  ├── HiAnime scraping (megacloud/megaplay extraction)
  └── Provider fallback scraping (when HTTP APIs change)

## Internal API Contract (Rust → Node)

### GET /internal/health
Response: `{ ok: true, providers: ["showbox", "mp4hydra", ...], uptime: 12345 }`

### GET /internal/cookies/:provider
Providers: `febbox`, `sflix`, `moviebox`
Response: `{ cookies: ["ui=abc123...", ...], expires: 1712345678 }`

### POST /internal/scrape/browser
Request: `{ provider: "showbox", tmdbId: "12345", type: "movie", season: 1, episode: 1 }`
Response: `{ success: true, streams: [{ url, title, quality, provider, format, subtitles }] }`

### POST /internal/scrape/anime
Request: `{ slug: "one-piece", episode: 1070 }`
Response: `{ success: true, stream: { url, server, tracks: [...] } }`

## What Gets Deleted

- `providers/sflix.js`, `vixsrc.js`, `kisskh.js`, `dramacool.js`, `moviebox.js` — Huerust covers these
- `providers/huerust.js` — circular bridge, replaced by Rust→Node calls
- `routes/` — all route logic moves to Rust
- `proxy/proxyServer.js` — Rust proxy replaces this (73KB)
- `providers/registry.js` — Rust registry replaces this
- `services/rustCircuitSync.js` — dead
- `*.disabled` files
- `debug-*.js`, `capture-*.js`, `brute-force-api.js`, `call_subject_play.sh`
- `vendor/` — branded hianime copy
- `services/test-megaplay.js` — test in wrong place

## What Node Keeps (Internal Scraper)

node-scraper/
  src/
    index.js              — Express app, internal routes only
    providers/
      showbox.js
      mp4hydra.js
      vidzee.js
      vidsrccc-playwright.js
      moviesmod.js
      uhdmovies.js
      4khdhub.js
      hianime.js           — anime scraping (megacloud/megaplay)
    services/
      browserManager.js
      cookieWorker.js      — rotates cookies for febbox/sflix/moviebox
      stealth.js
      browserConfig.js
    utils/
      interceptor.js       — ad-blocking browser interceptors
      jsunpack.js

## Data Flow: Stream Request

1. Client → `GET /api/streams/movie/12345` → Rust API
2. Rust checks Redis cache → hit → return proxied streams
3. Cache miss → Rust fires HTTP providers in parallel (SFlix, Vixsrc, KissKH, etc.)
4. Simultaneously, Rust calls Node `POST /internal/scrape/browser` for Playwright-only providers
5. Rust deduplicates by URL, applies filters, caches in Redis, wraps in proxy URLs
6. Rust returns aggregated results

## Data Flow: Cookie Refresh

1. Node `cookieWorker.js` runs on a 30-min interval
2. For each provider needing cookies: launches headless browser, navigates to login page, extracts cookies
3. Stores in Redis as `cookies:{provider}` with TTL
4. Rust reads from Redis via `GET /internal/cookies/:provider` → Node reads from its own Redis

## Key Design Decisions

- **No shared code between Rust and Node** — they communicate via versioned HTTP contract
- **Redis as the shared state** — both services read/write the same Redis for cache, cookies, rate limits
- **Node never calls Rust** — dependency is one-directional
- **Rust API is the single source of truth for stream URLs** — Node returns raw CDN URLs, Rust wraps them in proxy tokens
- **Either service can restart independently** — if Node is down, Rust still serves cached results + HTTP-only providers
- **Provider timeout hierarchy**: Fast HTTP (5s) → Slow HTTP (25s) → Browser fallback (60s)
