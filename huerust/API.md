# API Reference

Base URL: `http://127.0.0.1:24609`

## Stream Aggregation

### `GET /api/streams/:type/:tmdbId`

Fetch all provider streams for a TMDB title.

**Path params:**
- `type` — `movie` or `tv`
- `tmdbId` — numeric TMDB ID

**Query params:**
- `s` — season number (default: 1)
- `e` — episode number (default: 1)

**Response:** `Array<StreamResult>`

```json
[
  {
    "url": "http://localhost:24609/m3u8-proxy?h_token=...",
    "title": "Cinema - 1080p",
    "quality": "1080p",
    "provider": "Cinema",
    "format": "hls",
    "subtitles": [
      { "url": "...", "lang": "en", "label": "English" }
    ],
    "headers": { "Referer": "..." }
  }
]
```

### `GET /api/streams/:type/:tmdbId/live`

Same as above but streamed as Server-Sent Events.

**SSE events:**
- `provider` — provider name that just completed
- `cached` — cached results were returned
- `done` — all providers finished
- `error` — provider error message

### `GET /api/play/:type/:tmdbId`

Smart Play — validates streams with HEAD requests and returns the first working one.

**Response:** `StreamResult` (single best stream)

---

## Drama

### `GET /api/drama/detail`

Unified drama detail. Tries KissKH first, falls back to MovieBox.

**Query params:**
- `q` — title to search
- `id` — optional KissKH show ID (numeric). If provided, skips search and fetches directly.

**Response:**
```json
{
  "success": true,
  "provider": "kisskh",
  "id": "11923",
  "title": "Perfect Crown",
  "cover": "https://image.tmdb.org/t/p/w500/...",
  "description": "...",
  "year": 2025,
  "country": "South Korea",
  "status": "Ongoing",
  "genre": "",
  "episodes": [{ "id": "207851", "number": 1, "title": "Episode 1" }],
  "episode_count": 12
}
```

### `GET /api/drama/kisskh/search?q=...`

### `GET /api/drama/kisskh/show`

### `GET /api/drama/kisskh/browse?page=1&page_size=10&...`

### `GET /api/drama/kisskh/upcoming`

### `GET /api/drama/kisskh/top-rating`

### `GET /api/drama/kisskh/streams/:id/:episode`

### `GET /api/drama/dramacool/search?q=...`

### `GET /api/drama/dramacool/streams/:id/:episode`

### `GET /api/drama/moviebox/browse?page=1&per_page=10`

### `GET /api/drama/moviebox/search?q=...`

### `GET /api/drama/moviebox/streams/:title/:episode`

---

## HiTV

### `GET /api/hitv/library`

Full cached drama library.

### `GET /api/hitv/browse?country=...&sort=...&genre=...&page=...`

### `GET /api/hitv/search?q=...`

### `GET /api/hitv/latest`

### `GET /api/hitv/trending`

### `GET /api/hitv/hero`

### `GET /api/hitv/drama`

### `GET /api/hitv/info/:id`

Show detail by HiTV internal ID.

### `GET /api/hitv/play/:id/:episode`

Resolves HiTV internal ID → TMDB → redirects to Smart Play.

### `GET /api/hitv/countries`

### `GET /api/hitv/categories`

### `GET /api/hitv/genres`

### `GET /api/hitv/sorts`

### `GET /api/hitv/:country/:sort/:genre`

Dynamic browse path.

---

## MovieBox

### `GET /api/moviebox/home`

### `GET /api/moviebox/discover`

### `GET /api/moviebox/section`

### `GET /api/moviebox/trending`

### `GET /api/moviebox/popular-movie`

### `GET /api/moviebox/popular-series`

### `GET /api/moviebox/hot-shorts`

### `GET /api/moviebox/sa`

### `GET /api/moviebox/thai`

### `GET /api/moviebox/turkish`

### `GET /api/moviebox/western`

### `GET /api/moviebox/animation`

### `GET /api/moviebox/black-shows`

### `GET /api/moviebox/bollywood`

### `GET /api/moviebox/shorts`

### `GET /api/moviebox/hero`

### `GET /api/moviebox/filter?type=...&genre=...&year=...&page=...`

### `GET /api/moviebox/info?id=...`

### `GET /api/moviebox/search?q=...&page=...`

---

## Anime

### `GET /api/anime/search?q=...`

### `GET /api/anime/latest`

### `GET /api/anime/category?type=...`

### `GET /api/anime/home`

### `GET /api/anime/detail/:slug`

### `GET /api/anime/episodes/:id`

### `GET /api/anime/servers?id=...&episode=...`

### `GET /api/anime/stream?id=...&type=...&episode=...&server=...`

**Response:** `StreamPayload`

```json
{
  "stream": {
    "url": "...",
    "format": "hls",
    "quality": "1080p",
    "subtitles": [...],
    "tracks": [...],
    "intro": { "start": 0, "end": 85 },
    "outro": { "start": 1320, "end": 1420 }
  }
}
```

---

## System

### `GET /health`

Returns `"ok"`.

### `GET /metrics`

Prometheus-compatible metrics.

### `GET /api/status`

```json
{
  "version": "...",
  "uptime_seconds": 12345,
  "providers": [
    { "name": "Cinema", "real_name": "moviebox", "healthy": true },
    ...
  ]
}
```

### `GET /api/providers`

Lists all registered providers with circuit breaker state.

### `POST /api/admin/reload`

Invalidates user config cache.

### `POST /api/admin/refresh-tokens`

Triggers Node.js token refresh.

---

## Proxy Routes

### `GET /m3u8-proxy?h_token=...` or `?url=...&headers=...`

Proxies HLS playlists. Rewrites segment URLs to proxied tokens.

### `GET /ts-proxy?h_token=...`

Proxies TS/video segments. Supports HTTP Range headers.

### `GET /sub-proxy?h_token=...`

Proxies subtitles. Auto-converts SRT → WebVTT.

---

## Models

### `StreamResult`

| Field | Type | Description |
|-------|------|-------------|
| `url` | string | Proxied stream URL |
| `title` | string | Human-readable label |
| `quality` | string | Resolution or "Auto" |
| `provider` | string | Display name (Cinema, Nova, Zenith, ...) |
| `format` | string | `hls`, `mp4`, `mkv`, `video` |
| `subtitles` | Subtitle[] | Optional subtitle tracks |
| `headers` | Record<string, string> | Required request headers |
| `extra_metadata` | Record<string, string> | Provider-specific hints |

### `Subtitle`

| Field | Type |
|-------|------|
| `url` | string |
| `lang` | string |
| `label` | string |
