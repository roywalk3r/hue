# Stage 7: Deployment & DevOps

**Tier:** LOW | **Status:** Complete

## Key Findings

### Docker (Multi-Stage Build)
- Base: `node:20-bookworm-slim` (build + runtime stages)
- Build stage: `npm install --omit=dev` (prod deps only)
- Runtime stage: installs system Chromium via apt-get, ffmpeg, fonts-liberation, ca-certificates
- Skips Playwright browser downloads (`PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD=1`)
- Non-root user `app` created for security
- Port 24608, HTTP healthcheck to `/api/health` (interval 30s, timeout 5s, start_period 20s)
- OCI labels applied

### Docker Compose (4 services)
| Service | Port | Description |
|---|---|---|
| tmdb-embed-api | 24608 | Main Node.js API, `user_config_data` volume |
| huerust | 24609 | Rust streaming engine, depends on redis |
| scraper | 24610 | Browser automation microservice |
| redis | 6379 | Alpine Redis, cache backend |

All: `restart: unless-stopped`, healthchecks, shared network.

### PM2 (ecosystem.config.js) — STALE
- `hue-rust-api`: `cargo run --release` in `./huerust`, `RUST_PORT: 24608`
- `hue-scraper-microservice`: `scraperServer.js`, `SCRAPER_PORT`
- **Port conflict: PM2 assigns 24608 to Rust; Docker assigns 24608 to Node, 24609 to Rust**
- PM2 is development/legacy; Docker Compose is primary production orchestration

### Vercel (INCOMPATIBLE for full use)
- `vercel.json`: routes all to `apiServer.js` via `@vercel/node`
- `NODE_ENV=production` hardcoded
- **Critical limitations:** No Chromium, 10–60s execution timeout, no Redis, 250MB bundle limit, stateless
- Only lightweight stateless API endpoints would function correctly

### GitHub Actions (6 workflow files)

**docker-publish.yml:**
- Triggers: push to main/master, git tags (v*), manual dispatch
- Multi-arch: linux/amd64 + linux/arm64 via Docker Buildx
- Tags: `latest`, version from `package.json`, git tag
- Auth: DOCKERHUB_USERNAME + DOCKERHUB_TOKEN secrets

**5 Gemini AI Workflows:**
| Workflow | Purpose |
|---|---|
| gemini-dispatch.yml | Routes PR/issue events to Gemini handlers |
| gemini-review.yml | Automated PR code review (7-min timeout) |
| gemini-triage.yml | Auto-classify/label issues (prompt injection protection) |
| gemini-invoke.yml | General Gemini CLI invocation (25 max turns) |
| gemini-scheduled-triage.yml | Hourly cron for unlabeled issues |

All use GCP service account + Workload Identity, GitHub App tokens, and GitHub MCP server.

**Gemini is CI/CD only — not a runtime dependency.**

### Key Environment Variables
| Variable | Purpose | Default |
|---|---|---|
| TMDB_API_KEY | TMDB auth | required |
| APP_SECRET | Session + token signing | `hue-streaming-stable-secret-2026` |
| RUST_PROXY_URL | Rust engine address | `http://localhost:24609` |
| MICROSERVICE_URL | Scraper address | `http://scraper:24610` |
| CACHE_BACKEND | Cache type | `redis` |
| REDIS_URL | Redis connection | `redis://redis:6379` |
| SFLIX_COOKIE | SFlix JWT | optional (hardcoded fallback expired) |
| MOVIEBOX_X_USER | MovieBox JWT | optional (hardcoded fallback expired) |
| BROWSER_EXECUTABLE_PATH | Chromium path | `/usr/bin/chromium` (Docker) |

Config persistence: `/app/data/user-config.json` via Docker volume.

### Monitoring
- Healthchecks on all 4 services
- `/api/health` and `/health` endpoints
- `/api/metrics` referenced but Prometheus not fully configured
- No APM, no distributed tracing, no centralized logging beyond Docker/Vercel logs

### Scaling
- Horizontal: possible at API tier via Docker
- State: all in-process (cache, circuit breaker) — no coordination between instances
- No auto-scaling or load balancer in Compose config
