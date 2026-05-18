# Stage 1: Core Architecture & API Server

**Tier:** MEDIUM | **Status:** Complete

## Key Findings

### Server Design
- Dual-export: Express app on port 24608, also exported as `module.exports = app` for Vercel
- 3-process model: API server (24608), Rust engine (24609), Scraper microservice (24610, localhost-only)
- Scraper microservice handles Playwright-based scraping as fallback for SFlix/Dramacool

### Middleware Chain (in order)
1. CORS (allowlist from `ALLOWED_ORIGINS` env var; empty = allow all)
2. `express.json()` body parser
3. Custom metrics/logger (anonymizes IP, increments `metrics.requestsTotal`)
4. Route handlers (11 route files)
5. `express.static('public/')` — `/config.html` guarded by session cookie
6. 404 catch-all

**No global rate limiting on stream routes. Auth only on admin UI.**

### Route Count: 76 routes across 11 files
(apiServer.js direct: POST /auth/login, GET /config.html, GET /, /api/docs/*)

### Provider Dispatch
- Filesystem discovery: scans `providers/` for `.js` files
- Enable/disable: `user-config.json` > env var `ENABLE_<NAME>_PROVIDER` > default true
- Public name aliasing via `PROVIDER_RENAME_MAP` (23 entries)
- `sanitizeText()` scrubs real provider names from stream titles
- Fast providers: vixsrc, huerust, moviebox, autoembed, 4khdhub, moviesmod, uhdmovies

### Execution Strategy
- Fast pool: 5s collective deadline, 15s individual timeout
- Slow pool: 15s collective deadline, 60s individual timeout
- Status field: "Partial" if >14s elapsed, "Complete" otherwise
- SSE live endpoint: emits `event: provider` as each completes, 15s heartbeat

### Caching (3 layers)
1. In-flight coalescing: `pendingFetches Map` — duplicate concurrent requests share one Promise
2. Per-provider cache: 4h TTL, key `provider:type:tmdbId`
3. Aggregate cache: 4h TTL, 1000-entry max, URL-deduplication merge

### Error Handling
- Provider isolation: each fetch in `Promise.race` + `try/catch` → returns `[]`, never propagates
- Route-level: top-level `try/catch` → HTTP 500 `{ success: false, error: 'INTERNAL_ERROR' }`
- Global 404 catch-all
- No uncaught exception handler in app code — delegated to PM2

### Authentication
- PBKDF2-SHA512 (100,000 iterations, random 16-byte salt)
- Custom JWT-like tokens: `base64url(payload).HMAC-SHA256-sig`
- Session secret: `crypto.randomBytes(32)` at startup — **ephemeral, sessions die on restart**
- Default: admin / change-me (console warning on first run)

### Configuration
- Priority: `user-config.json` > `.env` > hardcoded defaults
- `saveConfigPatch()` hot-reloads without restart
- Multiple TMDB keys supported (JSON array or newline/comma/semicolon delimited)
- Config mirrored back to `process.env` for legacy provider compatibility
