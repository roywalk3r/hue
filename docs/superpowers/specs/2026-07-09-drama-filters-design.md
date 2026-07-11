# Drama page filters — design

**Branch:** `fix/drama-filters`
**Date:** 2026-07-09

## Context

The Flutter drama page (`lib/features/drama/presentation/pages/drama_page.dart`) has two related defects:

1. **Empty cards in filtered results** — when the user picks a Country/Genre/Sort filter, the resulting grid renders as a wall of flat colored rectangles. The card structure exists, but the cover image is not loading and the title text is either invisible or not visible enough to read. The user reads this as "empty cards." The rails (Trending, Korean Drama, etc.) are unaffected — only the filter path is broken.
2. **No year filter** — the filter bottom sheet offers Country, Genre, and Sort, but not year. The Rust `FilterQuery` already has a `year` field (line 32 of `routes/moviebox.rs`) marked `#[allow(dead_code)]`, and the Rust `MovieBoxClient::filter()` already accepts the upstream `year` field in its payload (line 822) but hardcodes it to `"All"`. The Node layer (`utils/moviebox-api.js:301-329`) already forwards year. The wiring exists; it's just not connected end-to-end.

## Goals

- Add a Year filter to the drama page filter bottom sheet, wired through to the Rust `MovieBoxClient::filter()` and forwarded to the upstream MovieBox `subject/filter` endpoint.
- Fix the empty-card rendering in `_PosterCard` so the cover-failed case shows a readable, useful placeholder (year + title) instead of a flat rectangle.
- No changes to the rails path (`_dramaPageProvider`) — those work.
- No new routes, no new providers, no new files.

## Architecture

Two-file backend change + one-file frontend change. All changes are additive (a new optional parameter) and additive to the UI (a new chip wrap), so the existing rails and the existing filter behavior are unaffected.

### Backend

**`huerust/src/clients/moviebox.rs:806`** — extend `MovieBoxClient::filter()` signature with `year: Option<&str>`. Replace the hardcoded `"year": "All"` (line 822) with `year.unwrap_or("All")`. The upstream MovieBox endpoint accepts a string ("All" or a 4-digit year like "2025").

**`huerust/src/routes/moviebox.rs:23-34, 295-320`** — remove `#[allow(dead_code)]` on `FilterQuery.year` (line 28-29), extract `let year = params.year.as_deref();` in `filter_handler`, and pass it to `state.moviebox.filter(...)`. No new fields, no new routes.

### Frontend

**`lib/features/drama/presentation/pages/drama_page.dart`** — five localized edits, all in one file:

1. **Extend `_DramaFilter`** (line 46-65): add `final int? year` field. Update `isActive` (line 56) to consider it. Update `copyWith` (line 58-64) to accept it. Default `null` (no year filter).

2. **Extend `DramaFilterOptions`** (line 68-78): add `List<int> years` field. Update the const constructor.

3. **Update `_dramaFilterOptionsProvider`** (line 292-298): supply a default year list. `[2026, 2025, 2024, 2023, 2022, 2021, 2020]` plus a sentinel for "Older" (pre-2020). The sentinel is `0` — `0` is chosen because it's not a valid 4-digit year, so the placeholder meaning is unambiguous. `null` continues to mean "no year filter."

4. **Add a Year chip wrap** to `_FilterBottomSheet.build()` (after Genre, before Country, around line 883): same `Wrap` + `_FilterChipPill` pattern as the existing sections. Tapping a year chip sets `_year`; tapping the active chip clears it back to `null`. The "Older" chip uses `0`; the other chips use the integer year. The `Apply Filters` button (line 941-944) passes `year: _year` through to `_DramaFilter`.

5. **Wire year into `_filterUrl`** (line 340-350): add `if (f.year != null && f.year! > 0) 'year': '${f.year}'`. The `> 0` check excludes the "Older" sentinel because MovieBox's upstream only supports specific 4-digit years (not ranges). When the user picks "Older," we send the year filter to **nothing** and rely on the date range logic below. **For "Older" we instead forward no `year` to the upstream and filter client-side on `releaseDate` year < 2020.** (See "Older" handling below.)

6. **Empty-card fix in `_PosterCard`** (line 1270-1338): when `cover` is null OR `CachedNetworkImage` fails, render a styled placeholder — a colored block with the **year (large, top)** and the **title (medium, bottom)** in centered text. The placeholder uses `AppColors.surfaceVariant` background and `AppColors.textPrimary` foreground. This replaces the current flat `Container(color: AppColors.surfaceVariant)`. Also wrap the cover `Expanded` in a `Flexible` and reserve a minimum 2-line height for the title `Text` so the layout never collapses to zero.

### "Older" handling (the one non-trivial piece)

MovieBox's upstream `year` field accepts `"All"` or a 4-digit year string. It does not accept ranges like `"2010-2019"` or `"<2020"`. To support an "Older" chip, the Flutter side sends `year=All` (or omits `year`) **and** applies a client-side post-filter: items whose `releaseDate` parses to a year < 2020 are kept, others are dropped. This post-filter happens in `_fetchFilteredPage` after the MovieBox response is mapped.

This is the only piece of client-side filtering. The other 6 year chips (2020–2026) use server-side year filtering through MovieBox's native support.

### Data flow

```
User taps year chip in bottom sheet
  → setState _year = 2025
  → Apply Filters
  → _DramaFilter(year: 2025) replaces _filter
  → _resetFiltered() triggered via post-frame
  → _fetchFilteredPage(1)
  → _filterUrl(f, 1) builds "...&year=2025&..."
  → client.getList(_filterUrl(f, 1))
  → Rust /api/moviebox/filter?year=2025
  → state.moviebox.filter(..., Some("2025"))
  → MovieBoxClient::filter includes "year": "2025" in upstream payload
  → Upstream returns filtered items
  → Flutter _fetchFilteredPage maps + dedupes
  → _filteredItems rendered in _PosterCard
```

For "Older" the flow is identical except: `year` is omitted from the URL, and after the response is mapped, items with `releaseDate` year ≥ 2020 are dropped before `setState`.

## Files

- `huerust/src/clients/moviebox.rs` — 1 line: signature + 1 line: payload (function body)
- `huerust/src/routes/moviebox.rs` — 1 line: remove `#[allow(dead_code)]` + 1 line: extract `year` + 1 line: pass to `filter()`
- `lib/features/drama/presentation/pages/drama_page.dart` — 5 edits in one file

## Out of scope

- No new tests (Rust `filter()` has no existing tests; Flutter `_PosterCard` and `_fetchFilteredPage` have no existing tests for filter behavior). Tests would be additive with no current gaps.
- No changes to rails (`_dramaPageProvider`, `_dramaRailSpecs`).
- No changes to `_DramaHero`.
- No changes to the country chip row at the top of the page.
- No new providers.
- No new routes.
- No new files.

## Risks

- **"Older" client-side filter breaks pagination semantics.** If a page of 24 from MovieBox contains 18 items from 2020+, we keep 6. The user sees 6 cards and the "load more" prompt appears immediately. This is acceptable for a single use case; if it becomes a problem, a follow-up could implement a "year range" param in the upstream payload.
- **Cover image URLs may be signed and short-lived.** The empty-card fix doesn't address that — it only makes the no-cover case less ugly. If the underlying cover URL is broken, the user still won't see covers. (Mitigation: the title is now visible in the placeholder, so the user can at least identify titles.)
