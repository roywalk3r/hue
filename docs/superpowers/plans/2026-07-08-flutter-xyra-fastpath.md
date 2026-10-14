# Flutter xyra fast-path race — implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an xyra fast-path race to the Flutter player so tap-to-first-frame drops from 10–20 s to sub-second on the happy path, mirroring the proven Nuxt `useHueStreamSSE` race.

**Architecture:** Inside `SseStreamAggregator.aggregate()`, fire a parallel `Timer(Duration.zero, ...)` that calls a new `StreamRemote.fetchXyra()` and merges its (already-decrypted, already-sorted) stream list into the existing `SseAggregateState.streams`. The existing `autoPickStreamProvider` is source-agnostic and picks the first stable direct stream regardless of which branch produced it, so no provider or player page change is required.

**Tech Stack:** Flutter 3.x, Dart 3, Riverpod 3.x, `package:http` + `package:http/testing.dart` (MockClient for unit tests), `flutter_test`, `flutter analyze`.

## Global Constraints

- Branch: `fix/flutter-xyra-fastpath` (create from `main` before Task 1)
- Conventional commits, scope-first, imperative mood, lowercase
- 8 s xyra timeout (below the existing 10 s SSE slow-deadline; mirrors Nuxt `useHueStreamSSE`)
- xyra does **not** add `nocache=1` (xyra has its own upstream cache; the existing `fetchStreams` HTTP fallback at 3 s still does)
- Auth: `x-api-key: EnvConfig.apiKey` only — no Bearer token on xyra
- xyra is **movie/tv only** — drama path (MovieBox direct) is unchanged and does not call xyra
- xyra failures (any kind) are swallowed — SSE remains the source of truth
- xyra response is **already kkey-decrypted and sorted** inside `fetchXyra`; the aggregator does NOT re-run `decryptStreamsForSse` or `rewriteStreamUrls` (those are SSE-parser-internal helpers for raw SSE event payloads, not the plain JSON xyra returns)
- URL-dedup at the merge point: if SSE has already produced streams by the time xyra lands, dedup by URL before appending
- `finalized` guard prevents xyra writes after the SSE stream has closed
- `xyraTimer.cancel()` in the existing `finally` block alongside `fallbackTimer.cancel()`
- Do **not** touch `HueStreamSseClient`, `sseStreamsProvider`, `autoPickStreamProvider`, or `player_page.dart`
- No comments unless the WHY is non-obvious
- `flutter analyze` clean and `flutter test` green at the end of every task that touches a file

---

### Task 1: Create `fix/flutter-xyra-fastpath` branch

**Files:** none

- [ ] **Step 1: Confirm working tree is clean**

Run from `/home/rseann/projects/flutter/hue`:
```bash
git status
```
Expected: no uncommitted changes (other than the spec/plan docs that are expected to be untracked).

- [ ] **Step 2: Create and check out the branch**

```bash
cd /home/rseann/projects/flutter/hue
git checkout main
git pull
git checkout -b fix/flutter-xyra-fastpath
```

- [ ] **Step 3: Verify branch**

```bash
git rev-parse --abbrev-ref HEAD
```
Expected: `fix/flutter-xyra-fastpath`

---

### Task 2: Add `StreamRemote.fetchXyra` with TDD

**Files:**
- Create: `test/features/player/data/stream_remote_test.dart`
- Modify: `lib/features/player/data/stream_remote.dart`

**Interfaces:**
- Consumes: `package:http` `Client`, `EnvConfig.rustBaseUrl`, `EnvConfig.apiKey`, `EnvConfig.kkey`, `KkeyCrypto.decrypt`
- Produces: `StreamRemote.fetchXyra({type, tmdbId, season?, episode?, title?, detailPath?}) → Future<List<HueStream>>` — already kkey-decrypted, sorted via `_sortStreams`, no `nocache=1`

- [ ] **Step 1: Write the failing test file**

Create `test/features/player/data/stream_remote_test.dart`:

```dart
import 'dart:async';
import 'dart:convert';
import 'package:flutter_test/flutter_test.dart';
import 'package:hue/config/env/env_config.dart';
import 'package:hue/features/player/data/stream_remote.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';

void main() {
  group('StreamRemote.fetchXyra', () {
    test('decrypts, sorts, and returns the xyra stream list', () async {
      final captured = <Uri>[];
      final remote = StreamRemote(
        client: MockClient((request) async {
          captured.add(request.url);
          return http.Response(
            jsonEncode({
              'success': true,
              'tmdb_id': '155',
              'count': 2,
              'streams': [
                {
                  'url': 'https://cdn.example/movie-720p.m3u8',
                  'title': '720p',
                  'quality': '720p',
                  'provider': 'xyra',
                  'format': 'hls',
                  'headers': <String, String>{},
                },
                {
                  'url': 'https://cdn.example/movie-1080p.m3u8',
                  'title': '1080p',
                  'quality': '1080p',
                  'provider': 'xyra',
                  'format': 'hls',
                  'headers': <String, String>{},
                },
              ],
            }),
            200,
            headers: {'content-type': 'application/json'},
          );
        }),
      );

      final streams = await remote.fetchXyra(type: 'movie', tmdbId: '155');

      expect(streams, hasLength(2));
      expect(streams.first.quality, '1080p');
      expect(streams.first.provider, 'xyra');
      expect(streams.first.url, 'https://cdn.example/movie-1080p.m3u8');
      expect(streams.first.isHls, isTrue);
    });

    test('throws on non-200', () async {
      final remote = StreamRemote(
        client: MockClient((_) async => http.Response('bad gateway', 502)),
      );
      expect(
        () => remote.fetchXyra(type: 'movie', tmdbId: '155'),
        throwsA(isA<Exception>()),
      );
    });

    test('throws TimeoutException after 8s on hung client', () async {
      final remote = StreamRemote(
        client: MockClient((_) async {
          final c = Completer<http.Response>();
          return c.future;
        }),
      );
      await expectLater(
        remote.fetchXyra(type: 'movie', tmdbId: '155'),
        throwsA(isA<TimeoutException>()),
      );
    }, timeout: const Timeout(Duration(seconds: 12)));

    test('returns empty list when streams is empty', () async {
      final remote = StreamRemote(
        client: MockClient(
          (_) async => http.Response(
            jsonEncode({'success': true, 'count': 0, 'streams': <dynamic>[]}),
            200,
            headers: {'content-type': 'application/json'},
          ),
        ),
      );
      final streams = await remote.fetchXyra(type: 'movie', tmdbId: '155');
      expect(streams, isEmpty);
    });

    test('TV path includes season and episode query params', () async {
      final captured = <Uri>[];
      final remote = StreamRemote(
        client: MockClient((request) async {
          captured.add(request.url);
          return http.Response(
            jsonEncode({'streams': <dynamic>[]}),
            200,
            headers: {'content-type': 'application/json'},
          );
        }),
      );

      await remote.fetchXyra(
        type: 'tv',
        tmdbId: '39340',
        season: 1,
        episode: 3,
      );

      expect(captured, hasLength(1));
      final u = captured.single;
      expect(u.path, '/api/streams/xyra/tv/39340');
      expect(u.queryParameters['season'], '1');
      expect(u.queryParameters['episode'], '3');
    });

    test('does not send nocache=1 (xyra has its own cache)', () async {
      final captured = <Uri>[];
      final remote = StreamRemote(
        client: MockClient((request) async {
          captured.add(request.url);
          return http.Response(
            jsonEncode({'streams': <dynamic>[]}),
            200,
            headers: {'content-type': 'application/json'},
          );
        }),
      );
      await remote.fetchXyra(type: 'movie', tmdbId: '155');
      expect(captured.single.queryParameters['nocache'], isNull);
    });
  });
}
```

- [ ] **Step 2: Run the tests to verify they fail**

```bash
cd /home/rseann/projects/flutter/hue
flutter test test/features/player/data/stream_remote_test.dart
```
Expected: all tests fail with "The method 'fetchXyra' isn't defined" or compile error on the new test file.

- [ ] **Step 3: Add the `fetchXyra` method to `StreamRemote`**

Open `lib/features/player/data/stream_remote.dart`. After the existing `fetchStreams` method (the one that ends with `return _sortStreams(streams);` at line 235), add:

```dart
/// Xyra fast-path. Mirrors `fetchStreams` but hits the xyra aggregator at
/// `/api/streams/xyra/{type}/{tmdbId}` which returns in ~1 s on the happy
/// path. Already kkey-decrypted and sorted on return. No `nocache=1` —
/// xyra has its own upstream cache.
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
  final resp = await _client.get(uri, headers: headers).timeout(
        const Duration(seconds: 8),
      );
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

- [ ] **Step 4: Re-run the tests to verify they pass**

```bash
flutter test test/features/player/data/stream_remote_test.dart
```
Expected: all 6 tests pass.

- [ ] **Step 5: Run `flutter analyze`**

```bash
flutter analyze lib/features/player/data/stream_remote.dart test/features/player/data/stream_remote_test.dart
```
Expected: `No issues found!`

- [ ] **Step 6: Commit**

```bash
git add lib/features/player/data/stream_remote.dart test/features/player/data/stream_remote_test.dart
git commit -m "feat(player): add StreamRemote.fetchXyra fast-path"
```

---

### Task 3: Race xyra inside `SseStreamAggregator.aggregate`

**Files:**
- Modify: `lib/features/streams/data/data_sources/sse_aggregator.dart`

**Interfaces:**
- Consumes: `StreamRemote.fetchXyra(type, tmdbId, season?, episode?, title?, detailPath?)` (Task 2); the existing closure-captured `current: SseAggregateState`, `stateCtrl: StreamController<SseAggregateState>`, `finalized: bool`, and the existing 3 s `fallbackTimer`
- Produces: A new xyra race branch — fires once on `Timer(Duration.zero, ...)` for movie/tv, dedup-merges into `current.streams` by URL, emits via `stateCtrl.add(current)`, swallows all failures. Cancels in the existing `finally` block.

- [ ] **Step 1: Add the xyra race branch**

Open `lib/features/streams/data/data_sources/sse_aggregator.dart`. Locate the `// If SSE never produces anything within 3s, fall back to a single HTTP` comment block (line 237) and the `fallbackTimer = Timer(...)` immediately after it (line 239). Immediately after that `fallbackTimer` declaration and **before** the `try { yield* stateCtrl.stream; }` block (line 268), insert:

```dart
    // Race xyra in parallel. Sub-second on the happy path; ~1 s typical.
    // Movie/tv only — drama uses the direct MovieBox path above.
    final xyraTimer = Timer(Duration.zero, () async {
      if (isDrama || finalized) return;
      try {
        final xyraStreams = await _remote.fetchXyra(
          type: type,
          tmdbId: tmdbId,
          season: season,
          episode: episode,
          title: title,
          detailPath: detailPath,
        );
        if (finalized || xyraStreams.isEmpty) return;

        if (current.streams.isNotEmpty) {
          final existing = current.streams.map((s) => s.url).toSet();
          final fresh =
              xyraStreams.where((s) => !existing.contains(s.url)).toList();
          if (fresh.isEmpty) return;
          final freshProviders = fresh
              .map((s) => s.provider ?? 'xyra')
              .where((p) => !current.providers.contains(p))
              .toList();
          current = current.copyWith(
            streams: [...current.streams, ...fresh],
            providers: [...current.providers, ...freshProviders],
            total: current.streams.length + fresh.length,
          );
        } else {
          current = current.copyWith(
            streams: xyraStreams,
            providers: xyraStreams
                .map((s) => s.provider ?? 'xyra')
                .toSet()
                .toList(),
            total: xyraStreams.length,
            startedAt: DateTime.now(),
          );
        }
        if (!stateCtrl.isClosed) stateCtrl.add(current);
      } on TimeoutException {
        debugPrint('[XYRA] timed out after 8s');
      } catch (e) {
        debugPrint('[XYRA] fetch failed: $e');
      }
    });
```

- [ ] **Step 2: Cancel the xyra timer in the `finally` block**

In the same file, locate the `finally` block at line 270 (the one containing `fallbackTimer.cancel();`). Add `xyraTimer?.cancel();` immediately after `fallbackTimer.cancel();`. The final shape of the block should be:

```dart
    } finally {
      fallbackTimer.cancel();
      xyraTimer?.cancel();
      await sub.cancel();
      await sse.close();
      if (!stateCtrl.isClosed) await stateCtrl.close();
    }
```

- [ ] **Step 3: Run `flutter analyze`**

```bash
flutter analyze lib/features/streams/data/data_sources/sse_aggregator.dart
```
Expected: `No issues found!`

- [ ] **Step 4: Run the full test suite**

```bash
flutter test
```
Expected: all existing tests pass; the new `stream_remote_test.dart` tests from Task 2 still pass.

- [ ] **Step 5: Smoke test xyra is actually called**

From the host:
```bash
TOKEN=$(curl -s -X POST http://127.0.0.1:24609/api/auth/token | jq -r .token)
curl -s "http://127.0.0.1:24609/api/streams/xyra/movie/155" \
  -H "Authorization: Bearer $TOKEN" \
  | jq '{count, providers: [.streams[].provider] | unique}'
```
Expected: a JSON object with `count > 0` and a non-empty `providers` list (e.g. `["4KHDHub", "DahmerMovies", "Vixsrc", "NoTorrent"]`).

This confirms the upstream xyra endpoint is reachable from this network. (The Flutter aggregator will hit the same endpoint through `EnvConfig.rustBaseUrl`.)

- [ ] **Step 6: Commit**

```bash
git add lib/features/streams/data/data_sources/sse_aggregator.dart
git commit -m "feat(streams): race xyra in SseStreamAggregator.aggregate"
```

---

### Task 4: Verify in a running app

**Files:** none (verification only)

- [ ] **Step 1: Confirm the project builds**

```bash
cd /home/rseann/projects/flutter/hue
flutter build apk --debug
```
Expected: build succeeds (warnings about other files are fine; no errors).

- [ ] **Step 2: Run `flutter analyze` on the whole project**

```bash
flutter analyze
```
Expected: `No issues found!` (or the same baseline as before the change).

- [ ] **Step 3: Run the full test suite one more time**

```bash
flutter test
```
Expected: all tests pass.

- [ ] **Step 4: Manual smoke — fast-path on a known movie**

```bash
flutter run -d <your-device>
```
1. Open the app.
2. Pick a known movie (e.g. The Dark Knight, TMDB 155).
3. Tap Play.
4. Watch the `flutter logs` output for `[XYRA]` lines.
5. Confirm a `[XYRA]` line appears within ~1 s of opening the player, and the first frame renders shortly after.
6. Open a TV episode; confirm the same `[XYRA]` log line appears and the URL contains `season=` and `episode=`.

- [ ] **Step 5: Manual smoke — drama path is untouched**

1. Open a drama.
2. Watch the `flutter logs` output.
3. Confirm **no** `[XYRA]` line is emitted (drama uses MovieBox direct, xyra is gated by `isDrama == false`).

- [ ] **Step 6: Confirm no regressions on the existing SSE path**

1. Force xyra to fail (point `EnvConfig.rustBaseUrl` at a non-existent host, or block the network for the xyra request only).
2. Open a movie.
3. Confirm playback still works via the SSE round + 3 s HTTP fallback; the player does not hang.

- [ ] **Step 7: Commit any leftover changes (if any)**

```bash
git status
```
If clean, no commit needed. If `flutter run` left stale build artifacts, `.gitignore` should cover them — but if anything slipped through, commit with `chore(player): smoke test artifacts` or similar.

---

## Self-review

**1. Spec coverage:**

| Spec section | Plan task |
|---|---|
| Add `StreamRemote.fetchXyra` API | Task 2 |
| Race xyra inside `SseStreamAggregator.aggregate` | Task 3 |
| Movie/tv only, not drama | Task 3 (gated by `if (isDrama || finalized) return;`) |
| 8 s timeout | Task 2 (`.timeout(const Duration(seconds: 8))`) |
| No `nocache=1` | Task 2 (test 6 explicitly verifies absence) |
| URL-dedup on merge | Task 3 (`existing` set in the dedup branch) |
| `finalized` guard | Task 3 (every write checks `finalized`) |
| Cancel in `finally` | Task 3 (Step 2) |
| xyra failures swallowed | Task 3 (`catch` clause + `TimeoutException` arm) |
| Test strategy (success / non-200 / timeout / empty / TV / no-nocache) | Task 2 (six test cases) |
| Manual smoke (movie, TV, drama) | Task 4 (steps 4–6) |
| `flutter analyze` clean | Task 2 Step 5, Task 3 Step 3, Task 4 Step 2 |
| `flutter test` green | Task 2 Step 4, Task 3 Step 4, Task 4 Step 3 |
| Branch `fix/flutter-xyra-fastpath` | Task 1 |
| Conventional commits | Tasks 2/3/4 (commit steps) |
| Drama path unchanged | Task 3 (xyra race gated by `isDrama`; no other code paths touched) |
| Untouched files: `HueStreamSseClient`, `sseStreamsProvider`, `autoPickStreamProvider`, `player_page.dart`, `fetchStreams` | No task touches any of these |

**2. Placeholder scan:** No TBDs. No "implement later" steps. Every code step shows full code.

**3. Type consistency:** `fetchXyra` signature in Task 2 Step 3 matches the call site in Task 3 Step 1 verbatim. `SseAggregateState.copyWith` parameters match the existing class shape (line 40 of `sse_aggregator.dart`). `StreamRemote` field `_client` is the same one used by `fetchStreams`. `_sortStreams` and `_decryptStream` are pre-existing private methods — Task 2's implementation calls them in the same order as `fetchStreams`.

**Gaps:** None. All spec requirements are covered.
