# Hue Command Center UX Plan

## Requirements Summary

Hue is currently a Node/Express streaming aggregation backend with a useful but fragmented bundled UI. The product opportunity is to turn it into a premium media command center: one interface for discovery, stream validation, live sports, watch playback, provider health, and operator controls.

The plan must respect the current backend shape:

- `apiServer.js` mounts `/api/streams`, `/api/play`, `/api/moviebox`, `/api/hitv`, `/api/livetv`, docs, static UI, and proxy routes.
- `providers/registry.js` dynamically scans `providers/*.js`, aliases provider names, and exposes `listProviders()` / `getProvider()`.
- `routes/streams.js` owns aggregate TMDB stream lookup.
- `routes/play.js` owns fast smart-play lookup.
- `proxy/proxyServer.js` owns HLS/media proxying, playlist rewriting, and segment caching.
- `routes/system.js` exposes config, cache, status, debug, and password routes.
- Current public UI is split between `public/index.html`, `public/index.js`, `public/config.html`, `public/style.css`, `public/player.html`, and `public/player.js`.

The target UX should be visually ambitious, but it cannot depend on unreliable or unsafe backend behavior. Backend stabilization is phase zero.

## Decision Drivers

- Playback trust matters more than decorative visuals. Users need to know which source is working, which provider found it, and why playback failed.
- The first viewport must be useful immediately. Avoid giant empty hero space; show discovery, live status, and stream controls above the fold.
- Operator controls must be protected. The UI should not expose config mutation or debug data to unauthenticated users.
- The interface should feel like a media control room, not a generic streaming clone.
- Keep the backend API stable enough for an external Nuxt/React frontend later, while improving the bundled UI path now.

## RALPLAN-DR Summary

### Principles

- Security before spectacle: harden config, proxy, and password flows before building UI that depends on them.
- Dense first screen: every viewport should reveal useful content, state, or action.
- Provider transparency: expose provider health, latency, cache status, and fallback behavior to build trust.
- Progressive resilience: every playback path needs fallback, retry, and diagnostic states.
- Repo hygiene: reduce generated artifacts, lockfile drift, and secret leakage before release.

### Viable Options

Option A: Upgrade the existing static `public/` UI.

Pros:
- Lowest migration cost.
- Uses current Express static serving.
- Can ship incrementally with no new frontend framework.

Cons:
- Existing UI is already large, imperative, and hard to evolve.
- Rich interactions will become difficult to maintain.
- Testing and component reuse remain weak.

Option B: Build a new app shell inside `public/` with modern vanilla modules.

Pros:
- Still no build pipeline or new dependency burden.
- Can replace pages gradually.
- Better architecture than current inline/global scripts.

Cons:
- More custom state management.
- Complex player/admin UX will still outgrow vanilla quickly.

Option C: Add a dedicated frontend app that consumes the Hue API.

Pros:
- Best long-term UX architecture.
- Easier component model, routing, tests, and visual polish.
- Can become the primary user-facing product while backend remains API-focused.

Cons:
- Requires build/deploy coordination.
- Larger initial implementation.
- Must solve CORS/auth/API contract carefully.

Recommendation: Option C for the full “crazy UX/UI” product, with a short Option A/B bridge only for urgent fixes to the bundled UI. The backend is already acting as an API platform; a dedicated frontend lets the experience become genuinely premium without turning `public/index.js` into a fragile monolith.

## Architecture Plan

### Phase 0: Stabilize Backend Contracts

Fix high-risk backend issues before the new UI depends on them.

- Fix aggregate stream cache misses in `routes/streams.js`.
- Protect system routes in `routes/system.js` with `requireAuth` from `utils/auth.js`.
- Align password update routes between `public/login.js`, `routes/system.js`, and `utils/auth.js`.
- Add SSRF blocking to `routes/proxy.js` to match the hardened checks in `proxy/proxyServer.js`.
- Resolve `enableProxy` behavior so `/api/play` does not return local proxy URLs when root proxy routes are disabled.
- Replace the hardcoded warmup year in `services/warmupWorker.js` with the current year.
- Decide whether empty provider results should trip the global circuit breaker in `routes/streams.js`.

### Phase 1: Define API Contracts For The UI

Create stable, UI-oriented API contracts without exposing internals.

- `GET /api/status`: provider health, proxy status, worker status, cache summary, current version.
- `GET /api/discover`: composed home payload from MovieBox, HiTV, status, and live sports.
- `GET /api/search`: unified query across MovieBox, HiTV/SFlix, and optional TMDB.
- `GET /api/play/:type/:id`: smart-play response with selected stream plus candidate diagnostics.
- `GET /api/streams/:type/:id`: full provider results for advanced users.
- `GET /api/live`: normalized live sports and event rows.
- `GET /api/admin/config`: protected configuration payload.
- `POST /api/admin/config`: protected config mutation.

The goal is to stop the frontend from stitching together too many provider-specific endpoints.

### Phase 2: Build The Command Center Frontend

Create a dedicated frontend app that consumes the Hue API. Preferred stack:

- Nuxt or Next for routing and SSR-safe player boundaries.
- TypeScript for API payloads.
- A typed fetch layer for `/api/*`.
- Componentized player, provider chips, stream diagnostics, search panels, and admin controls.
- Playwright smoke checks for core routes.

Initial route map:

- `/`: Command Center
- `/discover`: Full discovery browser
- `/streams`: Advanced provider search
- `/watch/:type/:id`: Watch surface
- `/live`: Sports and live events
- `/status`: Provider and system health
- `/admin`: Protected operations console

### Phase 3: First Viewport UX

The first screen should show a useful operational state immediately.

Layout:

- Left: large active title/backdrop with watch action and provider-confidence badge.
- Center/bottom: trending rails visible without scrolling.
- Right: live system strip with provider health, proxy state, cache state, and live sports.
- Top command bar: global search, media type toggle, live indicator, admin/status link.

Avoid:

- A mostly empty hero viewport.
- Marketing copy explaining what the app does.
- Decorative cards inside cards.
- Generic purple/blue gradient styling.

Visual system:

- Graphite/black base with emerald, amber, cyan, and red status accents.
- Compact cards with 8px radius or less.
- Poster/backdrop imagery from live data.
- Motion for scanning, validation, fallback attempts, and live status changes.
- Clear loading skeletons when backend providers are slow.

### Phase 4: Watch Experience

The watch page should make stream reliability visible.

Core elements:

- SSR-safe video player boundary.
- Candidate stream drawer with provider, quality, latency, proxy mode, headers-required state, and validation result.
- “Try next source” action.
- Auto-fallback when a stream fails.
- Subtitle list and language labels.
- Proxy diagnostics: direct URL, m3u8 proxy, ts proxy, and failure reason.
- Debug copy button for a sanitized playback report.

Acceptance target: a user can understand whether playback failed because no provider matched, the source URL is dead, proxy is disabled, headers are missing, or the browser rejected the media.

### Phase 5: Status And Admin UX

Split status from admin.

Status should be readable by operators:

- Provider health grid.
- Circuit breaker state.
- Last successful stream per provider.
- Cache size and expiry.
- Worker heartbeat.
- Proxy enabled/disabled.
- Recent errors with secret redaction.

Admin should be protected and deliberate:

- Config sections grouped by purpose: providers, proxy, quality filters, keys/cookies, cache, workers.
- Dangerous actions require confirmation.
- Secrets are write-only or masked.
- Validation errors are inline and specific.
- Save produces a clear diff preview.

## Acceptance Criteria

- `/api/streams/movie/:id` returns JSON on cache miss instead of throwing.
- Protected config/debug/cache/password endpoints reject unauthenticated requests.
- Password change requires an authenticated session and the current password.
- `/api/proxy?url=` blocks localhost and private network targets.
- The first frontend viewport shows hero, rails, live/system state, and search without large empty regions at desktop and mobile sizes.
- Watch page can show at least three states: loading providers, playable stream, and provider/proxy failure.
- Status page exposes provider health without leaking secrets.
- Admin page can save config only after authentication.
- Build and smoke checks run from documented package scripts.

## Implementation Steps

### 1. Backend Correctness And Security

Files:

- `routes/streams.js`
- `routes/system.js`
- `utils/auth.js`
- `public/login.js`
- `routes/proxy.js`
- `routes/play.js`
- `services/warmupWorker.js`

Tasks:

- Add missing aggregate-handler locals: `streamsResults` and `providerTimings`.
- Apply `requireAuth` to config mutation, cache clearing, debug env, and password routes.
- Add `verifyPassword`-backed password update or a dedicated `changePassword` helper.
- Align frontend password endpoint with backend route.
- Add private IP / localhost blocking to `/api/proxy`.
- Guard `/api/play` proxy wrapping behind `config.enableProxy`.
- Replace hardcoded `2025` with `new Date().getFullYear()`.

### 2. Verification Harness

Files:

- `package.json`
- `scripts/`

Tasks:

- Add `verify:health`.
- Add `verify:auth-routes`.
- Add `verify:proxy-ssrf`.
- Add `verify:streams-cache-miss`.
- Add one combined `verify` script.

### 3. API Contract Layer

Files:

- `routes/discover.js` or equivalent
- `routes/search.js` or equivalent
- `utils/openapi-spec.js`

Tasks:

- Add composed discovery endpoint.
- Add unified search endpoint.
- Extend smart-play response with diagnostics.
- Update OpenAPI docs to match reality.

### 4. Frontend App Shell

Files:

- New dedicated frontend directory or a separate Nuxt/Next app.
- Shared API client and types.

Tasks:

- Create global app shell.
- Add command bar.
- Add provider/status store.
- Add responsive layout primitives.
- Add real route pages.

### 5. Command Center Homepage

Tasks:

- Compose MovieBox, HiTV, status, and live data.
- Build dense first viewport.
- Add discovery rails.
- Add status and live side panels.
- Add empty/error states that preserve layout.

### 6. Watch And Diagnostics

Tasks:

- Build player route.
- Add stream candidate drawer.
- Add auto-fallback.
- Add proxy diagnostics.
- Add playback report export.

### 7. Admin And Status

Tasks:

- Build status dashboard.
- Build protected admin form groups.
- Add config diff preview.
- Mask secrets.
- Add cache/provider controls.

### 8. Visual QA And Browser Verification

Tasks:

- Run desktop and mobile screenshots.
- Check first viewport density.
- Check no text overlap.
- Check player controls fit.
- Check status/admin responsive behavior.

## Risks And Mitigations

- Risk: Provider scraping is inherently unstable.
  Mitigation: show provider health, fallback state, and last successful result.

- Risk: Proxy endpoints can become an open relay.
  Mitigation: keep tokenized URLs, add SSRF blocking everywhere, and avoid exposing raw proxy controls publicly.

- Risk: Admin config leaks secrets.
  Mitigation: mask secrets in reads, protect routes, and avoid rendering raw `.env`-like data.

- Risk: A highly visual UI becomes slow.
  Mitigation: image lazy loading, small rail payloads, skeleton states, and defer player bundles.

- Risk: Multiple package managers keep producing inconsistent installs.
  Mitigation: choose one lockfile and enforce it in docs, Docker, and CI.

## Verification Steps

Backend:

- `npm run lint`
- `npm run verify:health`
- `npm run verify:auth-routes`
- `npm run verify:proxy-ssrf`
- `npm run verify:streams-cache-miss`
- `npm run verify`

Frontend:

- `pnpm build` or framework equivalent.
- Route smoke for `/`, `/discover`, `/streams`, `/watch/movie/603`, `/live`, `/status`, `/admin`.
- Playwright screenshots at 390x844, 768x1024, 1440x900, and 1920x1080.
- Manual playback test with at least one MovieBox-backed title and one fast-provider title.

## Suggested Agent Staffing

- `security-reviewer`: auth, config, proxy SSRF, secret handling.
- `debugger`: `/api/streams` cache-miss crash and `/api/play` proxy-disabled behavior.
- `executor`: backend fixes and verification scripts.
- `designer`: command-center UI system and first viewport.
- `executor`: frontend app shell and route implementation.
- `verifier`: final evidence pass across backend, frontend, and browser screenshots.

## Initial Delivery Slice

The best first milestone is not the full UI. It is a shippable foundation:

- Backend critical fixes.
- Verification scripts.
- New command-center homepage prototype consuming real `/api/status`, `/api/moviebox/home`, and `/api/play`.
- Browser screenshots proving the first viewport is dense, useful, and not blank.

After that, expand to watch diagnostics, live sports, and protected admin.
