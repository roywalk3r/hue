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
*   **General Content:** Nova (SFlix), Aura (Vixsrc), Luna (Vidrock), Solar (Vidsrc-xyz).
*   **Asian Drama:** Zenith (KissKH), Orient (Dramacool), Cinema (MovieBox).
*   **Live Sports:** Stadium (Livesport), Field (AiSports), Arena (LiveTV), Event (PPV.to).

### 🌏 **Regional Discovery Engine**
A dedicated subsystem for global content discovery.
*   **HiTV & SFlix:** Explore thousands of KDramas and CDramas by Genre, Country, and Popularity.
*   **MovieBox:** Full catalog of Western and Asian content with high-quality MP4/HLS streams.
*   **Smart Play:** Automatically maps discovery IDs to playable streams from Zenith, Nova, or Cinema.
*   **Metadata:** Rich details including ratings, cast, and trailers.

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

### 1. Unified Aggregation (TMDB)
Aggregate streams for mainstream content.
```http
GET /api/streams/movie/{tmdbId}
GET /api/streams/series/{tmdbId}?season=1&episode=1
```

### 2. Anime (HiAnime)
Discover servers and fetch streams for anime episodes.
```http
GET /api/streams/anime/{id}
GET /api/streams/anime/{id}?server=hd-1&type=sub
GET /api/streams/anime/{id}?serversOnly=true
```

### 3. Regional Discovery (HiTV / MovieBox)
Browse and search the global content library.
```http
# HiTV
GET /api/hitv/browse?sort=Hottest&genre=Romance
GET /api/hitv/search?q=Squid Game
GET /api/hitv/play/{id}/{episode}

# MovieBox
GET /api/moviebox/hero               # Get trending/hero items
GET /api/moviebox/search?keyword=...  # Search catalog
GET /api/moviebox/play/{id}/{path}    # Extract direct streams
```

### 4. Live Sports Streaming
Watch live sports from multiple sources.
```http
# Smart Resolver (Multi-Source)
GET /sports-stream/{matchName}

# PPV.to (Event)
GET /api/ppv/streams/{matchName}

# AiSports (Field)
GET /api/aisports/list/0            # List live matches
GET /api/aisports/play/{id}         # Get direct streams
```

### 5. System & Metrics
Monitor and configure your instance.
```http
GET /api/status                     # Stats, providers, and endpoints
GET /api/config                     # Get current configuration
POST /api/config                    # Update configuration patch
```

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
