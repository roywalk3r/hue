# Stage 5: Services & Utils Infrastructure

**Tier:** MEDIUM | **Status:** Complete

## Key Findings

### Browser Management (browserManager.js)
- Singleton browser with promise-chained mutex (prevents concurrent spawns)
- 10-page global semaphore (`MAX_GLOBAL_CONCURRENT_PAGES`), queue-based backpressure
- `context.close()` monkey-patched with `closed` guard to prevent double semaphore release
- Resource blocking: aborts image/font/stylesheet + 9 ad/tracking domains
- Allows: `.m3u8`, `.mp4`, `.ts`, `challenges.cloudflare.com`, `turnstile`
- ~26 Chromium hardening flags including `--disable-blink-features=AutomationControlled`

### Playwright Resolver (playwrightResolver.js)
- Navigates embed URL, intercepts all `request` events for `.m3u8` patterns
- Early exit: resolves immediately on `index.m3u8`, `master.m3u8`, `playlist.m3u8`
- Clicks up to 5 selectors across ALL iframes: `#player`, `video`, `.play-button`, `button`, `canvas`, `.ratio-16x9`, `body`, `iframe`
- Quality priority: 4k=4, 1080=3, 720=2, else 1 — sorted descending
- Hard 10s timeout, `Promise.race` for early exit
- Default referer: `https://fbstreams.pm/`
- Captured request headers included in results (enables authenticated proxy passthrough)

### Stealth (stealth.js)
- 6 UAs: 4 desktop (Windows/Mac/Linux Chrome 120, Firefox 121) + 2 mobile (iOS Safari 17, Android Chrome 120)
- Full `Sec-Fetch-*` header suite, correctly set based on referer presence
- `jitter(500, 1500)`: timing humanization via `Math.random()`
- `anonymize(input)`: SHA-256 truncated to 8 chars for PII-safe logging

### Caching (cache.js)
- `ttl-cache`: 4h TTL, 1000-entry LRU cap
- Key: `{type}_{tmdbId}_{season}_{episode}`
- `addCachedStreams()`: URL-dedup merge via Map, new overwrites existing same-URL entry
- **In-process only — no Redis, no persistence across restarts**
- TMDB has its own separate `Map`-based cache (6h TTL, 4 independent maps)

### Authentication (auth.js + security.js)
**Layer 1 — Admin sessions:**
- Users in `utils/auth-users.json`, PBKDF2-SHA512 (100K iter, random 16-byte salt)
- Custom JWT-like: `base64url(payload).HMAC-SHA256-sig`
- Ephemeral session secret — all sessions invalid on restart
- `crypto.timingSafeEqual` for password comparison (correct)
- Default: admin/change-me, auto-created with console.warn

**Layer 2 — Stream URL tokens:**
- AES-256-CBC, scrypt key derivation
- **Static salt `'salt'` — weakness on Node side (also on Rust side)**
- `APP_SECRET` fallback: `'super-secret-hue-key-change-me'` hardcoded if env unset
- `decryptUrl()` validates AES block alignment before decryption (avoids padding oracle)

### TMDB (tmdb.js + tmdbKey.js)
- Random key selection from pool (`Math.random()`, not round-robin)
- Fallback: `config.tmdbApiKeys[]` → `config.tmdbApiKey` → `TMDB_API_KEY` env
- 8 endpoints: info, external IDs, details, season, search (multi/movie/tv), popular, now-playing, trending
- 15s axios timeout; 6h cache per endpoint

### Stream Filters (streamFilters.js)
- Quality numeric mapping: org=4320, 4k/2160=2160, 1080/auto=1080, 720/hd=720, 480/sd=480, cam=100, ts=200
- Codec filtering: checks `stream.codecs` array for DV/HDR/HDR10/HDR10+
- `applyFilters()` reads per-provider or global `minQualities` config
- Provider-specific override not currently used (only `minQualitiesConfig.default` read)

### Interceptor (interceptor.js)
- Canonical shared Playwright ad-blocker (13 domain patterns, more than browserManager.js)
- Also blocks: googletagmanager, facebook, twitter (browserManager.js does not)
- Allows `.workers.dev` (Cloudflare Workers)
- `isStreamUrl()`: excludes `.vtt`/`.srt`/`/subtitle/` before checking video extensions
- **Not used by all providers** — at least `kisskh.js` implements own inline blocking (inconsistency)

### Background Services
**tokenWorker.js:**
- 15-min interval, runs immediately on startup
- Sequential (not parallel) refresh to avoid resource spikes
- Mutex guard (`isRefreshing`) prevents concurrent cycles
- refreshSflix: navigates sflix.film, networkidle (90s timeout), extracts `sflix_token` + UA
- refreshMoviebox: tries 4 domains in sequence, extracts `mb_token` + `uuid` + UA

**warmupWorker.js:**
- 1-hour interval, 60s startup delay
- Fetches 40 TMDB popular → filters → slices to 20
- **Bug: `currentYear = 2025` hardcoded → silently skips all 2026 content**
- Year filter: must be ≤ currentYear, must have release date, must not contain '2026'/'2027' in title
- Only uses `p.enabled && p.isFast` providers
- 5s gap between items, skips already-cached entries

### HiAnime (services/hianime.js)
- HTTP-only client against `myani.cfd` API (no Playwright)
- 3 domain fallbacks
- Streams from `megaplay.buzz/stream/getSources`
- Hex ID detection: `/^[0-9a-fA-F]{24}$/` (MongoDB ObjectID)
- Prefers `format === 'json'` over embed for HLS; falls back to iframe

### LiveTV Scraper (services/livetvScraper.js)
- 4-stage: getSports → getMatches → getStreamVariants → resolveStream
- SSL: `rejectUnauthorized: false` (accepts self-signed)
- Sport IDs: 1=Football, 2=Ice Hockey, 3=Basketball, 4=Tennis
- Match dedup via Map before returning

### JS Unpacker (utils/jsunpack.js)
- Full P.A.C.K.E.R decoder (radix 2–95)
- Detection: `/eval\(function\(p,a,c,k,e,(?:r|d)/`
- Reconstructs symbol table, replaces tokens with symtab lookups

### Logger (utils/logger.js)
- `registerSecret(str)` — runtime secret masking (min length 5)
- IPv4: replaces last octet with `***`
- API key/token URL params masked
- Dual output: colorized console + `fs.appendFileSync('app.log')`
- Levels: DEBUG(0) INFO(1) WARN(2) ERROR(3) NONE(4)
