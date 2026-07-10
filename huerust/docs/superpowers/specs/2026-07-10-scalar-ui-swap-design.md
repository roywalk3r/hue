# Scalar UI swap for huerust `/api/docs`

## Context

`huerust` currently serves an OpenAPI 3.1 reference at `/api/docs` using
`utoipa-swagger-ui` (`Cargo.toml:78`, mounted at `src/app.rs:820-821`). The
12 hand-written Markdown files (`ARCHITECTURE.md`, `PROVIDERS.md`, `README.md`,
and 8 deep-dives under `docs/`) cover the conceptual layer and remain the
source of truth for everything the OpenAPI spec does not annotate.

You asked to swap the Swagger UI for **Scalar** (scalar.com) — same OpenAPI
spec, nicer browsing experience. This spec is a UI-swap only:

- The `#[utoipa::path]` macros, `ApiDoc` struct, and registered route
  handlers stay as-is.
- The 12 Markdown files, `CLAUDE.md`, and the proxy/auth/runtime behavior
  are out of scope.
- The 403 on `/api/docs/openapi.json` (pre-existing auth bug, separate
  issue) is flagged but not fixed in this change.

## Approach

Two-line dependency swap. `utoipa-scalar = "0.3"` is the official utoipa
adapter; it reuses the same `ApiDoc` struct that utoipa-swagger-ui consumes
today, so no spec-regeneration work is needed. Mounted at the same path
(`/api/docs`) so any bookmarks, runbooks, or upstream caller links keep
working.

```
/api/docs                  → Scalar reference UI (was Swagger UI)
/api/docs/openapi.json     → unchanged; served by utoipa's spec emitter
```

The `Scalar` mount is a single `axum::Router::merge(Scalar::with_url(...))`
call — the same `merge` pattern already used at `src/app.rs:819`. The
`Servable` trait on `Scalar` is the only API surface we touch.

## Files

- **Modify**: `huerust/Cargo.toml` (lines 76-78) — drop
  `utoipa-swagger-ui`, add `utoipa-scalar = "0.3"`.
- **Modify**: `huerust/src/app.rs` (lines 19, 819-822) — swap the
  `use` and the `.merge(SwaggerUi::...)` block for the Scalar equivalent.

No other files change. The `ApiDoc` struct (`src/openapi.rs`) is consumed
by both crates via the `OpenApi` trait, so it needs no edits.

## Plan

1. **Cargo.toml** — replace the `utoipa-swagger-ui` line with
   `utoipa-scalar = "0.3"` (no features needed; the axum mount is core).

2. **app.rs imports** — replace
   ```rust
   use utoipa_swagger_ui::SwaggerUi;
   ```
   with
   ```rust
   use utoipa_scalar::{Scalar, Servable};
   ```

3. **app.rs router mount** — replace the `SwaggerUi` block at lines
   819-822 with:
   ```rust
   .merge(
       Scalar::with_url("/api/docs", crate::openapi::ApiDoc::openapi()),
   )
   ```
   The path argument matches the old path verbatim; the spec argument is
   the same `ApiDoc::openapi()` value Swagger was reading.

4. **Build & verify** — `cargo build` to confirm the new crate
   compiles, `cargo test` to confirm the existing 117 tests still pass
   (none touch the docs UI), then `pm2 restart hueapi` and confirm
   `GET /api/docs/` returns the Scalar HTML (200 with `text/html`,
   title `Scalar API Reference` by default) and `GET /api/docs/openapi.json`
   still returns the JSON spec.

5. **Smoke test** — open `/api/docs` in a browser, confirm the 7
   annotated routes are listed (streams, luna_streams, sse, providers,
   play, sports, auth), the schema panel renders `StreamResult` /
   `Subtitle` / `SportsStream` / `SportsDetailResponse` /
   `AuthTokenResponse`, and "Try it out" issues a request against
   `/api/streams/movie/155` and shows the encrypted-URL response.

## Why this is the right scope

- **No spec rewrite** — `utoipa-scalar` accepts the same `utoipa::OpenApi`
  output. Touching `ApiDoc` to add the other ~80 unannotated routes is
  real work and a separate decision; the current user-facing value is
  "nicer UI for the same data."
- **No Markdown deletion** — the 12 files cover architecture, provider
  scoring, the proxy CDN table, subtitle paths, drama/anime pipelines,
  etc. None of that becomes obsolete when the UI changes. Whether
  Scalar eventually replaces them is a future decision once more routes
  are annotated.
- **Auth bug stays** — `/api/docs/openapi.json` returns 403 when
  `API_KEY` is set because the auth middleware sits above the
  SwaggerUi/Scalar mount. Fixing that is a separate change (one-line
  addition to the middleware's path allowlist at `src/app.rs:848-853`).
  Out of scope here.

## Verification

```bash
# 1. Build
cd huerust && cargo build 2>&1 | tail -5
# expect: Finished `dev` profile

# 2. Tests
cargo test --quiet 2>&1 | tail -5
# expect: 117 passed; 0 failed (baseline from before this change)

# 3. Restart
pm2 restart hueapi

# 4. Scalar UI
curl -s -o /dev/null -w "%{http_code} %{content_type}\n" \
  http://127.0.0.1:24609/api/docs/
# expect: 200 text/html

# 5. Spec still served
curl -s -o /dev/null -w "%{http_code} %{content_type}\n" \
  http://127.0.0.1:24609/api/docs/openapi.json
# expect: 200 application/json  (will still be 403 if API_KEY is set —
# pre-existing bug, not introduced by this change)

# 6. Browser smoke
# open http://127.0.0.1:24609/api/docs, confirm Scalar UI loads with the
# 7 annotated routes
```

## Out of scope

- Adding `#[utoipa::path]` to the unannotated routes (tmdb, drama,
  moviebox, kisskh, anime, etc.) — the spec is intentionally minimal
  today and growing it is a follow-up project.
- Fixing the 403 on `/api/docs/openapi.json` when `API_KEY` is set.
- Removing or rewriting the 12 hand-written Markdown files.
- Customizing Scalar's theme/sidebar — `utoipa-scalar` exposes
  `Scalar::custom_html(html)` for that, but the only knob we need is
  "nice UI for the existing spec." Add later if you want the dark
  theme or a custom logo.
