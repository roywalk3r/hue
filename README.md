# Hue Streaming Platform

A multi-backend streaming aggregator powering the Hue UI. The platform consists of a **Rust backend** (primary engine) and a **Node.js backend** (legacy fallback for auth, live TV, and sports).

## Repositories

- **Rust Engine** (`huerust/`) — Stream aggregation, proxying, drama/anime/movie browse. See `huerust/README.md`.
- **Node.js Backend** (`/`) — Auth, live TV, sports, scraper fallback. Documented below.
- **Nuxt 4 Frontend** (`hue-ui/`) — SPA streaming UI. See `hue-ui/README.md`.

## Node.js Backend

### What it still handles

| Domain | Routes | Status |
|--------|--------|--------|
| Auth | `/auth/login`, `/auth/password` | Active |
| Live TV | `/api/livetv/*` | Active |
| Sports | `/api/sports/*` | Active |
| TMDB Proxy | `/api/tmdb/*` | Active (partial) |
| Scraper | `/scrape/*` (port 24610) | Active (SFlix/Dramacool fallback) |

### What has moved to Rust

| Domain | Rust Routes | Status |
|--------|-------------|--------|
| Stream aggregation | `/api/streams/*`, `/api/play/*` | **Rust** |
| Proxy | `/m3u8-proxy`, `/ts-proxy`, `/sub-proxy` | **Rust** |
| Drama | `/api/drama/*`, `/api/hitv/*` | **Rust** |
| Anime | `/api/anime/*` | **Rust** |
| MovieBox | `/api/moviebox/*` | **Rust** |

### Prerequisites

- Node.js v18+
- Chrome/Chromium (for Playwright scraper)

### Start

```bash
npm install
npm start
```

Node.js runs on port `24608` by default. The Rust backend runs on `24609`.

### Environment Variables

```bash
PORT=24608
TMDB_API_KEY=your_key
APP_SECRET=your_secret
ALLOWED_ORIGINS=http://localhost:3000
```

### PM2 (Production)

```bash
pm2 start ecosystem.config.js
```

Runs Node.js API + Rust backend + scraper server together.

## Documentation

- **Rust API:** `huerust/API.md`
- **Rust Architecture:** `huerust/ARCHITECTURE.md`
- **Rust Providers:** `huerust/PROVIDERS.md`
- **Node.js Dead Code Audit:** `DEAD_CODE.md`
- **Frontend Docs:** `hue-ui/docs/`

## License

MIT
