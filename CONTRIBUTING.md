# Contributing to Hue Stream API

## Getting Started

```bash
git clone https://github.com/roywalk3r/hue.git
cd hue
docker compose up -d
```

The Rust API serves on `:24609`, Node scraper on `:24610` (internal).

## Architecture

- **huerust/** — Rust API server (public endpoints, HTTP providers, proxy, caching)
- **node-scraper/** — Node.js scraping service (Playwright providers, cookie rotation, anime)
- **providers/** — Node.js provider registry (browser-based providers only)

## Adding a Provider

### HTTP-based (Rust)
1. Implement `StreamProvider` trait in `huerust/src/providers/{name}.rs`
2. Register in `huerust/src/main.rs`
3. Add tests

### Browser-based (Node)
1. Add provider to `node-scraper/src/providers/{name}.js`
2. Export a scrape function
3. Register in `node-scraper/src/routes/internal.js`

## Commit Style

Use short imperative prefixes: `feat:`, `fix:`, `chore:`, `docs:`, `refactor:`, `test:`

## Testing

```bash
# Rust
cd huerust && cargo test

# Node contract tests
./tests/contract/test-node-scraper-api.sh

# Lint
npm run lint
```
