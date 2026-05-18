# Stage 4: Frontend / Public UI

**Tier:** LOW | **Status:** Complete

## Key Findings

### Pages
- `index.html` (950+ lines) — main discovery, hero carousel, 7-source tabs, search, stream modal
- `player.html` (110 lines) — Stream Lab for HLS/DASH testing with JSON API input
- `config.html` (240 lines) — enterprise admin panel
- `docs.html` (22 lines) — OpenAPI via Scalar API Reference
- `test-player.html`, `test.html` — debug fixtures

### Player Capabilities
- Formats: HLS (.m3u8), DASH (.mpd), Direct (mp4, webm, mkv, avi, mov, ogg)
- Libraries: HLS.js 1.5.13, Dash.js, native HTML5
- VidZee token decryption: AES-CBC, base64 + hardcoded 32-byte key `"qrincywincyspider"`
- Proxy wraps URLs for CORS bypass + Referer/User-Agent injection
- 3-tier fallback: proxy → direct URL → next stream
- Watchdog timeout: 15–45s based on provider
- Quality auto-sort by resolution; displays provider timings
- Download buttons with filename injection

### Main Index Features
- Fixed navbar: "HUE STREAM" branding, API Docs, Metrics
- Hero carousel: 8s autoplay, featured content with trailers/ratings/descriptions
- 7 content source tabs: HiTV, MovieBox, AiSports, TMDB, KissKH, Dramacool, Sports (LiveTV)
- Adaptive filters: genre selectors, status (Live/Upcoming), type selectors per source
- Search: source-aware submit behavior
- Results grid: 4-col (desktop) → 2-col (mobile), lazy load, infinite scroll

### Admin Panel Sections
1. Server Status — real-time metrics, provider health, functional test
2. Core Settings — port, region, provider enable/disable matrix
3. Quality & Filtering — preset radios (All/480p+/720p+/1080p+/1440p+/4K+), codec exclusion
4. API Keys — multi-TMDB key manager
5. FebBox/PStream — cookie token manager
6. Advanced — feature flags (cache, URL validation, proxy)
7. Live Config — JSON display of merged + override configs

### Authentication UI
- Rate limiting: lockout with countdown timer (mm:ss)
- Flash messages: loggedOut, expired, invalid attempts remaining
- Session verification on load → redirect if authenticated
- Password change: 8-char minimum, old password verification

### Tech Stack
- **No frameworks** — pure ES6+ JavaScript, IIFE encapsulation
- Bootstrap 5.3.0, HLS.js 1.5.13, Dash.js, Scalar API Reference
- Google Fonts: Space Grotesk (player), DM Mono (monospace)
- No bundler, no build step

### Design System
- Primary bg: `#0c101a` / secondary: `#1a2235`
- Text: `#e7edf9` (primary), `#91a0bb` (muted)
- Accent: `#0d6efd` (blue), `#31d7a9` (teal), `#7c5dff` (purple, player only)
- Glassmorphism: `backdrop-filter: blur(12px)`
- Transitions: `0.18s cubic-bezier(.4,0,.2,1)`
- Shadows: `0 15px 60px rgba(0,0,0,0.45)`
- Responsive breakpoints: 880px (hamburger), 960px (grid collapse), 560px (single-col)
