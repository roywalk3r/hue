# Smart Play: `GET /api/play/:type/:tmdbId`

The "just give me one playable URL" endpoint. Used by clients that
want to embed a single HLS link in `<a href="...">` or as a
`video_player` source without forcing the user through a stream
picker.

## What it returns

A single JSON object:

```json
{
  "url": "https://api.vexli.online/ts-proxy?h_token=...",
  "title": "Inception",
  "quality": "1080p",
  "provider": "Luna",
  "format": "hls",
  "subtitles": [...]
}
```

Or `404` if no stream could be found in the time budget.

## Algorithm

`routes/play.rs::play_handler` runs the same fast-phase fan-out as
the main aggregator, then does a `HEAD` check on each candidate URL
to verify the upstream is alive before returning.

```
1. Coalescer + cache check
2. Fast phase — parallel fetch from all healthy fast providers
3. Pick the top 5 candidate URLs (HLS preferred, then MP4)
4. HEAD-check each **concurrently** via a `JoinSet`
   - Per-candidate timeout: `HEAD_CHECK_TIMEOUT_SECS` (default 2 s, env-tunable)
   - On 200: return that URL
   - On non-2xx / timeout: discard that candidate
5. If fast phase yielded no HEAD-passing URL:
   - Run the slow phase
   - HEAD-check those results the same way
6. If still nothing: 404
```

## Why a HEAD check?

The fast phase can return *stale* URLs — a stream that worked
30 minutes ago may have been rotated by the upstream CDN. The
HEAD check filters out the dead ones before the user clicks
play. The cost is one extra round-trip per candidate, capped at
2 s.

## Why this endpoint exists

The main `/api/streams/...` returns a *list* of streams with
metadata. The Player uses that list to populate a stream picker.
Smart Play is for contexts where the stream picker would be
noise — e.g.:

- A `<a href="...">` link in a Telegram preview.
- A "click to play" button in a notification.
- A fallback in the Player when the main picker fails to load.

## Differences from `/api/streams/...`

| | `/api/streams/...` | `/api/play/...` |
|---|---|---|
| Returns | List of streams + metadata | Single best URL |
| Selection | Client picks | Server HEAD-checks |
| Slow phase | Conditional on thin fast | Fires if fast HEAD-checks all fail |
| Response shape | `StreamsResponse` | `PlayResponse` |
| Latency target | ≤ 5 s (fast) | ≤ 7 s (fast + HEADs) |
| Cache | `StreamCache` (4 h) | No cache (always HEAD-checks) |

## Failure modes

- **All candidates 404/timeout**: returns `404` with a JSON body
  explaining which providers were tried. The client should fall
  back to the main `/api/streams/...` endpoint.
- **Auth failure**: 401, like every other API route.
- **Rate limit**: 429, like every other API route.
- **Invalid type (not movie/tv)**: 400.

## Tuning

- `HEAD_CHECK_TIMEOUT_SECS` (default 2, env-tunable) — per-candidate
  timeout for the HEAD probe.
- Max candidates to HEAD-check is hardcoded to `5` in
  `src/routes/play.rs::pick_best_stream`. Edit and rebuild to change.
