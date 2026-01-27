# LiveTV Integration for Hue API

This document describes the LiveTV.sx integration that adds live sports streaming capabilities to the Hue API.

## Overview

LiveTV.sx is a sports streaming aggregator that provides:
- Multiple sports categories (Football, Basketball, Tennis, etc.)
- Live and upcoming match listings
- Multiple stream variants per match
- Direct M3U8 stream URLs

## Implementation

### Files Added/Modified

1. **`utils/playwrightResolver.js`** - Utility to resolve webplayer URLs using Playwright
2. **`services/livetvScraper.js`** - Core scraper for LiveTV.sx
3. **`providers/livetv.js`** - Provider implementation compatible with the registry
4. **`routes/livetv.js`** - Express routes for LiveTV-specific endpoints
5. **`test-livetv.js`** - Test script for LiveTV functionality

### API Endpoints

#### 1. Get Sports and Matches
```
GET /api/livetv
GET /api/livetv?sportId={sportId}
```
Returns available sports categories and matches.

#### 2. Get Sports Categories
```
GET /api/livetv/sports
```
Returns all available sports with icons and IDs.

#### 3. Get Matches
```
GET /api/livetv/matches?sportId={sportId}
```
Returns matches for a specific sport or all matches if no sportId provided.

#### 4. Get Match Streams
```
GET /api/livetv/match?url={matchUrl}&resolve={true|false}
```
Returns stream variants for a specific match. Set `resolve=true` to get direct M3U8 URLs.

#### 5. Direct Stream Access (Provider-style)
```
GET /livetv/{matchName}
```
Extracts streams directly using the provider system.

## Usage Examples

### 1. Browse Sports
```javascript
// Get all sports
const response = await fetch('/api/livetv/sports');
const { sports } = await response.json();
```

**Sample Response:**
```json
{
  "sports": [
    {
      "id": "1",
      "title": "Football",
      "url": "https://livetv.sx/enx/allupcomingsports/1/",
      "icon": "https://livetv.sx/img/s1.gif"
    },
    {
      "id": "2",
      "title": "Ice Hockey",
      "url": "https://livetv.sx/enx/allupcomingsports/2/",
      "icon": "https://livetv.sx/img/s2.gif"
    }
  ]
}
```

### 2. Get Matches
```javascript
// Get matches for football (id: 1)
const matchesResponse = await fetch(`/api/livetv/matches?sportId=1`);
const { matches } = await matchesResponse.json();
```

**Sample Response:**
```json
{
  "matches": [
    {
      "slug": "2245678",
      "title": "Real Madrid – Barcelona",
      "url": "https://livetv.sx/enx/eventinfo/2245678_real_madrid_barcelona/",
      "timeText": "Today, 21:00",
      "isLive": true,
      "id": "2245678"
    }
  ]
}
```

### 3. Get Stream for a Match
```javascript
// Get stream variants
const variantsResponse = await fetch(`/api/livetv/match?url=${encodeURIComponent(match.url)}&resolve=true`);
const { streams } = await variantsResponse.json();
```

**Sample Response:**
```json
{
  "streams": [
    {
      "title": "Stream 1",
      "url": "https://cmn1.vdcast.live/...",
      "quality": "HD",
      "language": "English",
      "isLive": true,
      "proxyUrl": "http://localhost:3000/api/proxy?url=...&headers=..."
    }
  ]
}
```

### 4. Direct Match Access
```javascript
// Using match slug
const response = await fetch(`/livetv/real-madrid-vs-barcelona`);
const { streams } = await response.json();
```

## Integration Notes

1. **Proxy Support**: Streams are proxied through `/api/proxy` to handle CORS and Headers.
2. **Provider Name**: Registered as "Arena" in the API responses.
3. **Caching**: LiveTV streams are not cached due to their live nature.
4. **Browser Requirements**: Uses Playwright (Chromium) for stream resolution.

## Testing

Run the test script to verify functionality:
```bash
node test-livetv.js
```

This will:
1. Fetch sports categories
2. Get matches for the first sport
3. Fetch stream variants for a match
4. Resolve a stream to direct URL

## Comparison with Livesport

| Feature | Livesport | LiveTV |
|---------|-----------|---------|
| Source | livesport.su | livetv.sx |
| Sports | Limited | Wide variety |
| Stream Quality | Variable | Multiple options |
| Resolution | Direct | Playwright required |
| Provider Name | Stadium | Arena |

Both providers can be used simultaneously for better coverage.
