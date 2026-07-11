# HUE ENGINE ❄
### System Designation: The Architecture of Aggregation

# HiAnime Stream Extraction Protocol

Reverse-engineered from HiAnime's Next.js 15 app (May 2026).

## Architecture

HiAnime uses a three-tier architecture:

| Layer | Endpoint | Purpose | Status |
|-------|----------|---------|--------|
| Metadata | `animedata.cfd/api` | Anime info, episode lists, server links | Active |
| CDN domains | `megaplay.buzz` / `animeplay.video` | Stream source extraction | Active |
| Delivery | `mewstream.buzz` / `cinewave2.site` | Final HLS `.m3u8` manifests | Active |

The previous backend `myani.cfd` is **offline** (returns 404). The current backend is `animedata.cfd`.

## Layer 1: Anime Discovery

### Search

```
POST https://animedata.cfd/api/search
Content-Type: application/json

{ "title": "one piece" }
```

Returns an array of anime objects. Each object contains:
- `slugs[]` — episode slugs like `one-piece-gahocg`, `one-piece-8jkhmb`
- `public_id[]` — short random IDs
- `mal_id` — MyAnimeList ID
- `totalEpisodes` — total episode count
- `_id` — MongoDB hex ID (24 chars), needed for episode list lookup

### Anime Info

```
GET https://animedata.cfd/api/anime/{slug}
```

Returns full anime details. The response contains `anime.episodes[]` but **only the last episode** is included in this endpoint. For the full list, use the episodes endpoint.

## Layer 2: Episode Resolution

### Episode List (Paginated)

```
GET https://animedata.cfd/api/episodes/{animeHexId}
```

Returns episodes in batches. Default batch is episodes **1–100**. To fetch a specific episode, use the `start` query parameter set to the episode number:

```
GET https://animedata.cfd/api/episodes/{animeHexId}?start={episodeNumber}
```

**Pagination behavior:**
- `?start=1` → episodes 1–100
- `?start=50` → episodes 50–100
- `?start=99` → episodes 99–100
- `?start=101` → episodes 101–1160 (all remaining)
- `?start=1119` → episodes 1119–1160

Each episode object contains:
- `_id` — episode hex ID
- `episodeNumber` — numeric episode
- `slugs[]` — array of URL-safe slugs like `one-piece-episode-1119-qo89wh`
- `link.sub[]` / `link.dub[]` — server embed URLs

### Server Links

```
GET https://animedata.cfd/api/episode/{episodeSlug}
```

Returns the episode object with `link.sub[]` and `link.dub[]` arrays. Example response:

```json
{
  "episode": {
    "id": "68e7802441a45d67b638860c",
    "link": {
      "sub": ["https://megaplay.buzz/stream/s-2/128113/sub"],
      "dub": ["https://megaplay.buzz/stream/s-2/128113/dub"]
    },
    "public_id": ["jvrjry", "y07qup", "qo89wh", ...]
  }
}
```

**Server ID extraction:** Parse the numeric ID from `/s-2/{id}/` in the link URL.

## Layer 3: Stream Extraction

### Megaplay API

```
GET https://megaplay.buzz/stream/getSources?id={serverId}
X-Requested-With: XMLHttpRequest
Referer: https://megaplay.buzz/
```

**Required headers:**
- `X-Requested-With: XMLHttpRequest` — mandatory, returns `403 Forbidden` without it
- `Referer: https://megaplay.buzz/` — required by CDN

Returns:

```json
{
  "sources": {
    "file": "https://cdn.mewstream.buzz/anime/.../master.m3u8"
  },
  "tracks": [
    {
      "file": "https://1oe.lostproject.club/anime/.../subtitles/eng-2.vtt",
      "label": "English",
      "kind": "captions",
      "default": true
    }
  ],
  "intro": { "start": 31, "end": 59 },
  "outro": { "start": 273, "end": 300 },
  "server": 4
}
```

### Fallback Stream Domains

If `megaplay.buzz` fails, try in order:

1. `https://megaplay.buzz/stream/getSources?id={serverId}`
2. `https://animeplay.video/stream/getSources?id={serverId}`
3. `https://embed.anicdn.to/stream/getSources?id={serverId}`

All domains require the same `X-Requested-With` and `Referer` headers.

## Discovery

### API Migration

The original `myani.cfd` API went offline in early 2026. The new backend `animedata.cfd` was discovered by:

1. Loading `https://hianimes.se/watch/one-piece-episode-1119-qo89wh`
2. Inspecting the Next.js webpack chunks
3. Finding the watch page chunk: `page-42f73115c0daf40b.js`
4. Searching for `concat` patterns: `concat(f,"/episodes/")` and `concat(f,"/episode/")`
5. Variable `f` resolved to `https://animedata.cfd/api`

### Chunk Map

| Chunk | Path | Purpose |
|-------|------|---------|
| `page-42f73115c0daf40b.js` | `app/watch/[epslug]` | React component, fetches episode data |
| `188-94239e07cc28c9b0.js` | shared | axios instance |
| `239-3bafa837856e2f90.js` | shared | query client |

### Pagination Discovery

The `start` parameter was found by testing:
- `?page=12` → ignored, returns 1–100
- `?offset=1100` → ignored, returns 1–100
- `?start=1100` → returns 1100–1160 ✓

This indicates the backend uses `start` as an episode-number-based cursor, not a page index.

## Usage

```js
const hianime = require('./services/hianime');

// 1. Search for anime
const results = await hianime.search('one piece');
const anime = results[0];

// 2. Get episodes (find the one you want)
const epData = await hianime.getEpisodes(anime._id, 1119);
const episode = epData.episodes.find(e => e.episodeNumber === 1119);

// 3. Resolve servers from episode slug
const servers = await hianime.getServers(episode.slugs[0]);
// { sub: [{ id: "128113", name: "HD-1", format: "json" }], dub: [...] }

// 4. Extract stream
const stream = await hianime.getAnimeStream({
  id: anime._id,
  type: 'sub',
  episode: 1119
});
// stream.stream.link.file = "https://cdn.mewstream.buzz/anime/.../master.m3u8"
```

## Verification

```bash
$ curl -s -H "User-Agent: Mozilla/5.0" \
  -H "X-Requested-With: XMLHttpRequest" \
  "https://animedata.cfd/api/episode/one-piece-episode-1119-qo89wh" \
  | grep -o 's-2/[0-9]\+'
s-2/128113

$ curl -s -H "User-Agent: Mozilla/5.0" \
  -H "X-Requested-With: XMLHttpRequest" \
  -H "Referer: https://megaplay.buzz/" \
  "https://megaplay.buzz/stream/getSources?id=128113" \
  | python3 -c "import sys, json; d=json.load(sys.stdin); print(d['sources']['file'])"
https://cdn.mewstream.buzz/anime/3a9de64a6c62c8cd6ff8320bafb8452f/922aff78d1a9ea4e258cec74b2346c66/master.m3u8

$ curl -s -L -H "Referer: https://megaplay.buzz/" \
  -H "User-Agent: Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36" \
  "https://cdn.mewstream.buzz/anime/3a9de64a6c62c8cd6ff8320bafb8452f/922aff78d1a9ea4e258cec74b2346c66/master.m3u8" \
  | head -3
#EXTM3U
#EXT-X-STREAM-INF:PROGRAM-ID=1,BANDWIDTH=1332631,RESOLUTION=1920x1080,FRAME-RATE=23.974
```
