# Repository Guidelines

## Project Structure

- `huerust/` — Rust API server (primary public API)
- `node-scraper/` — Node.js internal scraping service
- `providers/` — Legacy Node.js providers (Playwright-based)
- `docker-compose.yml` — Full stack deployment

## Build & Run

```bash
# Rust API
cd huerust && cargo run --release

# Node Scraper
cd node-scraper && npm start

# Full stack
docker compose up -d
```

## Testing

```bash
# Rust unit tests
cd huerust && cargo test

# Node scraper contract tests
./tests/contract/test-node-scraper-api.sh

# Node lint
npm run lint
```

## Architecture Rules

- Rust is the public API — all client-facing endpoints go there
- Node is internal only — exposes `/internal/*` endpoints on 127.0.0.1:24610
- No circular dependencies — Node never calls Rust
- Shared state goes through Redis
- New HTTP-based providers go in Rust (huerust/src/providers/)
- New browser-based providers go in Node (node-scraper/src/providers/)

## Adding a New Provider

### HTTP-based (Rust)
1. Create `huerust/src/providers/{name}.rs` implementing the `StreamProvider` trait
2. Register it in `huerust/src/main.rs`
3. Add tests

### Browser-based (Node)
1. Create `node-scraper/src/providers/{name}.js`
2. Export a scrape function matching the registry contract
3. Add to the provider map in `node-scraper/src/routes/internal.js`
4. Add tests
