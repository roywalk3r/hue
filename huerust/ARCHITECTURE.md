# Architecture

## Module Map

```
src/
  main.rs                 — Tokio runtime, config load, server start
  app.rs                  — AppState + state-group FromRef impls, build_state (provider
                            registry + circuit breaker wiring), build_router (route table),
                            auth_middleware, vixsrc_enc_key_handler
  config.rs               — Env-var based configuration (AppConfig, validate_runtime)
  models.rs               — StreamResult, Subtitle, MovieBoxCard
  models/anime.rs         — AnimeCard, AnimeDetail, EpisodeSummary, AnimeRail
  openapi.rs              — utoipa OpenAPI spec
  security.rs             — Proxy URL HMAC signing, API token mint/verify
  filters.rs              — Stream filtering rules (URL deny list, provider scoring)
  http_client.rs          — reqwest client builder + retry/connect-timeout middleware
  user_config.rs          — Per-user stream filter overrides (loaded from .env-sidecar)
  microservice.rs         — Internal microservice scaffolding
  constants.rs            — Compile-time string constants
  clients/                — External API/scraper clients (no provider pipeline)
    tmdb.rs               — TMDB metadata + search (key rotation, moka cache)
    moviebox.rs           — MovieBox API client (browse, search, detail, streams)
    hitv.rs               — HiTV drama aggregation client
    hianime.rs            — Anime scraper / stream extractor
    vidrock.rs            — VidRock AES-encrypted API client
    vidlink.rs            — Vidlink client
    vidvault.rs           — Vidvault (disabled — hakunaymatata CDN 403s)
    vidsrcembed.rs        — Vidsrc.to embed scraper
    notorrent.rs          — NoTorrent (hianime-hosted) client
    lordflix.rs           — Lordflix (disabled — Cloudflare attestation)
    animekill.rs          — AnimeKill scraper
    animexin.rs           — Animexin scraper
    anilist.rs            — AniList GraphQL client
    aniwatch.rs           — AniWatch client
    aniwave.rs            — AniWave client
    anime_agg.rs          — AnimeAggregator fan-out/dedup (AniList, AniWatch, HiAnime)
    aisports.rs           — AiSports scraper (powers /api/aisports/*)
  providers/              — Stream provider implementations (client + StreamProvider)
    mod.rs                — StreamProvider trait, provider name → display name map
    registry.rs           — ProviderRegistry with circuit breaker integration
    dispatch.rs           — Parallel provider fetch orchestration (fast/slow phases)
    moviebox_provider.rs  — MovieBox stream source
    vidrock_provider.rs   — Vidrock streams
    vidlink_provider.rs   — Vidlink streams
    vidzee.rs / _provider — VidZee two-layer AES decryption
    vidvault_provider.rs  — Vidvault (disabled)
    vidsrcembed_provider  — Vidsrc.to embed
    vixsrc.rs / _provider — Vixsrc.to
    kisskh.rs / _provider — KissKH (Oriental)
    kissasian.rs / _provider — KissAsian (Oriental)
    kissasian_top.rs / _provider — KissAsian.top (Rose)
    kisskh_xyra.rs / _provider — KissKH via xyra streamhub (disabled)
    dramacool.rs / _provider — Dramacool (disabled)
    sflix.rs / _provider  — SFlix
    mp4hydra.rs / _provider — MP4Hydra (disabled)
    notorrent_provider.rs — NoTorrent (hianime-hosted) + fake-iframe filter
    videasy.rs / _provider — Videasy (disabled)
    lordflix_provider.rs  — Lordflix (disabled)
    ppvto.rs / _provider  — PPV.to sports
    xyra_aurex.rs / _provider — Xyra streamhub `aurex-alpha` (drama aggregator)
    xyra_vixsrc.rs / _provider — Xyra streamhub `vixsrc` (Aura) provider
    xyra_subtitle.rs      — Xyra subtitle lookup
    vdrk_subtitles.rs     — Vidrock subtitle pipeline
    streamhub.rs / _provider — Generic xyra streamhub proxy (disabled — Aura duplicates)
  routes/                 — Axum route handlers
    streams.rs            — /api/streams/:type/:tmdbId (HTTP) + /luna + /xyra
    sse.rs                — /api/streams/:type/:tmdbId/live (SSE), report-failure, warm
    play.rs               — /api/play/:type/:tmdbId (Smart Play)
    drama.rs              — /api/drama/* (kisskh, kissasian, kissasian_top, moviebox)
    moviebox.rs           — /api/moviebox/* (home, browse, info, filter, etc.)
    hitv.rs               — /api/hitv/* (browse, search, play, dynamic routes)
    anime.rs              — /api/anime/* (search, detail, episodes, stream, discover, schedule)
    sports.rs             — /api/sports/* and /api/aisports/* (leagues, matches, streams)
    tmdb.rs               — /api/tmdb/* (home, popular, top-rated, trending, discover, search)
    subtitle.rs           — /api/subtitle/* (disabled — xyra faulty)
    download.rs           — /api/download/* (HLS download endpoint)
    system.rs             — /health, /metrics, /api/status, /api/auth/token, admin reload
    providers.rs          — /api/providers (registry snapshot for the UI)
    mod.rs                — route module re-exports
  proxy/                  — HLS/TS/Subtitle proxy engine
    mod.rs                — m3u8-proxy, ts-proxy, sub-proxy, generic_proxy handlers
    wrap.rs               — StreamResult → proxied-h_token URL wrapping
    adsurgery.rs          — HLS manifest ad segment removal
    sanitize.rs           — TS packet sanitization
  infra/                  — Infrastructure modules
    cache.rs              — StreamCache (moka / redis)
    circuit_breaker.rs    — Per-provider circuit breaker
    coalesce.rs           — FetchCoalescer (deduplicates concurrent identical requests)
    metrics.rs            — Prometheus-style metrics
    rate_limit.rs         — IP-based sliding window rate limiter
    provider_score.rs     — Per-title provider popularity score (drives priority)
    sticky_pref.rs        — Per-user "which provider works for me" sticky preference
```

## Runtime configuration

All config is env-var driven. See `src/config.rs` for `AppConfig` and the
`validate_runtime()` rules. See [README.md](README.md) for the env-var table.

Key derived things the docs need to call out:

- **Bind**: `RUST_HOST` / `RUST_PORT` (default `127.0.0.1:24609`).
- **Public proxy base**: `RUST_PROXY_URL` is what the server tells clients
  to use as the `Host` of `h_token` rewrites. In production this must
  match the public Rust origin (e.g. `https://api.vexli.online`) — the URL
  embedded in every h_token rewrite comes from here.
- **Kkey**: `KKEY` env var is a hex key used by the *Flutter* client to
  decrypt the `{epoch_hour}:{hex(xor)}` URL transport format. The Rust
  proxy does not decode it; that responsibility lives on the client.

## Request flow: stream aggregation

The two endpoints `GET /api/streams/:type/:tmdbId` (JSON) and
`GET /api/streams/:type/:tmdbId/live` (SSE) share the same provider fan-out
machinery. The JSON endpoint does a one-shot fetch and returns the final
state; the SSE endpoint streams progress events as providers resolve.

```
Client ──► /api/streams/movie/27205
    │
    ▼
┌─────────────────────────────────────────────────────────────┐
│  1. Coalescer check — is an identical request already       │
│     in flight? If yes, attach to its in-flight future.      │
│                                                              │
│  2. Sticky-pref lookup — if a previous successful provider  │
│     is recorded for this user/title, it goes first.         │
│                                                              │
│  3. Cache check — do we have fresh cached results?          │
│     (TTL = CACHE_TTL_SECS, default 4 hours)                 │
│                                                              │
│  4. Fast phase — fire fast-only providers in parallel.       │
│     Each provider has its own per-call timeout (15–60s).   │
│     Every fast provider gets the full fast deadline.         │
│     (No abort-on-threshold — see dispatch.rs note.)          │
│                                                              │
│  5. Slow phase (if fast phase was thin) — fire all healthy  │
│     providers (fast + slow). Aborts early once we have      │
│     enough streams to satisfy the user.                      │
│                                                              │
│  6. Filtering — dedup URLs, drop blacklisted domains,       │
│     drop iframe-only streams, apply provider scoring.       │
│                                                              │
│  7. Wrap with proxy — encrypt upstream URLs into h_token     │
│     query params, inject CDN headers, attach subtitles.      │
│                                                              │
│  8. Background fill (cache miss case) — after responding,   │
│     fill the cache from the slow phase for next time.       │
│                                                              │
│  9. Cache write + JSON or SSE response                      │
└─────────────────────────────────────────────────────────────┘
```

### Fast vs slow phase

Providers self-classify via `StreamProvider::is_fast()`. Fast providers
are expected to land in < 5 s; slow providers may take 30+ s. The
`FAST_PROVIDER_DEADLINE_SECS` (default 5) and
`SLOW_PROVIDER_DEADLINE_SECS` (default 25) env vars cap the two phases.

The fast phase intentionally does **not** abort early on threshold, because
aborting a still-resolving Vixsrc just to ship 3 Luna streams made Vixsrc
never contribute. The slow phase does abort on threshold to bound latency.

The "is this thin?" check between phases is a `min_streams_threshold`
defaulted to 3; if the fast phase returns fewer, the slow phase runs.

## Request flow: Xyra fast-path

The xyra fast-path is a separate, parallel route that returns in < 1 s on
the happy path and is **raced** by the browser-side `useHueStreamSSE` (Nuxt)
or `SseStreamAggregator.aggregate` (Flutter) against the main JSON endpoint.

```
Client ──► /api/streams/xyra/movie/27205  ──► api.xyra.stream/v1/streamhub
                                              (12 s timeout, no body shaping)
    │
    ▼
┌─────────────────────────────────────────────────────────────┐
│  1. Auth-rate budget — 12 s, then 502 / 504                  │
│  2. JSON parse xyra response into XyraStream[]               │
│  3. Normalize to StreamResult (provider name, quality,       │
│     format, headers)                                         │
│  4. wrap_with_proxy → encrypt URLs into h_token              │
│  5. Return standard StreamsResponse                          │
│     (same shape as /api/streams/... — browser doesn't know   │
│      which one answered first)                               │
└─────────────────────────────────────────────────────────────┘
```

Race semantics live on the client. The browser fires both endpoints in
parallel and consumes whichever returns playable streams first.

The xyra handler is **not** subject to the provider circuit breaker — it
sits outside the Rust provider pipeline. When xyra upstream is degraded
the client self-heals via a cooldown (Nuxt: `NUXT_PUBLIC_XYRA_ENABLED`
flag; Flutter: 2-strikes-then-5-min-skip in `sse_aggregator.dart`).

See [docs/xyra.md](docs/xyra.md) for the full integration.

## Request flow: Smart Play

`GET /api/play/:type/:tmdbId` is the "just give me one playable URL"
endpoint. It runs the same fast-phase fan-out, then for each candidate
URL does a `HEAD` check (timeout `HEAD_CHECK_TIMEOUT_SECS`, default 2 s)
to verify the upstream is alive. It picks the first HLS stream that
returns 200 and returns just that URL, no metadata.

If no fast-phase stream passes the HEAD check, the slow phase runs.
This endpoint exists to be embedded in `<a href="...">` style links
without forcing the client to render a stream picker.

See [docs/smart-play.md](docs/smart-play.md) for the selection algorithm.

## Proxy pipeline

```
Upstream URL ──► sign_proxy_url() ──► h_token query param
    │
    ▼
/m3u8-proxy?h_token=...        (manifests)
/ts-proxy?h_token=...          (segments)
/sub-proxy?h_token=...         (subtitle files)
/api/proxy?url=...&headers=... (generic, untrusted caller passthrough)
    │
    ▼
┌─────────────────────────────────────────────────────────────┐
│  1. Verify h_token HMAC signature + expiry                  │
│  2. CDN header injection by upstream host:                  │
│     • vixsrc.to        → Referer: https://vixsrc.to/       │
│     • vix-content.net  → Referer: https://vixsrc.to/       │
│     • hakunaymatata / aisports / poocloud                   │
│                         → Referer: https://moviebox.ph/     │
│     • mewstream / animeplay                                 │
│                         → Referer: https://hianimes.se/     │
│  3. Fetch upstream manifest / segment / subtitle            │
│  4. Manifest rewrite: relative → absolute, segment URLs     │
│     re-wrapped through proxy (token or url mode)            │
│  5. Optional ad surgery: strip ad segments from manifest     │
│  6. Optional TS sanitize: drop garbage NALU / junk packets   │
│  7. Segment prefetch: spawn background task to fetch the     │
│     first 5 segments after manifest rewrite                 │
└─────────────────────────────────────────────────────────────┘
```

`is_whitelisted_raw_url` short-circuits the proxy for hosts whose CDNs
IP-block datacenters (MovieBox, Pixeldrain, Google User Content,
promoxy.xyra.stream) — those are returned as plain HTTP redirects and
the client fetches them directly.

See [docs/proxy.md](docs/proxy.md) for the proxy token format, CDN
header injection matrix, and host whitelist rules.

## Authentication & rate limiting

- **API routes** (`/api/*`, `/health`, `/metrics`): Bearer token validated
  by `auth_middleware` (HMAC-signed by `APP_SECRET`, `TTL_MS` enforced,
  loopback exempt). `/api/auth/token` mints a new token. The token format
  is `sign_api_token(secret, client_ip, ttl_ms)` — IP-bound, expires.
- **Proxy routes** (`/m3u8-proxy`, `/ts-proxy`, `/sub-proxy`,
  `/api/proxy`): Self-authenticated via `h_token` HMAC signature in the
  query string. No Bearer token required. The signature covers the URL,
  expiry, and any per-host headers. See [docs/proxy.md](docs/proxy.md).
- **Rate limiting**: `rate_limit_middleware` — 120 requests / 60 s per
  IP, sliding window. Proxy routes are exempt. The auth endpoint has
  its own per-IP budget to prevent token-spam abuse.
- **CORS**: `CorsLayer::permissive()` — open for browser clients.
- **Compression**: `CompressionLayer` for response bodies.
- **Tracing**: `TraceLayer` for request logs.

## Background tasks

`spawn_background_tasks` (called once at startup from `app.rs`):

1. **HiTV library builder** — rebuilds the in-memory library every 12 h.
2. **Rate-limiter eviction** — sweeps stale per-IP buckets every 5 min.
3. **Anime aggregator refresh** — if a periodic refresh is configured.

## Dual backend strategy

`huerust` is the modern streaming engine. The Node.js backend at
`127.0.0.1:24608` still owns:

- User auth (login / password / session)
- Live TV & sports (HLS streams proxied through a Node-side worker)
- TMDB proxy (some legacy routes)
- Playwright scraper fallback (when an upstream JS-renders)
- Static UI pages (the legacy web shell)

The Rust backend owns everything stream-related: aggregation, proxy
engine, drama, anime, moviebox, hitv, sports metadata.

## Subsystem deep-dives

For how each subsystem actually works under the hood, see:

- [docs/xyra.md](docs/xyra.md) — Xyra streamhub integration
- [docs/drama.md](docs/drama.md) — drama pipeline (kisskh, kissasian, moviebox)
- [docs/anime.md](docs/anime.md) — anime aggregator
- [docs/subtitles.md](docs/subtitles.md) — subtitle lookup (xyra, vdrk, kisskh)
- [docs/provider-scoring.md](docs/provider-scoring.md) — sticky preferences, popularity, circuit breakers
- [docs/proxy.md](docs/proxy.md) — proxy token format, CDN header injection, host whitelist
- [docs/smart-play.md](docs/smart-play.md) — `/api/play` head-check selection
- [docs/notorrent.md](docs/notorrent.md) — NoTorrent provider and the fake-iframe filter
