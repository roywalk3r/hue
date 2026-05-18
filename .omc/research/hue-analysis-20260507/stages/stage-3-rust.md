# Stage 3: Rust Component (huerust)

**Tier:** MEDIUM | **Status:** Complete

## Key Findings

### Architecture
- Standalone async HTTP server (Axum + Tokio), NOT a library or sidecar
- Binds on configurable `RUST_HOST:RUST_PORT` (default 24609 in Docker)
- Full application lifecycle with graceful shutdown on SIGTERM/SIGINT
- Not spawned in-process by Node.js

### 6 Registered Providers (StreamProvider trait)
| Struct | Display Name | Source |
|---|---|---|
| MovieBoxProvider | Cinema | moviebox.ph / aoneroom.com API |
| SflixProvider | Nova | sflix.film (same aoneroom API) |
| KissKhProvider | Zenith | kisskh.ovh API |
| DramacoolProvider | Orient | api.xyra.stream |
| VixsrcProvider | Aura | vixsrc.to (HTML regex) |
| Mp4HydraProvider | Hydra | mp4hydra API |

VixSrc is most fragile — pure regex on HTML, breaks if site's JS bundle changes variable names.

### HiTV (Separate from Provider Registry)
- `HiTvClient` with browse, search, detail, library building
- Background task: `tokio::spawn` loop every 43,200s (12 hours)
- Scans 4 countries × 3 categories × 5 pages → deduplicated in `RwLock<Vec<HiTvItem>>`
- Shares upstream API base (`h5-api.aoneroom.com`) with MovieBox and SFlix

### Security Module (security.rs)
- Key derivation: `scrypt(secret, b"salt", N=2^14, r=8, p=1)` → 32-byte key
- **Static salt `b"salt"` — security weakness**
- Encryption: AES-256-CBC + PKCS7, random 16-byte IV per token
- Token format: `hex(iv):hex(ciphertext)`
- Payload: `{"u": url, "e": expiry_ms, "h": optional_headers}`
- Expiry: `now + 4 hours`; `decrypt_url()` returns `None` if expired

### Authentication Middleware
- API key: `x-api-key` header vs `API_KEY` env var; loopback IPs bypass unconditionally
- Rate limiting: `DashMap<String, (count, Instant)>`, 120 req/60s window per IP
- Returns 403 or 429 respectively

### 26 API Routes
- Health/Ops: `/health`, `/metrics`
- Proxy: `/m3u8-proxy`, `/ts-proxy`, `/sub-proxy`
- HiTV: 12 routes under `/api/hitv/`
- Universal streams: `/api/streams/:type/:tmdbId`, `/api/streams/:type/:tmdbId/live` (SSE)
- Providers list, admin reload, play shortcut
- Drama-specific: kisskh and dramacool search/streams
- OpenAPI/Swagger

### Node.js Bridge
SFlix and Dramacool fall back to Node scraper when direct API returns empty:
- `POST {MICROSERVICE_URL}/scrape/sflix` with `{url, isMovie, season, episode}`
- `POST {MICROSERVICE_URL}/scrape/dramacool` with `{url}`
Response format: `{success: bool, streams: Vec<StreamResult>}`

### Hardcoded JWTs (EXPIRED)
- **SFlix default JWT**: expired ~April 10, 2026 ❌
- **MovieBox default JWT**: expired ~May 16, 2026 ❌
Both checked against `HueUserConfig::get_live()` first — user-config override is the fix path.

### Infrastructure Modules (not directly analyzed)
- `CircuitBreaker`, `StreamCache` (moka + Redis), `FetchCoalescer`, `Metrics`
- Existence confirmed via `AppState` fields and `mod` declarations
