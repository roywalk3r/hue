# GEMINI.md - Project Context

## Project Overview
**Name:** TMDB Embed API (`tmdb-embed-api`)
**Type:** Node.js/Express Stream Aggregation Service
**Description:** A standalone API that takes a TMDB ID (Movie/TV) and aggregates streaming sources from various "providers" (scrapers). It includes a secure admin panel, multi-key TMDB rotation, and a plugin-based provider system.

## Key Technologies
*   **Runtime:** Node.js (v18+ recommended)
*   **Web Framework:** Express.js
*   **Scraping/Automation:** Playwright (with `puppeteer-extra-plugin-stealth`), Axios, Cheerio.
*   **State/Config:** In-memory caching, JSON-based persistent config (`utils/user-config.json`).

## Architecture

### 1. API Server (`apiServer.js`)
The main entry point. It sets up:
*   **Routes:**
    *   `/api/streams/:type/:tmdbId`: Main aggregation endpoint.
    *   `/api/drama/:type/:tmdbId`: **New** dedicated endpoint for Drama providers (KissKH, Dramacool).
    *   `/sports-stream/:matchName`: **New** endpoint for live sports.
    *   `/api/config`: Configuration management.
    *   `/api/health`, `/api/metrics`: Monitoring.
    *   `/auth/*`: Session-based authentication.
*   **Middleware:** CORS, Rate limiting (login), Static file serving (`public/`).

### 2. Provider System (`providers/`)
Providers are pluggable JS modules that export a specific function signature to fetch streams.
*   **Registry (`providers/registry.js`):** Dynamically loads providers, manages the `providerFunctionMap`, and handles execution.
*   **Caching:** All provider results are now cached globally in `providers/registry.js` using `utils/cache.js` (1hr TTL).
*   **Structure:** Each provider (e.g., `providers/dramacool.js`) fetches data and returns a normalized stream object:
    ```json
    {
      "title": "Stream Title",
      "url": "https://...",
      "quality": "1080p",
      "provider": "ProviderName",
      "headers": { ... }
    }
    ```
*   **Types:**
    *   **API-based:** Uses HTTP requests (Axios) to fetch data (e.g., `moviesmod`, `vidrock`, `livesport`).
    *   **Browser-based:** Uses Playwright to render pages and extract links (e.g., `kisskh`, `sflix`, `vidsrc-embed`).
    *   **Hybrid:** Combines API for discovery and Browser for extraction (current `dramacool.js`).

### 3. Browser Manager (`utils/browserManager.js`)
Centralized management for Playwright instances.
*   **Concurrency:** Limits concurrent pages (`MAX_GLOBAL_CONCURRENT_PAGES`, default 5) to prevent resource exhaustion.
*   **Optimization:** Blocks images, fonts, and ad domains to speed up scraping.
*   **Stealth:** Uses `puppeteer-extra-plugin-stealth` to evade bot detection.

## Building and Running

### Prerequisites
*   Node.js installed.
*   Browsers installed for Playwright (handled by `setup-browser.js`).

### Commands
*   **Install Dependencies:** `npm install`
*   **Start Server:** `npm start`
    *   Runs `node setup-browser.js` first, then `node apiServer.js`.
*   **Dev Mode:** `npm run start:dev`
    *   Uses `nodemon` to watch for changes (ignores `cache` folders).
*   **Lint:** `npm run lint`

### Testing
*   **Standalone Scripts:** The project relies heavily on standalone scripts for testing specific functionality without starting the full server.
    *   `node test-vidsrc-browser.js`: Test the VidsrcEmbed provider.
    *   `node test-registry-cache.js`: Verify caching logic.

## Development Conventions
*   **Provider Isolation:** New providers should be self-contained in `providers/` and registered in `providers/registry.js`.
*   **Config Overrides:** Do not modify `utils/config.js` directly. Use the Admin UI or modify `utils/user-config.json` for persistent local overrides.
*   **Browser Usage:** Always use `getNewPage()` or `getNewContext()` from `utils/browserManager.js` instead of launching new browser instances manually. This ensures resource limits are respected.
*   **Error Handling:** Providers should catch their own errors and return an empty array `[]` rather than crashing the aggregation promise.

## Current State & Notes
*   **Drama Separation:** A new endpoint `/api/drama` has been created. `kisskh` and `dramacool` are excluded from the main `/api/streams` browser loop to improve performance.
*   **Hybrid Dramacool:** The `dramacool` provider now uses the Xyra API for search/info but falls back to a custom Playwright scraper if the API returns a server URL instead of a direct link.
*   **Caching Implemented:** `registry.js` now caches successful provider results, making repeat requests instant.
*   **Performance:** `browserManager.js` limit increased to 5 concurrent pages, with new Chromium flags for speed.