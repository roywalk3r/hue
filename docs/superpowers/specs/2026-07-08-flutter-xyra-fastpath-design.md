# Flutter xyra fast-path race — design

**Status:** proposed
**Date:** 2026-07-08
**Branch:** `fix/flutter-xyra-fastpath`
**Project:** flutter/hue

## Problem

Tap-to-first-frame on the Flutter player takes 10–20 s on a fast connection, on every content type the user can play. The Rust SSE aggregator at `/api/streams/{type}/{tmdbId}/live` is the only stream source. Its happy path is 3–5 s before the first playable stream arrives (the aggregator waits on its slow phase), and the Flutter player only auto-starts on the first stable direct stream — so a slow SSE start cascades straight into the user-visible spinner.

The Nuxt `hue-ui` solved this two weeks ago by racing xyra (sub-second, residential egress that sidesteps Cloudflare 403s on vixsrc / vidrock / etc.) against the SSE aggregator, keeping whichever lands first. The Flutter player still has the un-raced SSE pipeline.

## Goal

Match the Nuxt fast-path: when the player opens, race xyra against the SSE aggregator. The first non-empty response wins and the player auto-starts from it. On the happy path this cuts tap-to-frame from 3–5 s (SSE) to ~1 s (xyra).

## Non-goals

- Drama path. Drama already has its own direct MovieBox path (`SseStreamAggregator.aggregate()` short-circuits for `MediaType.drama`); it does not touch the SSE pipeline and is unchanged.
- Per-provider xyra fan-out. The single all-providers xyra route is enough.
- Replacing the SSE aggregator. xyra is a fast-path addition, not a replacement.
- Persisting xyra results to disk between sessions. The player only needs them within one playback session.
- Caching xyra results in SharedPreferences. Same reason.
- Touching `useHueStreamSSE` (the non-SSE Nuxt composable). Out of scope by design.

## Architecture

### Current state

```
SseStreamAggregator.aggregate(args)
  ├─ MovieBox direct (drama only)
  └─ SSE connection to /api/streams/{type}/{tmdbId}/live
       └─ on empty after 3s → HTTP fallback to /api/streams/{type}/{tmdbId}

sseStreamsProvider(args)            ← AsyncValue<SseAggregateState>
autoPickStreamProvider(args)        ← HueStream?  (picks first stable non-iframe)
player_page.dart                    ← watches both, auto-starts via _openStream()
```

The aggregator is the **only** stream source for movie/tv. Its internal `_streamController` emits a sequence of `SseAggregateState` updates as streams arrive.

### New state

```
SseStreamAggregator.aggregate(args)
  ├─ MovieBox direct (drama only)
  ├─ SSE connection to /api/streams/{type}/{tmdbId}/live  (unchanged)
  └─ NEW: HTTP GET /api/streams/xyra/{type}/{tmdbId}  (movie/tv only)
       └─ result streamed into the same _streamController (URL-deduped)

sseStreamsProvider(args)            ← unchanged
autoPickStreamProvider(args)        ← unchanged (picks first stable, source-agnostic)
player_page.dart                    ← unchanged
```

Both xyra and SSE feed into the **same** `SseAggregateState.streams` list. `autoPickStreamProvider` reads that list, finds the first stable direct stream, and emits it. The provider does not know — and does not need to know — which source produced the stream.

### Race semantics

`SseStreamAggregator.aggregate()` becomes a fan-in:

1. **Always:** open the SSE connection (existing). Emit events as they arrive. If SSE yields a stream before xyra resolves, the player auto-starts from it. No regression on the existing path.
2. **Movie/tv only:** concurrently `await fetchXyra(args)`. When it resolves, run the existing `appendStreams` (URL-dedup) into the same controller, then close the xyra branch. xyra's response is treated as a single big batch of `provider` events — same shape the SSE parser produces.
3. **3-second HTTP fallback:** unchanged. If SSE is empty at 3 s, the existing fallback still fires. xyra does not replace it; xyra races it.
4. **Drama:** unchanged. MovieBox direct path is taken, xyra is not called.

### Why fan-in at the aggregator and not the provider layer

`SseStreamAggregator.aggregate()` already owns the merge logic (`appendStreams` does URL dedup). Adding a second concurrent source there means a single dedup pass, no merge logic in providers, and no change to the public provider surface. A separate `xyraStreamsProvider` would require `autoPickStreamProvider` to read both providers and dedup again — duplicated logic.

### What does NOT change

- `HueStreamSseClient` — SSE parser is untouched.
- `sseStreamsProvider` — signature and behaviour unchanged.
- `autoPickStreamProvider` — already source-agnostic, picks first stable.
- `player_page.dart` — already watches the right providers, already auto-starts.
- `StreamRemote.fetchStreams()` — the 45 s single-shot HTTP fallback stays as-is.
- The Rust side — `/api/streams/xyra/{type}/{tmdbId}` is already deployed; xyra is just not yet called from Flutter.
- Drama path.

## API

`StreamRemote.fetchXyra(...)` — new method, parallel to the existing `fetchStreams()`. Same parameter list, same return shape, different endpoint.

```dart
/// Xyra fast-path. Mirrors fetchStreams() but hits the xyra aggregator
/// at /api/streams/xyra/{type}/{tmdbId} which returns in ~1 s on the
/// happy path (sub-second, residential egress).
Future<List<HueStream>> fetchXyra({
  required String type,
  required String tmdbId,
  int? season,
  int? episode,
  String? title,
  String? detailPath,
}) async {
  final params = <String, String>{};
  if (season != null) params['season'] = '$season';
  if (episode != null) params['episode'] = '$episode';
  if (title != null) params['title'] = title;
  if (detailPath != null) params['dp'] = detailPath;

  final uri = Uri.parse(
    '${EnvConfig.rustBaseUrl}/api/streams/xyra/$type/$tmdbId',
  ).replace(queryParameters: params);

  final headers = <String, String>{
    if (EnvConfig.apiKey.isNotEmpty) 'x-api-key': EnvConfig.apiKey,
  };
  final resp = await _client.get(uri, headers: headers).timeout(const Duration(seconds: 8));
  if (resp.statusCode != 200) {
    throw Exception('Xyra fetch failed: ${resp.statusCode}');
  }
  final decoded = jsonDecode(resp.body);
  final rawStreams = (decoded is List<dynamic>
          ? decoded
          : (decoded as Map<String, dynamic>)['streams'] as List<dynamic>?)
      ?? [];

  final streams = rawStreams
      .whereType<Map<String, dynamic>>()
      .map(HueStream.fromJson)
      .map(_decryptStream)
      .toList();
  return _sortStreams(streams);
}
```

Shape:
- **Input:** `type`, `tmdbId`, `season?`, `episode?`, `title?`, `detailPath?` — same as `fetchStreams` (so the aggregator can pass them straight through)
- **Output:** `List<HueStream>` — already kkey-decrypted, sorted via `_sortStreams`, ready to feed `appendStreams`
- **Throws:** `Exception` on non-200, `TimeoutException` after 8 s

The 8-second budget mirrors Nuxt's `useHueStreamSSE` and is below the existing 10 s slow-deadline on the SSE aggregator. On the happy path xyra lands in ~1 s; the cap is for cold starts and slow egress.

Auth matches the existing `fetchStreams` path: `x-api-key: EnvConfig.apiKey` only. xyra does not need a Bearer token.

## Race wiring inside the aggregator

The existing `aggregate()` opens the SSE stream, listens for events, and emits `SseAggregateState` updates into a `StreamController`. Each SSE branch mutates a closed-over `current` and `add`s it. The 3-second HTTP fallback is a `Timer` that also mutates `current` and calls `finalize()`.

The new xyra branch is a parallel `Timer` that:
- runs only when `isDrama == false`
- calls `_remote.fetchXyra(...)` with the same args
- on success: parses streams via the same `_parseStreams` + decrypt + `rewriteStreamUrls` pipeline the SSE branch uses, then emits a new state with the merged stream list
- on any failure: logs and swallows — the SSE pipeline is unaffected

```dart
// In aggregate(), after the existing fallbackTimer is set up:

final xyraTimer = Timer(Duration.zero, () async {
  if (isDrama || finalized) return;
  try {
    final xyraStreams = await _remote.fetchXyra(
      type: type, tmdbId: tmdbId, season: season, episode: episode,
      title: title, detailPath: detailPath,
    );
    if (finalized || xyraStreams.isEmpty) return;
    if (current.streams.isNotEmpty) {
      // SSE already landed. Dedup by URL before appending.
      final existing = current.streams.map((s) => s.url).toSet();
      final fresh = xyraStreams.where((s) => !existing.contains(s.url)).toList();
      if (fresh.isEmpty) return;
      current = current.copyWith(
        streams: [...current.streams, ...fresh],
        providers: [...current.providers, ...fresh.map((s) => s.provider ?? 'xyra').toSet()],
        total: current.streams.length + fresh.length,
      );
    } else {
      current = current.copyWith(
        streams: xyraStreams,
        providers: xyraStreams.map((s) => s.provider ?? 'xyra').toSet().toList(),
        total: xyraStreams.length,
        startedAt: DateTime.now(),
      );
    }
    stateCtrl.add(current);
  } on TimeoutException {
    debugPrint('[XYRA] timed out after 8s');
  } catch (e) {
    debugPrint('[XYRA] fetch failed: $e');
  }
});
```

Notes:
- `Timer(Duration.zero, ...)` keeps the launch pattern consistent with the existing `fallbackTimer` and lets the listen() subscription start cleanly first.
- The `current` variable is captured by closure — same pattern the SSE branch and the HTTP fallback use.
- `finalized` guard prevents writes after the SSE stream is done.
- URL-dedup protects against the case where xyra returns the same upstream stream the SSE round produces later.
- xyra's response is **already kkey-decrypted and sorted** by `fetchXyra` itself, so the aggregator only needs to merge and emit — it does not re-run `decryptStreamsForSse` or `rewriteStreamUrls` (those are the SSE-parser-internal helpers, and xyra's HTTP response is plain JSON, not raw SSE events).
- `unawaited(xyraTimer)` is not needed because we don't await it; the closure runs and the timer fires once.

The xyra branch and the SSE branch are independent; whichever lands first writes to `stateCtrl`. Order of emissions into a single-subscription `StreamController` is preserved, so `sseStreamsProvider` sees the streams in arrival order — and `autoPickStreamProvider` picks the first stable one regardless of which branch produced it.

In the `finally` block (existing), add `xyraTimer.cancel()` next to `fallbackTimer.cancel()`.

## Error handling

| Condition | Behaviour |
|-----------|-----------|
| xyra returns 502 / 504 | Log warn, swallow. SSE / 3 s fallback handles it. |
| xyra returns 200 with `streams: []` | Log debug, swallow. SSE / 3 s fallback handles it. |
| xyra returns malformed JSON | Log warn, swallow. |
| xyra times out (8 s) | Log debug, swallow. |
| xyra throws any other exception | Log warn, swallow. |
| Network unreachable | `fetchXyra` throws → caught → swallowed. |

xyra is best-effort. It can fail in any way and the player still works — it just falls back to the existing SSE path. No new user-visible failure modes.

## Test strategy

### Unit (new test)

`test/features/player/data/stream_remote_test.dart` — covers `fetchXyra` (new file; no existing test for `StreamRemote`):

1. **Success path** — pass a `MockClient` (from `http/testing.dart`) that returns 200 with a known xyra body. Assert the result is a `List<HueStream>` with the right `title`, `provider`, `url`, `quality`, `format`, `headers`. Assert the URL has been kkey-decrypted.
2. **Non-200** — mock 502. Assert `fetchXyra` throws `Exception` with the status code.
3. **Timeout** — pass a `MockClient` that returns a `Future` that never completes. Assert `fetchXyra` throws `TimeoutException` after 8 s.
4. **Empty `streams`** — mock 200 with `{"streams": []}`. Assert empty list returned, no throw.
5. **TV path** — args `{ type: 'tv', tmdbId: '39340', season: 1, episode: 3 }` should hit `/api/streams/xyra/tv/39340?season=1&episode=3`. Assert the captured `Uri` from the mock matches.
6. **Auth header** — assert the request includes `x-api-key: EnvConfig.apiKey` when configured.
7. **No `nocache=1`** — the existing `fetchStreams` adds `nocache=1`; xyra should not (xyra has its own upstream cache).

Use `package:http/testing.dart`'s `MockClient` to inject canned responses. The aggregator-level xyra race is best tested via an integration smoke (next section) since wiring up the timer + state controller + finalize guard needs a real test harness.

### Integration (manual smoke)

1. `flutter run` on a known movie (e.g. The Dark Knight, TMDB 155).
2. Open the player. Watch the network tab in `flutter logs` (or DevTools).
3. Confirm two concurrent requests: `/api/streams/movie/155/live` and `/api/streams/xyra/movie/155`.
4. xyra should land first (~1 s). Player auto-starts from a xyra stream.
5. SSE should still resolve ~3 s later. The player should already be playing; the SSE streams are silently appended to the list (not visible to the user).
6. Repeat for a TV show (any S/E).
7. Open a drama — confirm no `/api/streams/xyra/drama/...` request fires (drama uses MovieBox direct).

### Existing tests

`flutter test` must remain green. The aggregator change is a fire-and-forget add — existing tests of `aggregate()` and `autoPickStreamProvider` should not need updating.

## Files

**Modified (2):**
- `lib/features/player/data/stream_remote.dart` — add `fetchXyra` method (~30 lines), following the shape of the existing `fetchStreams`
- `lib/features/streams/data/data_sources/sse_aggregator.dart` — add xyra race branch in `aggregate()` (~25 lines), fire-and-forget on a `Timer(Duration.zero, ...)`, cancel in the existing `finally` block

**Created (1):**
- `test/features/player/data/stream_remote_test.dart` — new file, covers `fetchXyra` success / non-200 / timeout / empty / TV with season+episode

**Untouched (verification):**
- `lib/features/streams/data/data_sources/sse_client.dart` — unchanged
- `lib/features/streams/presentation/state/sse_streams_provider.dart` — unchanged
- `lib/features/player/presentation/pages/player_page.dart` — unchanged
- `lib/features/player/data/stream_remote.dart::fetchStreams` — unchanged (HTTP fallback)
- The Rust `/api/streams/xyra/{type}/{tmdbId}` endpoint — already deployed

## Out-of-scope follow-ups (parking lot)

- Drama path xyra support. The Rust xyra endpoint doesn't cover drama today; would require Rust-side additions.
- Persisted xyra result cache across sessions.
- A separate `xyraStreamsProvider` for offline pre-fetch on the detail page (could pre-warm on card tap).
- Per-provider xyra fan-out (8 separate routes) for finer-grained selection.

## Acceptance criteria

- [ ] `StreamRemote.fetchXyra` exists with the documented signature
- [ ] `SseStreamAggregator.aggregate` races xyra alongside SSE for movie/tv
- [ ] Drama path does not call xyra
- [ ] xyra failures (any kind) do not break the existing SSE path
- [ ] `flutter test` green; new fetchXyra tests pass
- [ ] `flutter analyze` clean
- [ ] Manual smoke: tap-to-frame under 2 s on a known movie on a fast connection
- [ ] Manual smoke: drama plays as before, no xyra request fires
- [ ] Branch is `fix/flutter-xyra-fastpath` and commits follow the project's conventional-commit style
