# Hue Stream API 🎬

> **The Advanced, High-Performance Streaming Aggregation Engine.**

![License](https://img.shields.io/badge/license-MIT-blue.svg) ![Node](https://img.shields.io/badge/node-v18%2B-green.svg) ![Status](https://img.shields.io/badge/status-active-success.svg)

**Hue Stream API** is a robust, production-ready REST API that transforms a simple **TMDB ID** into a list of playable, high-quality video streams. It powers your streaming frontend by aggregating content from over 10+ sources, managing browser automation, and bypassing playback restrictions.

---

## ✨ Features

### 🚀 High Performance
*   **Smart Caching**: Results are cached globally (TTL 1h). Repeat requests are **instant (0ms)**.
*   **Optimized Concurrency**: Managing up to **5 simultaneous browser instances** (configurable) for maximum throughput without crashing your server.
*   **Parallel Execution**: "Fast" API providers run immediately, while "Slow" browser providers process in the background.

### 🔌 Powerful Provider System
*   **Diverse Sources**: Aggregates from APIs, direct scrapes, and protected sites (via Cloudflare Turnstile solvers).
*   **Dedicated Drama Hub**: Specialized logic (`/api/drama/...`) for K-Drama/Asian content (KissKH, Dramacool).
*   **Live Sports**: Real-time extraction for sports events (`/sports-stream/...`).

### 🛡️ Smart Proxying
*   **CORS Bypass**: Built-in proxy rewrites `.m3u8` playlists and keys to ensure content plays on *any* web player (Video.js, JWPlayer, etc.).
*   **Header Spoofing**: Automatically injects `Referer` and `User-Agent` headers for protected streams.

### 🔧 Developer Ready
*   **Admin Panel**: Web-based UI (`/config.html`) to toggle providers, rotate keys, and view metrics.
*   **Docker Ready**: simple deployment with Docker.
*   **OpenAPI/Swagger**: Full API documentation available at `/api/docs`.

---

## 📖 Documentation
Interactive API documentation is available via **Scalar** (Modern) and **Swagger UI** (Classic):
- **Scalar Docs**: `http://localhost:24608/api/docs` (Recommended)
- **Swagger UI**: `http://localhost:24608/api/docs/swagger`
- **Raw Spec**: `http://localhost:24608/api/docs/swagger.json`

## 🌟 New Features
- **KDrama Discovery**: Full browsing, filtering (Country, Genre), and sorting (Hottest, Newest) powered by HiTV.
- **Smart Play**: Intelligent mapping from HiTV IDs to KissKH (Zenith) and SFlix (Nova) streams.
- **Networking Failover**: Automatic IPv4 failover for problematic CDNs.
- **Subtitle Proxy**: CORS-bypass proxy for external subtitle files.

## 🚀 Getting Started

### 1. Installation
```bash
git clone <repo_url>
cd hue-stream-api
npm install
```

### 2. Setup Browsers (Important!)
This project uses Playwright for advanced scraping.
```bash
npx playwright install chromium firefox
```

### 3. Run
```bash
# Production
npm start

# Development (Auto-restart)
npm run start:dev
```
*   **API**: `http://localhost:24608/api`
*   **UI**: `http://localhost:24608/`

---

## 📡 Key Endpoints

| Method | Endpoint | Description |
| :--- | :--- | :--- |
| `GET` | `/api/streams/movie/:id` | Get streams for a movie (TMDB ID). |
| `GET` | `/api/streams/series/:id` | Get streams for a TV show (Requires `?season=X&episode=Y`). |
| `GET` | `/api/drama/:type/:id` | Get Asian Drama streams (optimized separately). |
| `GET` | `/sports-stream/:matchName` | **New!** Get live sports streams by match name. |
| `GET` | `/api/status` | View system health, cache stats, and provider status. |

---

## 🧩 Provider Status

| Provider | Type | Speed | Description |
| :--- | :--- | :--- | :--- |
| **Vixsrc** | API | ⚡ Fast | Multi-source aggregator. |
| **VidRock** | API | ⚡ Fast | Reliable high-speed source. |
| **VidsrcEmbed** | Browser | 🐢 Slow | **New!** Solves Cloudflare Turnstile to extract reliable links. |
| **SFlix** | Browser | 🐢 Slow | High-quality links, requires browser automation. |
| **KissKH** | Browser | 🐢 Slow | **Drama Only**. Massive library. |
| **Dramacool** | Hybrid | ⚡ Mixed | **Drama Only**. API Search + Browser Extraction. |
| **Livesport** | API | ⚡ Fast | Real-time sports events. |

---

## ⚙️ Configuration

You can configure the server via environment variables or the built-in **Admin UI**.

### Environment Variables (.env)
```env
PORT=24608
TMDB_KEY=your_tmdb_key_here
MAX_CONCURRENT_PAGES=5  # Increase for higher scraping throughput
BROWSER_HEADLESS=true   # Set to false to see the browsers working
```

### Admin UI
Visit `http://localhost:24608/config.html` to:
*   Enable/Disable specific providers.
*   Set default regions.
*   Manage proxy settings.

---

## 🛠️ Performance Tuning

### Caching
The system now implements **provider-level caching**. If a user requests "Fight Club", the result is saved for 1 hour. The next user who requests it gets the result in **0 milliseconds**, bypassing all external requests.

### Browser Concurrency
To speed up scraping, the `browserManager` is set to handle **5 concurrent pages** by default.
*   **Low Spec Server?** Set `MAX_CONCURRENT_PAGES=2` in your environment.
*   **High Spec Server?** Increase it to `10+` for faster bulk processing.

---

## 🤝 Contributing

1.  Create a new file in `providers/` (e.g., `my-source.js`).
2.  Implement a `getStreams(tmdbId, type, ...)` function.
3.  Register it in `providers/registry.js`.
4.  Submit a Pull Request!

---

## 📄 License
MIT License.
