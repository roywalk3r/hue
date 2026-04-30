# Repository Guidelines

## Project Structure & Module Organization

Hue is a Node.js streaming aggregator API with an optional Rust service.
The main API entry point is `apiServer.js`; `scraperServer.js` runs the scraper-focused service. Provider integrations live in `providers/`, shared helpers in `utils/`, route handlers in `routes/`, and background/service logic in `services/`. Static browser assets and test pages are under `public/`. Standalone utility scripts are in `scripts/`, while the Rust implementation is isolated in `huerust/` with source under `huerust/src/`.

Keep generated logs, cache files, and captured output out of reviews unless they are required evidence.

## Build, Test, and Development Commands

- `npm install` or `pnpm install`: install Node dependencies and Chromium via the `postinstall` hook.
- `npm run start`: run browser setup, then start `apiServer.js`.
- `npm run start:dev`: start the API with `nodemon`, watching `restart.trigger`.
- `npm run start:scraper`: run `scraperServer.js`.
- `npm run start:rust`: run the Rust service from `huerust/` in release mode.
- `npm run lint`: run ESLint across JavaScript files.
- `npm run check:hianime-imports`: validate HiAnime import wiring.
- `cd huerust && cargo test`: run Rust tests when Rust code changes.

Most JavaScript checks are standalone scripts named `test-*.js`; run the relevant file directly, for example `node test-sflix-flow.js`.

## Coding Style & Naming Conventions

Use ES modules and modern JavaScript (`ecmaVersion: 2022`). Follow `.eslintrc.cjs`: prefer `const`, avoid `var`, keep `console` allowed, and treat undefined names as errors. Use two-space indentation in JavaScript unless the surrounding file clearly differs. Name provider files after their upstream service, such as `providers/sflix.js` or `providers/livesport.js`. Keep shared behavior in `utils/` instead of duplicating request, cache, auth, or browser setup code.

## Testing Guidelines

Add or update the nearest `test-*.js` script for provider or scraper changes. Prefer tests that exercise the full provider flow: search, detail lookup, episode selection, and stream extraction. For route or service changes, include a direct API smoke check where practical. Rust changes should pass `cargo test` and, when behavior affects the Node bridge, `npm run start:rust` should be smoke-tested.

## Commit & Pull Request Guidelines

Recent history uses short imperative prefixes such as `feat:` and `chore:`. Keep commits focused and describe why the change exists. This repository also expects lore-style trailers when useful, for example `Confidence: high`, `Scope-risk: narrow`, `Tested: npm run lint`, and `Not-tested: external provider outage paths`.

Pull requests should include a concise summary, affected providers/routes, verification commands, linked issues if any, and screenshots only for `public/` UI changes. Update README or deployment docs when API behavior, environment variables, or runtime commands change.

## Security & Configuration Tips

Do not commit secrets from `.env`, cookies, tokens, or provider credentials. Use `.env.example` for safe placeholders. Be careful with scraper changes that touch headers, cookies, proxies, or browser stealth settings, because they can affect multiple providers.
