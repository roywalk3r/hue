# Research Report: Hue Stream API — Extensive Analysis

**Session ID:** hue-analysis-20260507  
**Date:** 2026-05-07  
**Branch:** feat/branding  
**Version:** 1.0.9  
**Status:** Complete (Verified)

---

## Executive Summary

**hue-stream-api** is a professional streaming aggregation platform running as a 4-process microservices architecture (Node.js API + Rust engine + Node scraper + Redis) with a premium dark-mode web UI. It aggregates 20+ providers across movies/series, Asian drama, anime, and live sports — all branded with public codenames that obscure real sources.

The `feat/branding` branch (6 commits, 268 files, +6,353 net lines) introduced two things simultaneously: cosmetic professional headers across the codebase, and an architectural TOML manifest system that defines provider identity, runtime assignment, and behavior config. The manifest system is infrastructure-complete but partially un-wired — the `priority` and `circuit_breaker` fields are declared but not yet consumed by the dispatch loop.

---

## Methodology

### Research Stages

| Stage | Focus | Tier | Status |
|-------|-------|------|--------|
| 1 | Core Architecture & API Server | MEDIUM | complete |
| 2 | Provider System & Manifests | MEDIUM | complete |
| 3 | Rust Component (huerust) | MEDIUM | complete |
| 4 | Frontend / Public UI | LOW | complete |
| 5 | Services & Utils Infrastructure | MEDIUM | complete |
| 6 | Branding Branch Changes | LOW | complete |
| 7 | Deployment & DevOps | LOW | complete |
| V | Cross-Validation | HIGH | passed |

7 parallel scientist agents executed stages 1–7, followed by a cross-validation agent that identified 5 conflicts (all minor/corrected in this report).

---

## Architecture

### Process Map

```
Client → Express API Server (Node.js :24608)
            ├── Rust Engine (huerust :24609)    ← fast-path API providers
            ├── Scraper Microservice (:24610)   ← Playwright fallback (local only)
            └── Redis (:6379)                   ← cache (Docker deployment only)
```

### Request Flow (happy path)

```
Request → CORS → Body Parse → Metrics Logger
  → Check in-flight coalescing (pendingFetches Map)
  → Check aggregate cache (4h TTL)
  → Fetch TMDB metadata (parallel)
  → Split providers: fast vs slow
  → Race fast providers → 5s collective deadline
  → If no results: race all providers → 15s collective deadline
  → Merge results (URL-dedup), store in cache
  → Apply quality/codec filters
  → Rewrite URLs through proxy if enabled
  → Return { streams, providerTimings, status: "Complete"|"Partial" }
```

### API Surface

76 routes across 11 route files:

| Mount | File | Purpose |
|-------|------|---------|
| `/api/streams` | routes/streams.js | Core aggregation, SSE live, drama, anime |
| `/api/hitv` | routes/hitv.js | HiTV browse/search/play (15 routes) |
| `/api/moviebox` | routes/moviebox.js | MovieBox content (21 routes) |
| `/api` | routes/sports.js | AiSports, PPV, LiveTV, sports-stream |
| `/api` | routes/drama.js | KissKH, Dramacool search/streams |
| `/api` | routes/system.js | Config, cache, health, auth |
| `/api/proxy` | routes/proxy.js | Generic proxy |
| `/api/livetv` | routes/livetv.js | LiveTV matches |
| `/api/play` | routes/play.js | Stream playback shortcut |
| `/api/tmdb` | routes/tmdb.js | TMDB search/trending |
| `/api/anime` | routes/anime.js | Anime search/stream |

### Caching (3 layers)

1. **In-flight coalescing** — identical concurrent requests share one Promise
2. **Per-provider cache** — 4h TTL, keyed `provider:type:tmdbId`
3. **Aggregate stream cache** — 4h TTL, URL-deduplicated merge, max 1000 entries

---

## Provider System

### Full Inventory (verified)

| Public Name | Real Name | Vertical | Runtime | Priority |
|---|---|---|---|---|
| Aura | vixsrc | Movie/Series | Both | 90 |
| Cinema | moviebox | Movie/Series | Both | 85 |
| Zenith | kisskh | Drama | Both | 80 |
| Stadium | livesport | Sports | Node | 80 |
| Orient | dramacool | Drama | Both | 75 |
| Field | aisports | Sports | Node | 75 |
| Blaze | autoembed | Movie/Series | Both | 70 |
| Arena | livetv | Sports | Node | 70 |
| Nova | sflix | Movie/Series | Both | 70 |
| Hydra | mp4hydra | Movie/Series | Both | 65 |
| Event | ppv | Sports | Node | 65 |
| Luna | vidrock | Movie/Series | Node | 60 |
| — | showbox, 9anime, anikai, uhdmovies, 4khdhub, moviesmod, vidsrc-embed, streamed | Various | Node | 50 (default) |

Total: 29 provider files, 2 file-level disabled, ~7 registry-disabled. When `enableHuerustProvider=true`, the 5 "Both" runtime providers are suppressed in Node — Rust handles them.

### Execution Strategy

- **Fast pool**: vixsrc, moviebox, huerust, autoembed, 4khdhub, moviesmod, uhdmovies
- **Fast deadline**: 5s collective, 15s individual timeout
- **Slow deadline**: 15s collective, 60s individual timeout
- **Circuit breaker**: 3 failures → 10min cooldown (in-memory, resets on restart)
- **Drama exclusion**: `dramacool`/`orient` filtered from `/api/streams` — only in drama-specific routes

### Manifest System

12 TOML manifests in `providers/manifests/`. Schema:

```toml
[provider]
name = "aura"
label = "Vixsrc"
runtime = "both"
media_types = ["movie", "series"]

[capabilities]
search = true
streams = true
episodes = true

[behavior]
timeout_ms = 15000
max_retries = 2
circuit_breaker = true
priority = 90

[test]
fixture_id = "tt0111161"
fixture_type = "movie"
expect_streams = true
```

**Gap:** `priority` and `circuit_breaker` manifest fields are loaded but not consumed by `routes/streams.js` or `providers/registry.js`. 8 of 20 active providers have no manifest.

---

## Rust Engine (huerust)

Standalone Axum/Tokio HTTP server. 6 registered providers behind `StreamProvider` trait:

| Public Name | Real Name | Source Method |
|---|---|---|
| Cinema | MovieBox | aoneroom.com API |
| Nova | SFlix | aoneroom.com API |
| Zenith | KissKH | kisskh.ovh API |
| Orient | Dramacool | api.xyra.stream |
| Aura | VixSrc | HTML regex scraping |
| Hydra | Mp4Hydra | API |

**Security:**
- AES-256-CBC encrypted URL tokens (scrypt key derivation, 4h TTL)
- Static salt `'salt'` used in both `huerust/src/security.rs` AND `utils/security.js`
- API key + 120 req/60s per-IP rate limiting
- Loopback IPs bypass auth unconditionally

**Critical:** Both hardcoded fallback JWTs are expired as of 2026-05-07:
- SFlix: expired ~April 10, 2026
- MovieBox: expired ~May 16, 2026

Without `user-config.json` overrides these providers fail silently.

**HiTV:** 12-hour background library builder (Korea/China/Thailand/Japan × Drama/Movie/Variety × 5 pages).

**Node bridge:** SFlix and Dramacool in Rust fall back to `POST {MICROSERVICE_URL}/scrape/{provider}` when direct API returns empty.

---

## Frontend / UI

5 pages, vanilla JS + Bootstrap 5 CDN, no build step:

| Page | Function |
|---|---|
| index.html | Discovery — hero carousel (8s autoplay), 7-source tab nav, search, infinite scroll |
| player.html | Stream Lab — HLS.js + Dash.js, 3-tier fallback, download buttons |
| config.html | Admin — provider toggle matrix, quality presets, TMDB key manager, live JSON |
| docs.html | OpenAPI via Scalar (`/api/docs/openapi.json`) |
| login.html | Auth with rate-limit lockout countdown |

**Design system:**
- Background: `#0c101a` (very dark blue)
- Accent: `#31d7a9` (teal) + `#7c5dff` (purple)
- Glassmorphism: `backdrop-filter: blur(12px)` panels
- Typography: Space Grotesk + DM Mono (player), system-ui (config)
- Responsive: 4-col → 2-col → 1-col grid

**Player:** VidZee AES-CBC decryption with hardcoded key `"qrincywincyspider"`. Proxy URLs inject headers browsers cannot set. Watchdog timeouts 15–45s per provider.

---

## Services & Infrastructure

| Component | Detail |
|---|---|
| `browserManager.js` | Playwright singleton, mutex launch guard, 10-page semaphore, 26 Chromium hardening flags, resource blocking |
| `playwrightResolver.js` | Navigate embed → intercept `.m3u8` → click selectors across all iframes → priority-sort by resolution |
| `tokenWorker.js` | 15-min interval: browser-refresh SFlix + MovieBox cookies, persist via `saveConfigPatch()` |
| `warmupWorker.js` | 1-hour interval: pre-cache top-20 TMDB popular (fast providers only). **Bug:** `currentYear=2025` hardcoded — silently skips 2026 content |
| `interceptor.js` | Canonical shared Playwright ad-blocker (13 domain patterns) — not used by all providers |
| `hianime.js` | HTTP-only `myani.cfd` client; vendored library fallback |
| `livetvScraper.js` | Cheerio → `livetv.sx`; Playwright for final stream URL |
| `jsunpack.js` | P.A.C.K.E.R decoder (radix 2–95) |
| `logger.js` | Runtime secret masking, IP anonymization, file + console |
| `tmdb.js` | 8 TMDB endpoints, random-key pool rotation, 6h cache |
| `auth.js` | PBKDF2-SHA512 (100K iter), custom HMAC sessions, 12h TTL |
| `config.js` | Layered config: user-config.json > env vars; hot-reload via `saveConfigPatch()` |

---

## Branding Branch

| Commit | Change |
|---|---|
| `849b8160` | Professional `@author/@license/@repository` JSDoc headers on all 268 files |
| `b28641bf` | 12 provider TOML manifests |
| `a6ac39dc` | `utils/manifestLoader.js` — singleton loader |
| Various | Rust engine expansion (5.8MB → 24.5MB), new route handlers, Docker Compose, PM2 |
| `3ebd0bfb`/`e518ce70` | Design spec + 1,869-line implementation plan |
| `1a7488c3` | Autoembed priority alignment fix |

---

## Deployment

| Target | Status | Notes |
|---|---|---|
| Docker Compose | Production-ready | 4 services, healthchecks, non-root user, Redis |
| Vercel | Incompatible | No browser, 60s timeout, stateless — only lightweight routes work |
| PM2 | Stale/dev | Port conflict: assigns 24608 to Rust; Docker gives 24608 to Node |
| GitHub Actions | Well-configured | Docker Hub publish on push; 5 Gemini AI workflows |

---

## Verified Issues

| # | Issue | Severity | Location |
|---|---|---|---|
| 1 | Both hardcoded JWTs expired | High | `huerust/src/providers/sflix.rs`, `moviebox.rs` |
| 2 | Manifest priority/circuit_breaker unconnected to dispatch | Medium | `routes/streams.js`, `providers/registry.js` |
| 3 | Static scrypt salt `'salt'` on both sides | Medium | `huerust/src/security.rs`, `utils/security.js` |
| 4 | PM2 port conflict (Rust: 24608 vs Docker: 24609) | Medium | `ecosystem.config.js` |
| 5 | `warmupWorker.js` `currentYear=2025` hardcoded | Medium | `services/warmupWorker.js` |
| 6 | Incomplete ad-blocking coverage (`kisskh.js` uses inline blocking) | Low | `utils/interceptor.js`, `providers/kisskh.js` |
| 7 | All state in-process — no horizontal scaling coordination | Medium | cache, circuit breaker, browser pool |
| 8 | Vercel + Docker configs coexist but are incompatible for full use | Low | `vercel.json`, `docker-compose.yml` |
| 9 | No drama aggregate endpoint (providers excluded, no replacement) | Low | `routes/streams.js`, `routes/drama.js` |

---

## Cross-Validation Corrections

- Route count: **76** (not 74)
- Provider files: **29 total, 2 file-disabled** (not 25/5)
- SFlix JWT expiry: **April 2026** (not March 2026) — still expired
- Branding branch did NOT move Rust JWT tokens to env vars — hardcoded constants remain
- Ad-blocking is one shared util + one provider bypassing it (not two full implementations)

---

## Recommendations

1. **Immediate:** Update SFlix and MovieBox JWTs in `user-config.json` or document the token refresh flow prominently — both hardcoded defaults are now broken.
2. **Short-term:** Wire manifest `priority` into the fast/slow provider split in `registry.js` to make the manifest system functional end-to-end.
3. **Short-term:** Fix `warmupWorker.js` year cutoff — replace `currentYear = 2025` with `new Date().getFullYear() - 1`.
4. **Medium-term:** Use a non-static scrypt salt (e.g., derive from `APP_SECRET` hash) in both `security.rs` and `utils/security.js`.
5. **Medium-term:** Add a drama aggregate endpoint parallel to `/api/streams/:type/:tmdbId` to restore drama provider parity.
6. **Long-term:** Move cache and circuit breaker state to Redis to enable horizontal scaling.
7. **Long-term:** Clarify deployment target — remove Vercel config or clearly document which features are Vercel-compatible.
