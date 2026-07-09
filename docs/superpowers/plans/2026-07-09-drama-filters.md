# Drama page filters — implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a year filter to the Flutter drama page and fix the empty-card rendering in the filtered results grid.

**Architecture:** Backend (Rust) gets `year` threaded through the existing MovieBox `filter()` client and route handler (already partially wired with `#[allow(dead_code)]`). Frontend (Flutter) extends `_DramaFilter` with an optional year, surfaces a Year chip wrap in the filter bottom sheet, and replaces `_PosterCard`'s flat-rectangle fallback with a year+title placeholder.

**Tech Stack:** Rust 1.x (axum, anyhow, serde), Flutter 3.x, Dart 3, Riverpod 3.x.

## Global Constraints

- Branch: `fix/drama-filters` (already created from `feat/branding`)
- Conventional commits, scope-first, imperative mood, lowercase
- Year param is a 4-digit integer (or `null`). Sentinel `0` means "Older (pre-2020)" — the Flutter side filters that client-side; the upstream MovieBox endpoint does not support ranges.
- `_DramaFilter.year` is `int?`; `null` = no year filter, `0` = "Older", positive = specific year
- `_filterUrl` excludes the `year` query param when `_DramaFilter.year` is `null` or `0`
- MovieBox upstream `year` field accepts string "All" or "YYYY" (4-digit year)
- No new providers, routes, or files
- No comments unless the WHY is non-obvious
- Backend `cargo build` + `cargo test` clean at end of Task 1
- Frontend `flutter analyze` clean and `flutter test` green at end of Task 2

---

### Task 1: Thread `year` through Rust MovieBox filter

**Files:**
- Modify: `huerust/src/routes/moviebox.rs:23-34, 295-320`
- Modify: `huerust/src/clients/moviebox.rs:806-852`

**Interfaces:**
- Consumes: `FilterQuery.year: Option<String>` (already declared with `#[allow(dead_code)]`)
- Produces: `MovieBoxClient::filter(page, per_page, country, sort, channel_id, genre, year)` — new `year: Option<&str>` parameter threaded into the upstream payload as `year.unwrap_or("All")`

- [ ] **Step 1: Update `FilterQuery` struct in `huerust/src/routes/moviebox.rs`**

Remove the `#[allow(dead_code)]` attribute on the `year` field. The struct should look exactly like this:

```rust
#[derive(Deserialize)]
pub struct FilterQuery {
    pub page: Option<u32>,
    pub per_page: Option<u32>,
    pub channel_id: Option<u32>,
    #[allow(dead_code)]
    pub genre: Option<String>,
    pub country: Option<String>,
    pub year: Option<String>,
    pub sort: Option<String>,
}
```

Only the `#[allow(dead_code)]` line above `pub year: Option<String>,` is removed (line 31 in the current file). Leave the line above `genre` alone.

- [ ] **Step 2: Update `filter_handler` in `huerust/src/routes/moviebox.rs` (around line 295-320)**

In the `filter_handler` function, extract `year` from the params and pass it to `state.moviebox.filter(...)`. The handler should look exactly like this:

```rust
pub async fn filter_handler(
    State(state): State<MovieBoxRouteState>,
    Query(params): Query<FilterQuery>,
) -> Response {
    let page = params.page.unwrap_or(1);
    let per_page = params.per_page.unwrap_or(28);
    let country = params.country.as_deref();
    let sort = params.sort.as_deref();
    let channel_id = params.channel_id;
    let genre = params.genre.as_deref();
    let year = params.year.as_deref();
    match state
        .moviebox
        .filter(page, per_page, country, sort, channel_id, genre, year)
        .await
    {
        Ok(items) => {
            let resp = serde_json::json!({ "success": true, "page": page, "items": items });
            (StatusCode::OK, Json(resp)).into_response()
        }
        Err(e) => {
            tracing::warn!("MovieBox filter failed: {}", e);
            let resp = serde_json::json!({ "success": false, "page": page, "items": [], "error": e.to_string() });
            (StatusCode::OK, Json(resp)).into_response()
        }
    }
}
```

Only the new `let year = params.year.as_deref();` line and the extra `year` argument to `state.moviebox.filter(...)` are added. Everything else stays byte-identical.

- [ ] **Step 3: Update `MovieBoxClient::filter()` signature in `huerust/src/clients/moviebox.rs` (line 806-852)**

Add `year: Option<&str>` as the last parameter, and replace the hardcoded `"year": "All"` with `year.unwrap_or("All")`. The function should look exactly like this:

```rust
pub async fn filter(
    &self,
    page: u32,
    per_page: u32,
    country: Option<&str>,
    sort: Option<&str>,
    channel_id: Option<u32>,
    genre: Option<&str>,
    year: Option<&str>,
) -> Result<Vec<MovieBoxItem>> {
    let url = format!("{}/subject/filter", BASE_URL);
    let payload = serde_json::json!({
        "page": page,
        "perPage": per_page,
        "channelId": channel_id.unwrap_or(2),
        "genre": genre.unwrap_or("All"),
        "country": country.unwrap_or("All"),
        "year": year.unwrap_or("All"),
        "sort": sort.unwrap_or("ForYou"),
        "tabId": 0,
        "classify": "All"
    });

    let resp = self
        .client
        .post(&url)
        .headers(self.get_base_headers())
        .json(&payload)
        .send()
        .await?;

    if !resp.status().is_success() {
        return Err(anyhow!("MovieBox filter error: {}", resp.status()));
    }

    let data: serde_json::Value = resp.json().await?;
    if data["code"] != 0 {
        return Ok(vec![]);
    }

    let items = data["data"]["items"]
        .as_array()
        .or_else(|| data["data"]["subjectList"].as_array())
        .map(|arr| arr.iter().filter_map(|v| self.map_item(v)).collect())
        .unwrap_or_default();

    Ok(items)
}
```

- [ ] **Step 4: Update callers of `MovieBoxClient::filter()` in `browse_rails`**

`browse_rails` (line 854-892) calls `self.filter(...)` twice. Both call sites need an extra `None` argument. The function should look exactly like this:

```rust
pub async fn browse_rails(
    &self,
    page: u32,
    per_page: u32,
    country: Option<&str>,
    sort: Option<&str>,
    channel_id: Option<u32>,
) -> Result<Vec<serde_json::Value>> {
    if let Some(c) = country {
        let items = self
            .filter(page, per_page, Some(c), sort, channel_id, None, None)
            .await?;
        let rail = serde_json::json!({
            "country": c,
            "items": items
        });
        return Ok(vec![rail]);
    }

    let countries = vec!["Korea", "China", "Japan", "Thailand"];
    let mut rails = Vec::new();

    for c in countries {
        match self
            .filter(page, per_page, Some(c), sort, channel_id, None, None)
            .await
        {
            Ok(items) if !items.is_empty() => {
                rails.push(serde_json::json!({
                    "country": c,
                    "items": items
                }));
            }
            _ => {}
        }
    }

    Ok(rails)
}
```

- [ ] **Step 5: Build and test the Rust changes**

Run from `/home/rseann/projects/Misc/hue/huerust`:

```bash
cargo build 2>&1 | tail -20
```

Expected: build succeeds. There must be no new warnings beyond the existing baseline.

```bash
cargo test 2>&1 | tail -30
```

Expected: all existing tests pass.

- [ ] **Step 6: Smoke-test the new `year` query param against the running server**

```bash
TOKEN=$(curl -s http://127.0.0.1:24609/api/auth/token | jq -r .token)
curl -s "http://127.0.0.1:24609/api/moviebox/filter?country=Korea&sort=Latest&per_page=3&page=1&channel_id=2&year=2025" \
  -H "Authorization: Bearer $TOKEN" \
  | jq '.items | length, [.[] | .releaseDate]'
```

Expected: an integer count, followed by an array of release-date strings (most of which should contain "2025" or "2024"-ish years since MovieBox's year filter is loose). If the count is 0, the upstream is rejecting "2025" — report and stop. If the count is non-zero, the year param is wired correctly.

If `cargo build` failed, or `cargo test` showed new failures, or the smoke test returned 0 items when upstream has data, **report and stop** — do not proceed to Task 2.

- [ ] **Step 7: Commit**

```bash
cd /home/rseann/projects/Misc/hue
git add huerust/src/clients/moviebox.rs huerust/src/routes/moviebox.rs
git commit -m "feat(drama): thread year param through MovieBox filter"
```

---

### Task 2: Add Year filter to the Flutter drama page

**Files:**
- Modify: `lib/features/drama/presentation/pages/drama_page.dart`

**Interfaces:**
- Consumes: existing `_DramaFilter` (line 46-65), `DramaFilterOptions` (line 68-78), `_dramaFilterOptionsProvider` (line 292-298), `_FilterBottomSheet` (line 776-967), `_filterUrl` (line 340-350)
- Produces: `_DramaFilter({country, genre, sort, year})` with `year: int?`; `DramaFilterOptions({countries, genres, sorts, years})`; bottom sheet shows Year chip wrap; `_filterUrl` includes `year=YYYY` when set; `_filteredItems` filtered to releaseDate year < 2020 when `year == 0`

- [ ] **Step 1: Extend `_DramaFilter` with `year`**

In `lib/features/drama/presentation/pages/drama_page.dart`, replace the existing `_DramaFilter` class (line 46-65) with:

```dart
class _DramaFilter {
  final String country;
  final String? genre;
  final String sort;
  final int? year;
  const _DramaFilter({
    this.country = 'All',
    this.genre,
    this.sort = 'Hottest',
    this.year,
  });

  bool get isActive =>
      country != 'All' || genre != null || sort != 'Hottest' || year != null;

  _DramaFilter copyWith({
    String? country,
    String? genre,
    String? sort,
    int? year,
  }) {
    return _DramaFilter(
      country: country ?? this.country,
      genre: genre,
      sort: sort ?? this.sort,
      year: year ?? this.year,
    );
  }
}
```

- [ ] **Step 2: Extend `DramaFilterOptions` with `years`**

Replace the existing `DramaFilterOptions` class (line 68-78) with:

```dart
class DramaFilterOptions {
  final List<String> countries;
  final List<String> genres;
  final List<String> sorts;
  final List<int> years;

  const DramaFilterOptions({
    required this.countries,
    required this.genres,
    required this.sorts,
    required this.years,
  });
}
```

- [ ] **Step 3: Add year list constant and update the fallback provider**

Add a year-list constant above the `_dramaFilterOptionsProvider` definition (line 292). Replace the existing constant + provider block with:

```dart
const _fallbackYears = <int>[2026, 2025, 2024, 2023, 2022, 2021, 2020, 0];

final _dramaFilterOptionsProvider = FutureProvider<DramaFilterOptions>(
  (_) async => const DramaFilterOptions(
    countries: _fallbackCountries,
    genres: _fallbackGenres,
    sorts: _fallbackSorts,
    years: _fallbackYears,
  ),
);
```

`0` is the sentinel for "Older (pre-2020)" — it sorts to the end of the displayed list and triggers client-side post-filtering in `_fetchFilteredPage` (Step 7 below).

- [ ] **Step 4: Update the inline fallback in `build()` to include `years`**

The `build()` method has a fallback `DramaFilterOptions` at line 508-513. Replace it with:

```dart
    final filterOptions = optionsAsync.value ??
        const DramaFilterOptions(
          countries: _fallbackCountries,
          genres: _fallbackGenres,
          sorts: _fallbackSorts,
          years: _fallbackYears,
        );
```

- [ ] **Step 5: Add a `late int? _year;` field and init in `_FilterBottomSheetState`**

In `_FilterBottomSheetState` (line 776-789), add the field and initialize from the incoming filter. The state class should look exactly like this:

```dart
class _FilterBottomSheetState extends State<_FilterBottomSheet> {
  late String _country;
  late String? _genre;
  late String _sort;
  late int? _year;

  @override
  void initState() {
    super.initState();
    _country = widget.filter.country;
    _genre = widget.filter.genre;
    _sort = widget.filter.sort;
    _year = widget.filter.year;
  }
```

- [ ] **Step 6: Add a Year chip wrap between Genre and Country in the bottom sheet**

In `_FilterBottomSheetState.build()` (line 790-967), between the existing Genre `Wrap` (line 871-882) and the Country label (line 886), insert a Year section. Find this exact block:

```dart
          children: genres.map((g) {
            final active = _genre == g;
            return _FilterChipPill(
              label: g,
              active: active,
              onTap: () => setState(() => _genre = active ? null : g),
            );
          }).toList(),
        ),
        const SizedBox(height: 20),

        // Country
        Text(
          'Country',
```

and replace it with:

```dart
          children: genres.map((g) {
            final active = _genre == g;
            return _FilterChipPill(
              label: g,
              active: active,
              onTap: () => setState(() => _genre = active ? null : g),
            );
          }).toList(),
        ),
        const SizedBox(height: 20),

        // Year
        Text(
          'Year',
          style: TextStyle(
            color: AppColors.textSecondary,
            fontSize: 12,
            fontWeight: FontWeight.w600,
            letterSpacing: 0.5,
          ),
        ),
        const SizedBox(height: 10),
        Wrap(
          spacing: 8,
          runSpacing: 8,
          children: widget.options.years.map((y) {
            final active = _year == y;
            final label = y == 0 ? 'Older' : '$y';
            return _FilterChipPill(
              label: label,
              active: active,
              onTap: () => setState(() => _year = active ? null : y),
            );
          }).toList(),
        ),
        const SizedBox(height: 20),

        // Country
        Text(
          'Country',
```

- [ ] **Step 7: Pass `year` through the Apply button**

In the same `build()` method, the `Apply Filters` button calls `widget.onApply` with a new `_DramaFilter`. Find this:

```dart
                      onPressed: () => widget.onApply(
                        _DramaFilter(
                            country: _country, genre: _genre, sort: _sort),
                      ),
```

and replace it with:

```dart
                      onPressed: () => widget.onApply(
                        _DramaFilter(
                            country: _country,
                            genre: _genre,
                            sort: _sort,
                            year: _year),
                      ),
```

- [ ] **Step 8: Update `_filterUrl` to include the year query param**

In `_DramaPageState` (line 340-350), update `_filterUrl`. Replace the existing function with:

```dart
  String _filterUrl(_DramaFilter f, int page) {
    final q = <String, String>{
      'channel_id': '2',
      'per_page': '24',
      'page': '$page',
      'sort': f.sort,
      if (f.country != 'All') 'country': f.country,
      if (f.genre != null) 'genre': f.genre!,
      if (f.year != null && f.year! > 0) 'year': '${f.year}',
    };
    return '/api/moviebox/filter?${q.entries.map((e) => '${Uri.encodeQueryComponent(e.key)}=${Uri.encodeQueryComponent(e.value)}').join('&')}';
  }
```

The `> 0` check excludes the `0` sentinel ("Older") from the upstream call — that case is handled client-side in Step 9.

- [ ] **Step 9: Add client-side "Older" post-filter in `_fetchFilteredPage`**

In `_fetchFilteredPage` (line 390-458), after the MovieBox/Rose merge loop completes (after the line `seenTitles.add(_normalizeTitleKey(title));` for the rose branch, line 425), filter out items whose releaseDate is not older than 2020 when `f.year == 0`. Find this:

```dart
      for (final item in roseRaw) {
        if (item is! Map<String, dynamic>) continue;
        final title = item['title'];
        if (title is! String || title.isEmpty) continue;
        // Skip Rose items already returned by MovieBox to avoid duplicates.
        if (seenTitles.contains(_normalizeTitleKey(title))) continue;
        mapped.add(_mapDramaItem(item, source: 'rose'));
        seenTitles.add(_normalizeTitleKey(title));
      }
      if (!mounted) return;
```

and replace it with:

```dart
      for (final item in roseRaw) {
        if (item is! Map<String, dynamic>) continue;
        final title = item['title'];
        if (title is! String || title.isEmpty) continue;
        // Skip Rose items already returned by MovieBox to avoid duplicates.
        if (seenTitles.contains(_normalizeTitleKey(title))) continue;
        mapped.add(_mapDramaItem(item, source: 'rose'));
        seenTitles.add(_normalizeTitleKey(title));
      }
      // "Older" sentinel: client-side post-filter to pre-2020 titles.
      if (f.year == 0) {
        mapped.removeWhere((m) {
          final rd = m['releaseDate']?.toString() ?? '';
          if (rd.length < 4) return true;
          final y = int.tryParse(rd.substring(0, 4));
          return y == null || y >= 2020;
        });
      }
      if (!mounted) return;
```

- [ ] **Step 10: Run `flutter analyze` and `flutter test`**

Run from `/home/rseann/projects/flutter/hue`:

```bash
flutter analyze lib/features/drama/presentation/pages/drama_page.dart
```

Expected: `No issues found!`

```bash
flutter test
```

Expected: all existing tests pass. (No new tests are added in this plan; the existing 90/90 must remain green.)

If analyze or test fails, **report and stop** — do not proceed.

- [ ] **Step 11: Commit**

```bash
cd /home/rseann/projects/flutter/hue
git add lib/features/drama/presentation/pages/drama_page.dart
git commit -m "feat(drama): add year filter to drama page filter sheet"
```

---

### Task 3: Fix empty-card rendering in `_PosterCard`

**Files:**
- Modify: `lib/features/drama/presentation/pages/drama_page.dart:1270-1338`

**Interfaces:**
- Consumes: existing `_PosterCard` widget, `AppColors` palette
- Produces: a `_PosterCard` whose no-cover fallback shows year + title in a styled block, not a flat colored rectangle

- [ ] **Step 1: Replace `_PosterCard` with the no-cover-aware version**

In `lib/features/drama/presentation/pages/drama_page.dart`, replace the existing `_PosterCard` class (line 1270-1338) with:

```dart
class _PosterCard extends StatelessWidget {
  final Map<String, dynamic> item;
  const _PosterCard({required this.item});

  @override
  Widget build(BuildContext context) {
    final cover = item['cover'] as String?;
    final title = (item['title'] ?? '') as String;
    final id = item['id']?.toString() ?? '';
    final releaseDate = item['releaseDate']?.toString();
    final year = releaseDate != null && releaseDate.length >= 4
        ? int.tryParse(releaseDate.substring(0, 4))
        : null;

    final detailPath = item['detailPath']?.toString() ??
        item['detail_path']?.toString() ??
        id;

    final placeholder = Container(
      color: AppColors.surfaceVariant,
      alignment: Alignment.center,
      padding: const EdgeInsets.all(8),
      child: Column(
        mainAxisAlignment: MainAxisAlignment.center,
        crossAxisAlignment: CrossAxisAlignment.center,
        children: [
          if (year != null)
            Text(
              '$year',
              style: TextStyle(
                color: AppColors.textPrimary,
                fontSize: 22,
                fontWeight: FontWeight.w800,
                height: 1.0,
              ),
            ),
          if (year != null) const SizedBox(height: 6),
          Text(
            title,
            textAlign: TextAlign.center,
            maxLines: 3,
            overflow: TextOverflow.ellipsis,
            style: TextStyle(
              color: AppColors.textPrimary,
              fontSize: 11,
              fontWeight: FontWeight.w600,
              height: 1.2,
            ),
          ),
        ],
      ),
    );

    return RepaintBoundary(
      child: GestureDetector(
        onTap: () => context.push(
          '/detail',
          extra: <String, dynamic>{
            'title': title,
            '_drama_cover': cover,
            '_drama_id': detailPath,
            'description': item['description'],
            'rating': item['rating'],
            'year': year,
            'genres': item['genres'],
          },
        ),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Expanded(
              child: ClipRRect(
                borderRadius: BorderRadius.circular(8),
                child: cover != null
                    ? CachedNetworkImage(
                        imageUrl: cover,
                        fit: BoxFit.cover,
                        width: double.infinity,
                        memCacheWidth: 360,
                        errorWidget: (_, _, _) => placeholder,
                      )
                    : placeholder,
              ),
            ),
            const SizedBox(height: 6),
            SizedBox(
              height: 32,
              child: Text(
                title,
                maxLines: 2,
                overflow: TextOverflow.ellipsis,
                style: TextStyle(
                  color: AppColors.textPrimary,
                  fontSize: 12,
                  fontWeight: FontWeight.w600,
                  height: 1.2,
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}
```

Three things change from the original:
1. A `placeholder` widget (year + title in a colored block) replaces the flat `Container(color: AppColors.surfaceVariant)`. It's used both when `cover == null` and when the `CachedNetworkImage` errors.
2. The title `Text` below the cover is wrapped in a `SizedBox(height: 32)` so it has reserved vertical space and can't collapse to zero.
3. The `releaseDate` parsing and the `_drama_id` `extra` payload reuses the same `year` int (no double-parse).

- [ ] **Step 2: Run `flutter analyze` and `flutter test`**

Run from `/home/rseann/projects/flutter/hue`:

```bash
flutter analyze lib/features/drama/presentation/pages/drama_page.dart
```

Expected: `No issues found!`

```bash
flutter test
```

Expected: all existing tests pass.

If analyze or test fails, **report and stop**.

- [ ] **Step 3: Commit**

```bash
cd /home/rseann/projects/flutter/hue
git add lib/features/drama/presentation/pages/drama_page.dart
git commit -m "fix(drama): show year+title placeholder in _PosterCard when cover is missing"
```

---

### Task 4: Verify the full feature

**Files:** none (verification only)

- [ ] **Step 1: Confirm the project builds**

Run from `/home/rseann/projects/flutter/hue`:

```bash
flutter build apk --debug 2>&1 | tail -10
```

Expected: build succeeds (warnings about other files are fine; no errors).

- [ ] **Step 2: Run `flutter analyze` on the whole project**

```bash
flutter analyze
```

Expected: `No issues found!` (or the same baseline as before the change — 41 pre-existing info-level issues in untouched files are acceptable).

- [ ] **Step 3: Run the full test suite one more time**

```bash
flutter test
```

Expected: all tests pass.

- [ ] **Step 4: Restart hueapi so the new Rust handler takes effect**

```bash
cd /home/rseann/projects/Misc/hue/huerust
pm2 restart hueapi
```

- [ ] **Step 5: Smoke-test the year filter end-to-end**

```bash
TOKEN=$(curl -s http://127.0.0.1:24609/api/auth/token | jq -r .token)
echo "=== year=2025 ==="
curl -s "http://127.0.0.1:24609/api/moviebox/filter?country=Korea&sort=Latest&per_page=3&page=1&channel_id=2&year=2025" \
  -H "Authorization: Bearer $TOKEN" \
  | jq '.items | length'
echo "=== year=All (no year) ==="
curl -s "http://127.0.0.1:24609/api/moviebox/filter?country=Korea&sort=Latest&per_page=3&page=1&channel_id=2" \
  -H "Authorization: Bearer $TOKEN" \
  | jq '.items | length'
```

Expected: both return non-zero counts. The `year=2025` count may be smaller (filter applied) or similar (MovieBox is loose with year). Either is acceptable — the goal is to confirm the param is forwarded.

- [ ] **Step 6: Manual smoke on a real device**

```bash
flutter run -d <your-device>
```

1. Open the Drama page.
2. Tap the filter icon (top right).
3. Confirm the Year section appears between Genre and Country, showing 2026 / 2025 / 2024 / 2023 / 2022 / 2021 / 2020 / Older.
4. Tap "2025," Apply.
5. Confirm the grid renders cards. Some covers may still be flat rectangles — those should now show **year + title** instead of an empty color block.
6. Open the filter again, tap "Older," Apply.
7. Confirm only titles with `releaseDate` year < 2020 appear.
8. Open a rail (e.g. "Korean Drama") to confirm rails still work.

- [ ] **Step 7: Commit any leftover changes (if any)**

```bash
cd /home/rseann/projects/flutter/hue
git status
```

If clean, no commit. If `flutter run` left stale build artifacts, `.gitignore` should cover them. If anything slipped through, commit with `chore(drama): smoke test artifacts` or similar.

---

## Self-review

**1. Spec coverage:**

| Spec section | Plan task |
|---|---|
| Extend `MovieBoxClient::filter()` with `year` | Task 1 Step 3 |
| Remove `#[allow(dead_code)]` on `FilterQuery.year` | Task 1 Step 1 |
| Update `filter_handler` to forward year | Task 1 Step 2 |
| Update `browse_rails` callers | Task 1 Step 4 |
| Add `year` field to `_DramaFilter` | Task 2 Step 1 |
| Add `years` field to `DramaFilterOptions` | Task 2 Step 2 |
| Year chip wrap in bottom sheet | Task 2 Step 6 |
| Wire year into `_filterUrl` | Task 2 Step 8 |
| Client-side "Older" post-filter | Task 2 Step 9 |
| Empty-card fix in `_PosterCard` | Task 3 Step 1 |
| Backend build + test + smoke | Task 1 Steps 5-6 |
| Frontend analyze + test | Task 2 Step 10, Task 3 Step 2 |
| End-to-end manual smoke | Task 4 Steps 4-6 |

**2. Placeholder scan:** No TBDs, no "implement later" steps. Every code step shows full code. No "similar to Task N" copy-paste shortcuts.

**3. Type consistency:**
- Rust: `filter(... , year: Option<&str>)` signature is the same in Task 1 Step 3, called with `None` in Step 4's `browse_rails`, called with `params.year.as_deref()` in Step 2's `filter_handler`. Match.
- Flutter: `_DramaFilter.year: int?` (Step 1), `DramaFilterOptions.years: List<int>` (Step 2), `_DramaFilter.copyWith({int? year})` (Step 1), `_FilterBottomSheetState._year: int?` (Step 5), `widget.onApply(_DramaFilter(..., year: _year))` (Step 7), `_filterUrl` reads `f.year` as `int?` (Step 8), `_fetchFilteredPage` checks `f.year == 0` (Step 9). All match.

**Gaps:** None. All spec requirements covered.
