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
| `API_KEY` | — | Optional `x-api-key` header for API routes |

## Architecture

See [ARCHITECTURE.md](ARCHITECTURE.md) for module map, request flow, and proxy pipeline.

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
