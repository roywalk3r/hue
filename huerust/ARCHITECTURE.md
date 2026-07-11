# Architecture

## Module Map

```
src/
  main.rs          — Tokio runtime, config load, server start
  app.rs           — AppState, DramaState, HiTvState, route group states
  config.rs        — Env-var based configuration
  models.rs        — StreamResult, Subtitle, anime models
  openapi.rs       — utoipa OpenAPI spec
  security.rs      — AES-CBC URL encryption/decryption
  filters.rs        — Stream filtering rules
  clients/         — External API clients
    tmdb.rs        — TMDB metadata + search (key rotation, moka cache)
    hitv.rs        — Drama aggregation client
    moviebox.rs    — MovieBox API client
    hianime.rs     — Anime scraper/stream extractor
    vidrock.rs     — VidRock AES-encrypted API client
  providers/       — Stream provider implementations
    mod.rs         — StreamProvider trait
    registry.rs    — ProviderRegistry with circuit breaker integration
    dispatch.rs    — Parallel provider fetch orchestration
    kisskh.rs      — KissKH client (kkey generation, subtitle fetching)
    sflix.rs         — SFlix client
    dramacool.rs   — Dramacool client
    vixsrc.rs      — Vixsrc.to client
    mp4hydra.rs    — MP4Hydra client
    vidzee.rs      — VidZee AES decryption client
    videasy.rs     — Videasy client (delegates to Node.js daemon)
    ppvto.rs        — PPV.to sports client
    *_provider.rs   — StreamProvider trait wrappers
  routes/          — Axum route handlers
    streams.rs     — /api/streams/:type/:tmdbId (HTTP + SSE)
    play.rs        — /api/play/:type/:tmdbId (Smart Play)
    drama.rs        — /api/drama/* (KissKH, Dramacool, MovieBox)
    hitv.rs        — /api/hitv/* (browse, search, play)
    moviebox.rs    — /api/moviebox/* (home, trending, categories)
    anime.rs        — /api/anime/* (search, detail, episodes, stream)
    system.rs      — /health, /metrics, /api/status
    proxy.rs        — Legacy proxy route (unused)
    vidrock.rs      — /api/vidrock/* (direct VidRock streams)
  proxy/           — HLS/TS/Subtitle proxy engine
    mod.rs          — m3u8-proxy, ts-proxy, sub-proxy handlers
    wrap.rs         — StreamResult URL wrapping into proxied tokens
    adsurgery.rs    — HLS manifest ad segment removal
    sanitize.rs    — TS packet sanitization
  infra/           — Infrastructure modules
    cache.rs        — StreamCache (moka/redis)
    circuit_breaker.rs — Per-provider circuit breaker
    coalesce.rs     — FetchCoalescer (deduplicates concurrent requests)
    metrics.rs      — Prometheus-style metrics
    rate_limit.rs   — IP-based sliding window rate limiter
```

## Request Flow: Stream Aggregation

```
Client ──► /api/streams/movie/27205
    │
    ▼
┌─────────────────────────────────────────────────────────────┐
│  1. Coalescer check — is an identical request already      │
│     in flight? If yes, wait for its result.                │
│                                                              │
│  2. Cache check — do we have fresh cached results?          │
│     (TTL = 4 hours by default)                               │
│                                                              │
│  3. ProviderRegistry dispatch — fire all registered          │
│     providers in parallel. Each provider has its own         │
│     timeout (15–60s) and circuit breaker state.            │
│                                                              │
│  4. Filtering — deduplicate URLs, remove blacklisted         │
│     domains, apply provider stability scoring.             │
│                                                              │
│  5. Proxy wrapping — encrypt upstream URLs into h_token      │
│     query params, inject CDN headers, rewrite subtitles.    │
│                                                              │
│  6. Cache write + JSON response (or SSE events)             │
└─────────────────────────────────────────────────────────────┘
```

## Proxy Pipeline

```
Upstream URL ──► encrypt_url() ──► h_token query param
    │
    ▼
/m3u8-proxy?h_token=...
    │
    ▼
┌─────────────────────────────────────────────────────────────┐
│  1. Decrypt h_token → original URL + headers                │
│                                                              │
│  2. CDN header injection based on host:                     │
│     • vixsrc.to → Referer: https://vixsrc.to/              │
│     • hakunaymatata.com / aisports.mobi / poocloud.in      │
│       → Referer: https://moviebox.ph/                     │
│     • mewstream.buzz / animeplay.cfd                        │
│       → Referer: https://hianimes.se/                     │
│                                                              │
│  3. Fetch upstream playlist / segment / subtitle            │
│                                                              │
│  4. Playlist rewriting: resolve relative URLs, rewrite    │
│     segment URLs back through proxy (token or url mode)    │
│                                                              │
│  5. Optional ad surgery: strip ad segments from manifest    │
│                                                              │
│  6. Optional TS sanitization: remove garbage packets       │
│                                                              │
│  7. Segment prefetch: spawn background task to fetch         │
│     first 5 segments after playlist rewrite                 │
└─────────────────────────────────────────────────────────────┘
```

## Authentication

- **API routes** (`/api/*`, `/health`, `/metrics`): `x-api-key` header check against `API_KEY` env var. Loopback IPs are exempt.
- **Proxy routes** (`/m3u8-proxy`, `/ts-proxy`, `/sub-proxy`): Self-authenticated via `h_token` encrypted URLs. No API key required.
- **Rate limiting**: 120 requests / 60s per IP. Proxy routes are exempt.

## Background Tasks

1. **HiTV Library Builder** — rebuilds every 12 hours
2. **Rate Limiter Eviction** — cleans stale entries every 5 minutes

## Dual Backend Strategy

The Rust backend (port `24609`) owns:
- Stream aggregation + proxying
- Drama browse/search/detail/stream
- Anime browse/search/detail/stream
- MovieBox browse
- HiTV browse

The Node.js backend (port `24608`) still handles:
- Auth (login, password)
- Live TV + sports
- TMDB proxy (some routes)
- Playwright scraper fallback
- Static UI pages
