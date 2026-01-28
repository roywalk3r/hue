# LiveTV Module Usage Guide

This guide details the available features and API endpoints for the LiveTV scraper (`livetv.sx` integration).

## Overview

The LiveTV module allows you to:
1.  **Browse Sports:** Get a list of available sports categories.
2.  **List Matches:** Get upcoming and live matches for a specific sport.
3.  **Get Variants:** Retrieve available stream links (webplayer wrappers) for a specific match.
4.  **Resolve Streams:** Extract direct `.m3u8` playback links from the wrappers.

## API Endpoints

### 1. Get Sports Categories
**Endpoint:** `GET /api/livetv/sports`

Returns a list of sports with IDs and icons.

**Response:**
```json
{
  "sports": [
    {
      "id": "1",
      "title": "Football",
      "url": "https://livetv.sx/enx/allupcomingsports/1/",
      "icon": "https://livetv.sx/img/s1.png"
    },
    ...
  ]
}
```

### 2. Get Matches
**Endpoint:** `GET /api/livetv/matches`

**Query Parameters:**
*   `sportId` (optional): The ID of the sport (e.g., `1` for Football). If omitted, returns "All Upcoming".

**Response:**
```json
{
  "matches": [
    {
      "slug": "123456",
      "title": "Real Madrid vs Barcelona",
      "url": "https://livetv.sx/enx/eventinfo/123456/...",
      "timeText": "20:45 (La Liga)",
      "isLive": true
    },
    ...
  ]
}
```

### 3. Get Match Streams (Standard JSON)
**Endpoint:** `GET /api/livetv/match`

**Query Parameters:**
*   `url` (required): The full LiveTV match URL (from the matches endpoint).
*   `resolve` (optional): Set to `true` to fully resolve links to `.m3u8`. If `false` (default), only returns the wrapper links.

**Behavior:**
*   Waits for **ALL** streams to be resolved before sending a response.
*   Can be slow (10-30s) depending on the number of streams.

**Response:**
```json
{
  "streams": [
    {
      "title": "Stream 1 (1080p)",
      "url": "https://example.com/playlist.m3u8",
      "quality": "1080p",
      "proxyUrl": "http://localhost:3000/api/proxy?..."
    }
  ]
}
```

---

## ⚡ Real-Time Streaming (SSE)

**Endpoint:** `GET /api/livetv/match`

**Query Parameters:**
*   `url` (required): The full LiveTV match URL.
*   `stream` (required): Set to `true`.

**Behavior:**
*   Uses **Server-Sent Events (SSE)**.
*   Sends resolved streams **one by one** as they are found.
*   Does not wait for the entire process to finish before returning data.
*   **Significantly** improves perceived performance.

### Client-Side Implementation (JavaScript)

```javascript
const eventSource = new EventSource('/api/livetv/match?stream=true&url=' + encodeURIComponent(matchUrl));

eventSource.onmessage = (event) => {
    const streamData = JSON.parse(event.data);
    
    // Check for errors in the stream object
    if (streamData.error) {
        console.warn('Stream failed:', streamData.error);
        return;
    }

    console.log('New Stream Found:', streamData);
    
    // Add to your UI immediately
    addStreamToPlayerList({
        name: streamData.title,
        url: streamData.proxyUrl || streamData.url
    });
};

eventSource.onerror = (err) => {
    console.log('Stream search finished or closed.');
    eventSource.close();
};
```

**Stream Data Structure:**
```json
{
  "title": "Stream 1 (720p)",
  "url": "https://...",
  "quality": "720p",
  "language": "English",
  "proxyUrl": "http://..."
}
```

## CLI / Testing

You can verify the scraper logic (without the API server) using the included test script.

```bash
node test-livetv.js
```

This will:
1.  Fetch sports.
2.  Fetch matches for the first sport.
3.  Pick a match and list its variants.
4.  Use Playwright to resolve the first variant to an `.m3u8`.
