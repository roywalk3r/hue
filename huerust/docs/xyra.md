# Xyra streamhub integration

The Rust backend has **two** integration points with
[api.xyra.stream](https://api.xyra.stream):

1. **The `/api/streams/xyra/:type/:tmdbId` fast-path route** — a thin
   pass-through that races against the main aggregator.
2. **The `Aurex` and `Aura (Xyra)` providers** inside the main
   aggregator pipeline.

Both consume the same upstream endpoint
(`https://api.xyra.stream/v1/streamhub/streams`) but have different
guarantees, timeouts, and side effects.

## The fast-path route

`routes/streams.rs::xyra_streams_handler` is a separate handler that
sits next to the main `streams_handler`. It is **not** routed through
the provider pipeline; it doesn't go through the registry, the circuit
breaker, the provider scorer, or the cache.

### Flow

```
Client ──► /api/streams/xyra/movie/27205
              │
              ▼
   xyra_streams_handler
              │
   1. Auth: Bearer token (loopback exempt)
   2. Build upstream URL:
        https://api.xyra.stream/v1/streamhub/streams
          ?api_key=key1
          &tmdb_id=<id>
          &type=movie|series
          [&season=N&episode=N]      ← only for series
   3. 12 s timeout on .send()
   4. JSON-deserialize into:
        { success, streams[], provider_timings{} }
   5. Map each stream to StreamResult:
        - format guessed from URL substring
            .m3u8 / /playlist/   → "hls"
            .mp4 / .mkv          → "mp4"
            (default)            → "hls"
        - provider name passed through from xyra's payload
        - headers (UA / Referer) preserved
   6. wrap_with_proxy() → encrypt URLs into h_token
   7. Return standard StreamsResponse
        (same shape as the main aggregator)
```

### Why the response shape is identical

The browser doesn't know which endpoint answered first. It fires both
in parallel via `useHueStreamSSE` (Nuxt) or
`SseStreamAggregator.aggregate` (Flutter) and consumes the first
non-empty response. Both endpoints return the same envelope, so the
downstream pipeline (`decryptStreams`, `pickBestStream`,
`rewriteStreamUrls`) is unchanged.

### Timeouts and error handling

- **12 s** hard timeout on the upstream call.
- `502 Bad Gateway` on transport / non-2xx upstream response.
- `504 Gateway Timeout` on 12 s expiry.
- `success: false` from xyra → `200` with `count: 0` (treat as
  empty-but-valid, not an error).

### Self-healing on the client

The Rust handler is dumb — it just forwards. The self-healing logic
lives on the client. When xyra upstream flaps:

- **Nuxt** (`useHueStreamSSE.ts`): gated by
  `NUXT_PUBLIC_XYRA_ENABLED` (build-time). When off, the
  race collapses to a single fetch against the main aggregator.
- **Flutter** (`sse_aggregator.dart`): `_XyraHealth` static class
  tracks consecutive failures (timeout / exception / empty). After
  2 consecutive failures the race is skipped for 5 minutes, then
  retried automatically. No rebuild required — the gate is
  process-lifetime and self-recovers.

## The `Aura (Xyra)` and `Aurex` providers

`Aurex` and `Aura (Xyra)` (the provider `real_name` is `vixsrc` because
xyra returns it under that label) are normal `StreamProvider` impls
inside the registry. They are called from the same dispatch path as
`Luna`, `Zenith`, etc. — they participate in fast/slow phases, the
circuit breaker, the provider scorer, and the cache.

Why have both? The fast-path exists for **cold start latency** (it
skips the entire aggregation pipeline). The provider integration
exists for **the slow phase** — when fast providers are thin,
xyra results still get folded into the same StreamsResponse with
dedup-by-URL against the main aggregator's results.

### Gating

- `ENABLE_XYRA_VIXSRC` (default: on) — toggles `Aura (Xyra)`. When
  off, the registry falls back to the direct `Aura` (Vixsrc) provider
  if `ENABLE_VIXSRC` is on. If both are off, the build logs a warning
  that Aura is fully off.
- `Aurex` has no env gate; it is always registered.
- `KissKH (Xyra)` (`kisskh_xyra`) is currently disabled in the
  registry. The direct `Zenith` (kisskh) provider handles tv, and
  the xyra path was returning duplicate / wrong-match results.

## Why not just call xyra from the dispatch path?

The dispatch path:

1. Is sequential — sticky-pref lookup, then fast phase, then slow.
2. Has a coalescer that may serve a cached in-flight result.
3. Has the circuit breaker that may short-circuit a provider.

The fast-path route skips all three. It is a *parallel attempt*, not
a *primary path*. The main aggregator is the source of truth; xyra
is a latency optimization.

## Operations

- **xyra key rotation**: handled server-side by xyra. Our Rust code
  embeds the key in the upstream URL (`api_key=key1`); if the
  embedded key stops working, the upstream returns a 4xx, which the
  handler maps to 502.
- **xyra quota**: not visible from the client. The 12 s timeout
  contains the blast radius.
- **xyra backpressure**: the client-side cooldown (Flutter
  `_XyraHealth`, Nuxt `xyraEnabled`) limits how often we race
  xyra when the upstream is degraded.

See [ARCHITECTURE.md](../ARCHITECTURE.md) for the broader request
flow and the dual-backend strategy.
