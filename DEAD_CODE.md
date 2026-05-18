# Node.js Dead Code Audit

## Deleted Providers (superseded by Rust)

| File | Reason | Deleted |
|------|--------|---------|
| `providers/autoembed.js` | Replaced by Rust Vixsrc (Aura). Task #25. | 2026-05-17 |
| `providers/9anime.js` | Explicitly disabled in config. | 2026-05-17 |
| `providers/4khdhub.js` | Explicitly disabled in config. | 2026-05-17 |
| `providers/uhdmovies.js` | Explicitly disabled in config. | 2026-05-17 |
| `providers/moviesmod.js` | Explicitly disabled in config. | 2026-05-17 |
| `providers/Showbox.js` | Excluded from registry. Dead code. | 2026-05-17 |
| `providers/showbox-playwright.js` | Excluded from registry. Dead code. | 2026-05-17 |
| `providers/VidZee.js` | Excluded from registry. Rust has VidZee. | 2026-05-17 |
| `providers/vidsrccc.js.disabled` | Already disabled by filename. | 2026-05-17 |
| `providers/vidsrcwtf.js.disabled` | Already disabled by filename. | 2026-05-17 |

## Deleted Services / Utilities

| File | Reason | Deleted |
|------|--------|---------|
| `services/hianimeApp.js` | Never used in `apiServer.js`. | 2026-05-17 |
| `daemon/daemon.js` | Port conflicts with scraper server (24610). Not in PM2 config. | 2026-05-17 |
| `m3u8-proxy-v2.js` | Superseded by main proxy implementations. | 2026-05-17 |
| `sports-proxy-server.js` | Unused. Sports proxy handled by main proxy. | 2026-05-17 |
| `cloudflare-worker-proxy.js` | Deployment artifact, not runtime code. | 2026-05-17 |

## Deleted Root Artifacts

| File | Reason | Deleted |
|------|--------|---------|
| `hitv.js` | Scraped dump (581KB). | 2026-05-17 |
| `hitv_library_cache.json` | Runtime cache artifact. | 2026-05-17 |
| `hitv_mapping_cache.json` | Runtime cache artifact. | 2026-05-17 |
| `hitv_taxonomy.json` | Runtime cache artifact. | 2026-05-17 |
| `movibox-home.json` | Scraped dump. | 2026-05-17 |
| `sflix-detail.json` | Scraped dump. | 2026-05-17 |
| `sflix-search-page.log` | Debug log. | 2026-05-17 |
| `res.json` | Generic response dump. | 2026-05-17 |
| `result.txt` | Generic output dump. | 2026-05-17 |
| `extracted-m3u8.json` | Scraped dump. | 2026-05-17 |
| `live-matches-with-m3u8.json` | Scraped dump. | 2026-05-17 |
| `ppv-world-darts-championship-streams-*.json` | Scraped dump. | 2026-05-17 |
| `philadelphia-76ers-vs-milwaukee-bucks-streams-*.json` | Scraped dump. | 2026-05-17 |
| `match-name-m3u8-*.txt` | Scraped dump. | 2026-05-17 |
| `match-name-streams-*.json` | Scraped dump. | 2026-05-17 |
| `hianimes-page.html` | Scraped HTML. | 2026-05-17 |
| `hianimes-ep1.png` | Screenshot. | 2026-05-17 |
| `hianimes-ep1-iframe.png` | Screenshot. | 2026-05-17 |
| `megaplay-iframe.html` | Scraped HTML. | 2026-05-17 |
| `debug_hamburg.html` | Debug HTML. | 2026-05-17 |
| `debug-hianimes-scrape.log` | Debug log. | 2026-05-17 |
| `app.log` / `server.log` / `cookies.txt` / `kisskhplan.log` | Runtime logs. | 2026-05-17 |
| `screenshots/` | Old UI screenshots. | 2026-05-17 |

## Archived Scripts

~70 root-level debug/test/crack scripts moved to `archive/debug-scripts/`:

- `test-*.js` — Provider test scripts
- `debug-*.js` — Debug scrapers
- `capture-*.js` — Screenshot capture scripts
- `check-*.js` — Health checks
- `find-*.js` — Search scripts
- `get-*.js` — Fetch scripts
- `extract-*.js` — Extraction scripts
- `crack_kkey*.js` — KissKH kkey generation debug scripts (now ported to Rust)

## Still To Evaluate

These files are still mounted in `apiServer.js` but may be redundant:

| File | Evaluation |
|------|------------|
| `routes/anime.js` | Does any client still hit `/api/anime` on Node? Frontend uses Rust. |
| `routes/proxy.js` (`/api/proxy`) | Does any client still use this simple Axios proxy? |
| `proxy/proxyServer.js` root mounts | Is all proxy traffic going to Rust now? |
| `routes/moviebox.js` | Does frontend hit Rust MovieBox routes directly? |

## Safe To Keep

| File | Reason |
|------|--------|
| `routes/sports.js` | No Rust equivalent. Node-only. |
| `routes/livetv.js` | No Rust equivalent. Node-only. |
| `routes/tmdb.js` | Frontend still hits some TMDB routes through Node. |
| `routes/system.js` | Config, cache clear, health. |
| `utils/auth.js` | Login/password auth. No Rust equivalent. |
| `scraperServer.js` | Playwright fallback for SFlix/Dramacool. |
| `services/livetvScraper.js` | LiveTV scraping. |
| `services/tokenWorker.js` | Cookie refresh for MovieBox/SFlix. |
| `services/warmupWorker.js` | Pre-scrapes popular content. |
| `utils/playwrightResolver.js`, `browserManager.js`, `browserConfig.js`, `interceptor.js` | Used by scraper server. |
