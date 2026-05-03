# Hue Stream API

Professional streaming aggregator API. Rust-powered with Node.js browser automation.

## Architecture

- **Rust API (huerust)** — Primary API server, HTTP providers, proxy, caching, discovery
- **Node Scraper** — Internal service for Playwright-based providers, cookie rotation, anime scraping
- **Redis** — Shared cache and state

## Quick Start

```bash
# Start all services
docker compose up -d

# Or run individually
cd huerust && cargo run --release    # Rust API on :24609
cd node-scraper && npm start         # Node scraper on :24610
```

## API Documentation

- Scalar Docs: http://localhost:24609/api/docs
- OpenAPI Spec: http://localhost:24609/api/docs/openapi.json

## Endpoints

### Stream Aggregation
```http
GET /api/streams/movie/{tmdbId}
GET /api/streams/tv/{tmdbId}?season=1&episode=1
```

### Discovery
```http
GET /api/hitv/browse?sort=Hottest&genre=Romance
GET /api/moviebox/hero
GET /api/anime/home
```

### Smart Play
```http
GET /api/play/movie/{tmdbId}
GET /api/play/tv/{tmdbId}?season=1&episode=1
```

### System
```http
GET /health
GET /metrics
GET /api/providers
```

## Contributing

1. Fork the Project
2. Create your Feature Branch (`git checkout -b feature/AmazingFeature`)
3. Commit your Changes (`git commit -m 'Add some AmazingFeature'`)
4. Push to the Branch (`git push origin feature/AmazingFeature`)
5. Open a Pull Request

## License

Distributed under the MIT License. See `LICENSE` for more information.
