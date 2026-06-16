# CLAUDE.md

Project guidance for Claude Code when working in `huerust`.

## Overview

`huerust` is the Hue streaming engine — a Rust/axum backend that aggregates video streams from many third-party providers, proxies HLS/TS/subtitle traffic through encrypted tokens, and exposes browse/search/detail/playback APIs for movies, TV, anime, and drama.

## Commands

```bash
# Dev / run
cd huerust
cargo run

# Build release
cargo build --release

# Tests / lint
cargo test
cargo clippy
```

Server binds to `127.0.0.1:24609` by default.

## Stack

- **Framework**: axum 0.7, Tokio, tower-http
- **HTTP client**: reqwest + reqwest-middleware + reqwest-retry
- **Serialization**: serde, serde_json, serde_qs
- **Crypto**: aes, aes-gcm, cbc, scrypt, sha2
- **Caching**: moka (memory) or deadpool-redis
- **Rate limiting**: governor
- **Metrics**: metrics + metrics-exporter-prometheus
- **Docs**: utoipa + Swagger UI at `/api/docs`

## Key directories

```
src/
  main.rs              — Tokio runtime, config load, server start
  app.rs               — AppState, build_state, route table
  config.rs            — Env-var config (AppConfig)
  models.rs            — StreamResult, Subtitle
  models/anime.rs      — AnimeCard, AnimeDetail, EpisodeSummary, etc.
  openapi.rs           — utoipa spec
  security.rs          — AES-CBC URL encryption/decryption
  filters.rs           — Stream filtering rules
  http_client.rs       — HTTP client builders with retry
  clients/             — External API clients (TMDB, MovieBox, HiTV, anime)
  clients/anime_agg.rs — AnimeAggregator fan-out/dedup
  providers/           — Stream provider implementations
    mod.rs             — StreamProvider trait
    registry.rs        — ProviderRegistry
    dispatch.rs        — Parallel fetch orchestration
    *_provider.rs      — Trait wrappers
    *.rs               — Provider clients
  routes/              — Axum route handlers
  proxy/               — HLS/TS/subtitle proxy engine
  infra/               — Cache, circuit breaker, coalesce, metrics, rate limit
```

## Runtime configuration

All config is env-based. `.env` is symlinked to `/mnt/devdrive/projects/Misc/hue/.env`.

Key variables:

```
RUST_HOST=0.0.0.0
RUST_PORT=24609
RUST_PROXY_URL=https://api.vexli.online
APP_SECRET=...
TMDB_API_KEYS=...
KKEY=...
CACHE_TTL_SECS=14400
FAST_PROVIDER_DEADLINE_SECS=5
SLOW_PROVIDER_DEADLINE_SECS=18
ENABLE_PROXY=true
CACHE_BACKEND=memory
REDIS_URL=...
```

See `src/config.rs` for full defaults.

## API surface

### Stream aggregation (TMDB only)

```
GET  /api/streams/:type/:tmdbId?s=1&e=1
GET  /api/streams/:type/:tmdbId/live  (SSE)
GET  /api/play/:type/:tmdbId            (Smart Play)
```

- `type` must be `movie` or `tv`.
- `tmdbId` is a numeric TMDB ID.
- `detail_path` (`dp`) is supported for MovieBox lookups only inside providers.

### Drama

```
GET /api/drama/detail?q=...
GET /api/drama/kisskh/...
GET /api/drama/dramacool/...
GET /api/drama/moviebox/browse
GET /api/drama/moviebox/search?q=...
GET /api/drama/moviebox/streams/:title/:episode
GET /api/drama/moviebox/streams-direct?detail_path=...&episode=...
```

### Anime

```
GET  /api/anime/home
GET  /api/anime/search?q=
GET  /api/anime/discover?...
POST /api/anime/discover
GET  /api/anime/filters
GET  /api/anime/schedule
GET  /api/anime/detail/:slug
GET  /api/anime/episodes/:id
GET  /api/anime/servers?id=&episode=&type=
GET  /api/anime/stream?id=&episode=&type=&server=
```

Deprecated aliases (still work, log `warn!`):

```
GET /api/anime/category  -> /api/anime/discover
POST /api/anime/filter   -> /api/anime/discover
GET /api/anime/latest    -> /api/anime/discover?sort=latest
```

### Proxy routes

```
GET /m3u8-proxy?h_token=...
GET /ts-proxy?h_token=...
GET /sub-proxy?h_token=...
```

Authenticated via encrypted `h_token`. No API key required.

## Architecture rules

1. **Provider pattern**: Every stream source implements `StreamProvider` in `providers/`. Wrappers adapt low-level clients to the trait.
2. **Registry + dispatch**: `ProviderRegistry::fetch_all` fires providers in parallel with per-provider timeouts and circuit breakers.
3. **Coalescer**: Identical concurrent requests share the same in-flight future.
4. **Cache**: Stream results cached in moka/redis with TTL.
5. **Proxy wrapping**: Upstream URLs are encrypted into `h_token` query params so the proxy can replay headers.
6. **CDN header injection**: Proxy injects Referer/User-Agent based on upstream host (see `proxy/mod.rs`).

## Anime-specific rules

- `AnimeAggregator` is the single entry point for anime list/discover/filter metadata/schedule.
- AniList is the filter spine; AniWatch and HiAnime backfill.
- AniWave is the primary anime provider for JP/CN/KR content; Animexin is excluded from `discover` because it returns iframe-only streams the mobile player can't play.
- `EpisodeSummary` carries both `slug` (per-episode) and `seriesSlug` for routing.

## Drama-specific rules

- MovieBox is the canonical drama provider used by the web app (`hue-ui`).
- `/api/drama/moviebox/streams-direct` is the endpoint the Vue/Flutter players use for MovieBox drama playback; it accepts the provider-native `detail_path`.
- **Do not route MovieBox drama IDs through `/api/streams/tv/:tmdbId` or `/api/play/tv/:tmdbId`** — those expect TMDB IDs.

## Adding a new provider

1. Implement the low-level client in `src/providers/<name>.rs`.
2. Add a `*_provider.rs` wrapper implementing `StreamProvider`.
3. Register it in `src/app.rs:build_state`.
4. Add display name / metrics label to the metrics and circuit breaker lists.
5. Update `PROVIDERS.md` if provider capabilities change.

## Adding a new route

1. Add handler in `src/routes/<module>.rs`.
2. Register in `src/app.rs:build_router`.
3. Add to `src/openapi.rs` if it should appear in Swagger UI.

## Common pitfalls

- `APP_SECRET` is used for AES-CBC token encryption. Changing it invalidates all in-flight proxy URLs.
- Provider timeouts and circuit breakers are global. Failing providers are opened/closed automatically.
- Proxy routes must stay exempt from rate limiting and API key checks.
- The `reqwest` retry middleware uses HTTP/1.1 for some providers (e.g. Vixsrc) to avoid Cloudflare bot management.
