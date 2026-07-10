# huerust — Hue Streaming Engine

A high-performance Rust backend for the Hue streaming platform. Aggregates video streams from multiple third-party providers, proxies HLS/TS/subtitle traffic through encrypted tokens, and serves browse/search/detail APIs for drama, anime, and movies.

## Quick Start

```bash
cd huerust
cargo run
```

The server binds to `127.0.0.1:24609` by default.

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `RUST_HOST` | `127.0.0.1` | Bind host |
| `RUST_PORT` | `24609` | Bind port |
| `RUST_PROXY_URL` | `http://{host}:{port}` | Public proxy base URL |
| `APP_SECRET` | `hue-streaming-stable-secret-2026` | AES-CBC encryption key |
| `TMDB_API_KEYS` | — | Comma/semicolon/newline separated TMDB keys |
| `CACHE_TTL_SECS` | `14400` | Stream cache TTL (4 hours) |
| `CACHE_MAX_CAPACITY` | `1000` | Moka cache max entries |
| `FAST_PROVIDER_DEADLINE_SECS` | `5` | Fast provider batch timeout |
| `SLOW_PROVIDER_DEADLINE_SECS` | `25` | Slow provider batch timeout |
| `HEAD_CHECK_TIMEOUT_SECS` | `2` | Smart Play HEAD validation timeout |
| `ENABLE_PROXY` | `true` | Enable HLS/TS/subtitle proxy |
| `CACHE_BACKEND` | `memory` | `memory` or `redis` |
| `REDIS_URL` | `redis://127.0.0.1:6379` | Redis connection string |
| `API_KEY` | — | Optional Bearer token for API routes. See [ARCHITECTURE.md](ARCHITECTURE.md) → "Authentication & rate limiting" for the token format. |

## Architecture

See [ARCHITECTURE.md](ARCHITECTURE.md) for module map, request flow, proxy
pipeline, and the dual-backend strategy.

## Deep-dives

For how each subsystem actually works under the hood:

- [docs/xyra.md](docs/xyra.md) — Xyra streamhub integration
  (fast-path race + `Aurex` / `Aura (Xyra)` providers)
- [docs/drama.md](docs/drama.md) — drama pipeline
  (MovieBox, KissKH / Zenith, KissAsian / Pearl, KissAsian.top / Rose)
- [docs/anime.md](docs/anime.md) — anime aggregator
  (AniList + AniWatch + HiAnime, AniWave, AnimeKill)
- [docs/subtitles.md](docs/subtitles.md) — subtitle pipeline
  (KissKH embedded, Vidrock fallback)
- [docs/provider-scoring.md](docs/provider-scoring.md) — sticky
  preferences, popularity, circuit breakers
- [docs/proxy.md](docs/proxy.md) — proxy token format, CDN header
  injection matrix, host whitelist
- [docs/smart-play.md](docs/smart-play.md) — `/api/play` HEAD-check
  selection algorithm
- [docs/notorrent.md](docs/notorrent.md) — NoTorrent provider and
  the fake-iframe filter

## API Reference

See [API.md](API.md) for complete endpoint documentation.

## Provider Registry

See [PROVIDERS.md](PROVIDERS.md) for provider name mappings, capabilities, and timeouts.

## OpenAPI / Swagger

Interactive docs are served at `/api/docs` when the server is running.

## Build

```bash
cargo build --release
```

## Test

```bash
cargo test
```
