# HUE STREAM API ⬢
### System Designation: The Architecture of Aggregation
**Version:** 1.8.4 | **Status:** Nominal | **Engine:** Node.js v18+

> [!IMPORTANT]
> **Manual-First Backend Engine:** Hue is a high-performance streaming aggregator. It is engineered for modularity, resilience, and schema-strict data delivery. This is not a media player; it is the infrastructure that powers them.

---

## ⬢ System Specifications

### 01. Modular Provider Registry
The Hue core operates as a central hub for independent provider nodes.
*   **Nova (SFlix):** High-speed spectral aggregation.
*   **Zenith (KissKH):** Focused regional drama telemetry.
*   **Stadium (Livesport):** Real-time sports data streams.
*   **Nova/Cinema/Luna:** Primary high-fidelity content sources.

### 02. Secure Blueprinting (Proxy)
All outbound telemetry is encapsulated via an AES-256 HMAC signed proxy system.
*   **Telemetry Locking:** Origin-restricted bandwidth management.
*   **IPv4 Failover:** Automatic route switching for legacy CDNs.

### 03. Discovery Schematic
A multi-layered catalog engine mapping global content (HiTV/MovieBox) to playable provider nodes.

---

## ⬢ Documentation Interface

The system exposes technical documentation for rapid integration and schema validation.

*   **Scalar Docs (Modern):** [`/api/docs`](http://localhost:24608/api/docs) - *Recommended*
*   **Swagger UI (Classic):** [`/api/docs/swagger`](http://localhost:24608/api/docs/swagger)
*   **OpenAPI Spec:** [`/api/docs/swagger.json`](http://localhost:24608/api/docs/swagger.json)

---

## ⬢ Deployment Protocol

### Prerequisites
*   Node.js v18+
*   Chrome / Chromium (Engineered for Playwright)

### Initial Initialization

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

4.  **Execute Start Sequence**
    ```bash
    npm start
    ```
    Access the UI at `http://localhost:24608`.

---

## ⬢ Telemetry Endpoints

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

## ⬢ System Contribution

Technical contributions are encouraged. Refer to the `providers/` directory for node implementation details.

1.  Fork the Project
2.  Create your Feature Branch (`git checkout -b feature/AmazingFeature`)
3.  Commit your Changes (`git commit -m 'Add some AmazingFeature'`)
4.  Push to the Branch (`git push origin feature/AmazingFeature`)
5.  Open a Pull Request

---

## ⬢ Licensing

Distributed under the MIT License. See `LICENSE` for more information.

---

<div align="center">
  <sub>Engineered by roywalk3r</sub>
</div>
