# Hue Premium Consumer Frontend Plan

## Product Direction

Hue should become a premium hosted streaming frontend for regular viewers, not an operator command center. The backend remains the stream aggregation engine; the frontend becomes the product people actually use: cinematic discovery, fast search, polished playback, live sports, and a saved list.

The core promise:

> Open Hue, find something worth watching in seconds, press play, and recover gracefully when a source fails.

Admin, provider diagnostics, config mutation, debug env, cache controls, and internal health details must stay out of the public experience. They belong in a private admin area or backend-only tooling.

## Target Audience

- Viewers who want movies, series, drama, and live sports in one polished interface.
- Users who do not know or care what MovieBox, HiTV, Vixsrc, proxy routes, provider registries, or circuit breakers are.
- Mobile-first users who browse visually and expect quick playback.
- Power users who want filters, subtitles, quality choice, and a reliable watch history without seeing backend machinery.

## Product Pillars

- Cinematic discovery: real posters, backdrops, carousels, genre lanes, trending rows, and regional drama collections.
- Fast intent capture: search is always available and supports title, person, sport, and direct ID-style lookup where useful.
- Reliable playback: stream fallback is automatic and user-friendly.
- Personal library: saved titles, continue watching, recently viewed, and watchlist.
- Live moments: live sports and events get a dedicated surface with clear live/upcoming state.
- Premium feel: fewer visible technical details, stronger art direction, polished motion, and responsive layouts.

## What This Is Not

- Not an admin dashboard.
- Not a provider status console.
- Not a developer API explorer.
- Not a blank hero with one title floating in a dark void.
- Not a clone of the existing Bootstrap UI.

## App Route Map

- `/`: Home, personalized discovery, trending, continue watching, live-now strip.
- `/movies`: Movie browsing with genre, country, year, quality, and popularity filters.
- `/series`: Series browsing with season-aware cards.
- `/drama`: KDrama, CDrama, Thai, Turkish, Bollywood, and regional categories.
- `/live`: Live sports and events with live/upcoming filters.
- `/search`: Full-screen search experience with instant categories and recent searches.
- `/watch/:type/:id`: Playback page with source fallback hidden behind a clean user flow.
- `/title/:type/:id`: Detail page with overview, cast-style metadata when available, seasons, related titles, and play/save actions.
- `/saved`: Watchlist, continue watching, and history.
- `/settings`: User preferences such as subtitle language, autoplay, quality preference, theme, and content filters.
- `/admin`: Private operator surface, visually separate from the consumer app.

## First Viewport

The first viewport should feel like a premium streaming product and also be dense enough to use immediately.

Desktop composition:

- Top navigation with brand, search, movies, series, drama, live, saved.
- Large cinematic feature area with real backdrop/poster and clear play/save actions.
- Right-side or lower-side compact rail for live-now and continue-watching.
- A visible row of trending titles before the fold ends.
- No huge empty vertical space.

Mobile composition:

- Sticky top search and brand.
- Feature card with backdrop, title, play, save.
- Horizontal quick chips: Movies, Series, Drama, Live, Search.
- Continue watching and trending rails visible quickly.

## Home Experience

Home should combine backend discovery sources into viewer-friendly lanes:

- Continue Watching
- Trending Now
- New This Week
- Movies People Are Watching
- Series To Binge
- KDrama Spotlight
- CDrama And Asian Drama
- Live Sports Now
- Short And Mini Series
- Because You Saved

If personalization is not ready, use local storage for a first version:

- saved titles
- recent titles
- preferred language
- preferred media type
- last watched stream position where feasible

## Browse And Search

Browsing should feel fast and visual:

- Filter drawer for genre, country, year, media type, source category, and sort.
- Search suggestions grouped by Movies, Series, Drama, Live, and Recent.
- Empty states should recommend adjacent content instead of saying “no results” only.
- Loading states should use poster skeletons, not spinners as the main experience.

## Watch Experience

The watch page must be simple for normal users:

- Big player, title, season/episode selector, subtitles, quality preference, next episode.
- If a stream fails, show “Trying another source” and recover automatically.
- Show a friendly error only after fallback attempts are exhausted.
- Keep provider names out of the primary UI.
- Advanced diagnostics can live behind a “Playback details” drawer for support.

Playback states:

- Preparing stream
- Checking sources
- Playing
- Trying another source
- Source unavailable
- Proxy or browser playback issue

## Live Experience

Live should feel like a schedule and a broadcast surface:

- Live now, starting soon, and replay/ended groups.
- Sport filters.
- Team/match search.
- One-click watch.
- Clear “live” and “upcoming” labels.
- Fallback source handling similar to movies/series.

## Visual Direction

The app should feel premium, cinematic, and usable:

- Use real artwork as the primary visual signal.
- Use a dark base with rich contrast, but avoid a one-note dark blue/slate look.
- Use warm gold, sharp cyan, signal green, and restrained red for status/action.
- Use compact 8px-radius cards for repeated title items.
- Avoid nested cards, decorative blobs, and generic gradient hero art.
- Use motion for page entrance, rail reveals, search transitions, and playback fallback.
- Typography should feel editorial and premium, not default Bootstrap/system-only.

Suggested UI feel:

- Brand: Hue
- Tone: cinematic, fast, confident, subtle
- Inspiration: premium streaming product plus boutique festival guide, not admin SaaS

## API Boundary

The frontend should not stitch together raw provider internals. Create or stabilize viewer-oriented endpoints:

- `GET /api/discover/home`
- `GET /api/discover/movies`
- `GET /api/discover/series`
- `GET /api/discover/drama`
- `GET /api/search?q=...`
- `GET /api/title/:type/:id`
- `GET /api/play/:type/:id`
- `GET /api/live`
- `GET /api/me/library` or local-storage equivalent for MVP

The frontend can use existing routes at first, but the target should be a clean public API layer.

## Required Backend Fixes Before Launch

- Fix `/api/streams/:type/:tmdbId` cache-miss crash.
- Protect config/debug/cache/password routes.
- Fix password-change route and current-password validation.
- Add SSRF protection to `/api/proxy`.
- Remove or rotate tracked secrets.
- Align package manager and Node version.
- Fix `/api/play` behavior when proxy is disabled.
- Replace hardcoded warmup year with current year.

## Implementation Strategy

Recommended build:

- Dedicated Nuxt 3 + TypeScript + Nitro frontend app hosted separately from the backend.
- Frontend hosted at the main domain or `app.example.com`.
- Hue backend hosted as a long-lived service at `api.example.com`, not as a serverless function.
- Optional stream/proxy domain at `stream.example.com` if bandwidth isolation becomes important.
- Nuxt server routes act as a thin BFF layer, exposing stable consumer endpoints such as `/v1/home`, `/v1/search`, `/v1/title/:id`, `/v1/play`, and `/v1/live`.
- The BFF calls the existing backend routes and normalizes provider-shaped payloads into viewer-facing data.
- Keep admin hosted separately at `admin.example.com` or behind a private route that never appears in the consumer shell.
- Launch MVP with anonymous/local session state for saved titles and continue watching; add real consumer accounts later with a separate auth provider.
- Do not reuse the current file-backed admin auth model for consumer identity.

Deployment topology:

- `app.example.com`: Nuxt frontend on Vercel, Netlify, Cloudflare, or similar.
- `api.example.com`: Hue backend on VPS, Fly, Render, or Docker host with persistent process support.
- `admin.example.com`: locked operator surface.
- `stream.example.com`: optional proxy/media domain.

Avoid hosting the current backend on Vercel for production playback. It has long-lived browser automation, proxying, cache, worker, and stream behavior that fits a persistent server better than single-function serverless constraints.

MVP phases:

1. Backend hardening and public API cleanup.
2. Frontend shell, theme, navigation, and data client.
3. Home, browse, search, and title detail.
4. Watch page with fallback-aware playback.
5. Live sports surface.
6. Saved/continue watching.
7. Admin isolation and deployment hardening.
8. Browser QA and launch polish.

## Acceptance Criteria

- A first-time viewer can open `/`, search or browse, and start a title without understanding providers.
- The first viewport has artwork, search, play action, and at least one content rail visible.
- Mobile home, search, and watch pages are fully usable.
- Playback failure triggers fallback before showing an error.
- Public users cannot access config, debug, cache mutation, or secret-like data.
- The frontend never exposes raw provider internals in normal UI.
- The app can be hosted as a public product with clear frontend/backend deployment boundaries.

## Verification Plan

Backend:

- Health check.
- Stream cache-miss check.
- Auth route protection check.
- Proxy SSRF check.
- Smart-play check for one known title.

Frontend:

- Build check.
- Route smoke for `/`, `/movies`, `/series`, `/drama`, `/live`, `/search`, `/saved`, `/watch/movie/603`.
- Playwright screenshots at mobile, tablet, desktop, and wide desktop.
- Visual QA for first viewport density.
- Playback QA for success, fallback, and exhausted-failure states.

## First Milestone

Build a consumer-facing prototype that proves the product direction:

- Home page with premium artwork-first layout.
- Real data from MovieBox/HiTV/status where available.
- Search entry and browse rails.
- Watch page using `/api/play`.
- Local saved list.
- No admin or provider dashboard on the public surface.

This milestone should look like something viewers would want to use, even before every backend source is perfect.
