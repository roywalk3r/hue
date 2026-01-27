# Hue Stream API 🎥

![License](https://img.shields.io/badge/license-MIT-blue.svg) ![Node](https://img.shields.io/badge/node-v18%2B-green.svg) ![Status](https://img.shields.io/badge/status-stable-success.svg)

> **The Ultimate Streaming Aggregator** for Movies, TV Shows, KDramas, and Live Sports.
> Built for performance, stability, and ease of use.

---

## 🌟 Overview

**Hue Stream** is a unified backend that turns disparate streaming sources into a single, polished API. It combines the speed of HTTP-based providers with the resilience of headless browser automation to deliver content reliably.

Whether you are building a **Netflix clone**, a **KDrama hub**, a **sports streaming platform**, or a personal media center, Hue provides the streams, metadata, and subtitles you need—securely and instantly.

---

## ✨ Key Features

### 🎬 **Smart Aggregation**
Fetch streams from multiple high-quality providers simultaneously. The system automatically races them and returns the best results first.
*   **Western Content:** `vixsrc`, `vidrock`, `sflix`
*   **Asian Drama:** `kisskh` (Zenith), `dramacool` (Orient)
*   **Live Sports:** `livetv` (Arena), `livesport` (Stadium)

### 🌏 **HiTV Discovery Engine**
A dedicated subsystem for Asian content discovery.
*   **Browse:** Explore thousands of KDramas, CDramas, and Movies by Genre, Country, and Popularity.
*   **Smart Play:** Automatically maps a HiTV ID to a playable stream from Zenith or Nova.
*   **Metadata:** Rich details including ratings, cast, and release dates.

### 🛡️ **Fortress Security**
Enterprise-grade protection for your streams.
*   **Signed URLs:** All proxy links are encrypted with **AES-256 HMAC**. They are time-limited (4h) and tamper-proof.
*   **Origin Locking:** Restrict API access to *your* frontend domains only.
*   **Anti-Leech:** Prevents unauthorized bandwidth usage and open relay abuse.

### 🚀 **Performance & Stability**
*   **Hybrid Scrapers:** Uses lightweight HTTP requests where possible, falling back to optimized Headless Browsers (Playwright) only when necessary.
*   **Ad-Blocking:** Built-in interceptors block ads and trackers at the network level, speeding up stream extraction by **300%**.
*   **Resilient Proxy:** Automatic **IPv4 Failover** ensures streams play even on strict networks or IPv6-hostile CDNs.

---

## 📚 API Documentation

We offer beautiful, interactive documentation to help you integrate quickly.

*   **Scalar Docs (Modern):** [`/api/docs`](http://localhost:24608/api/docs) - *Recommended*
*   **Swagger UI (Classic):** [`/api/docs/swagger`](http://localhost:24608/api/docs/swagger)
*   **OpenAPI Spec:** [`/api/docs/swagger.json`](http://localhost:24608/api/docs/swagger.json)

---

## 🛠️ Installation & Setup

### Prerequisites
*   Node.js v18+
*   Chrome / Chromium (for Playwright)

### Quick Start

1.  **Clone the repository**
    ```bash
    git clone https://github.com/roywalk3r/hue.git
    cd hue
    ```

2.  **Install dependencies**
    ```bash
    npm install
    # or
    pnpm install
    ```

3.  **Setup the Environment**
    The system will automatically generate a secure `APP_SECRET` on the first run.
    Optionally, create a `.env` file to customize:
    ```bash
    # .env
    PORT=24608
    TMDB_API_KEY=your_tmdb_key_here
    ALLOWED_ORIGINS=https://your-frontend.com,http://localhost:3000
    STRICT_SECURITY=true
    ```

4.  **Launch!**
    ```bash
    npm start
    ```
    Access the UI at `http://localhost:24608`.

---

## 📡 API Endpoints Guide

### 1. Western Movies & TV (TMDB)
Aggregate streams for mainstream content.
```http
GET /api/streams/movie/{tmdbId}
GET /api/streams/series/{tmdbId}?season=1&episode=1
```

### 2. KDrama Discovery (HiTV)
Browse and search the Asian content library.
```http
GET /api/hitv/browse?sort=Hottest&genre=Romance
GET /api/hitv/search?q=Squid Game
GET /api/hitv/info/{id}
```

### 3. Smart Play (KDrama)
The magic endpoint that plays HiTV content.
```http
GET /api/hitv/play/{id}/{episode}?title=Show Name
```

### 4. Live Sports Streaming
Watch live sports from multiple sources.
```http
# LiveTV.sx (Arena)
GET /api/livetv/sports              # Get sports categories
GET /api/livetv/matches?sportId=    # Get matches by sport
GET /api/livetv/match?url=          # Get stream variants
GET /livetv/{matchName}             # Direct match access

# Livesport.su (Stadium)
GET /sports-stream/{matchName}      # Direct match access
```

### 5. Direct Provider Access
Target specific providers for granular control.
```http
GET /api/kdrama/search?q=...
GET /api/dramacool/search?q=...
GET /api/streams/sflix/movie/{id}
```

---

## 🔒 Security Configuration

The API automatically secures itself.

*   **Key Rotation:** Run `npm run key:generate` to rotate your encryption secret.
*   **CORS:** Set `ALLOWED_ORIGINS` in `.env` to comma-separated domains to lock down access.
*   **Strict Mode:** Set `STRICT_SECURITY=true` to reject any request without a valid signature token (recommended for production).

---

## 🤝 Contributing

Contributions are welcome! Please check the `providers/` directory to see how to add new sources.

1.  Fork the Project
2.  Create your Feature Branch (`git checkout -b feature/AmazingFeature`)
3.  Commit your Changes (`git commit -m 'Add some AmazingFeature'`)
4.  Push to the Branch (`git push origin feature/AmazingFeature`)
5.  Open a Pull Request

---

## 📄 License

Distributed under the MIT License. See `LICENSE` for more information.

---

<div align="center">
  <sub>Built with ❤️ by roywalk3r</sub>
</div>