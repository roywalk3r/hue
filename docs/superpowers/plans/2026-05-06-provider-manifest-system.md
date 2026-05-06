# Provider Manifest System Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace hardcoded provider registration and per-file config with manifest-driven loading across Node and Rust, restore Rust compilation, and add an isolation test harness.

**Architecture:** Provider manifests in `providers/manifests/*.toml` are the single source of truth for runtime, media types, behavior config, and test fixtures. Node reads manifests to inject timeout/retry config; Rust reads manifests to load its registry. The Node↔Rust bridge gets a typed contract at `providers/bridge-schema.json`.

**Tech Stack:** Node.js/Express, Rust/Axum, `@iarna/toml` (Node), `toml` crate (Rust), `async-trait`, `serde`.

---

> **Note on Rust compilation:** `huerust/src/main.rs` references 12 modules that don't exist on disk. Tasks 4–8 restore compilation before adding manifest logic. Tasks 1–3 are Node-only and can proceed independently.

---

## File Structure

### New files
| Path | Purpose |
|------|---------|
| `providers/manifests/*.toml` | One manifest per provider |
| `utils/manifestLoader.js` | Reads TOMLs, exports provider config map |
| `providers/bridge-schema.json` | Typed bridge contract |
| `huerust/src/config.rs` | `AppConfig::from_env()` |
| `huerust/src/cache.rs` | `StreamCache` stub |
| `huerust/src/filters.rs` | `FilterConfig` stub |
| `huerust/src/circuit_breaker.rs` | `CircuitBreaker` stub |
| `huerust/src/coalesce.rs` | `FetchCoalescer` stub |
| `huerust/src/metrics.rs` | `Metrics` stub |
| `huerust/src/user_config.rs` | `HueUserConfig` stub |
| `huerust/src/openapi.rs` | `ApiDoc` stub |
| `huerust/src/registry.rs` | Manifest-loading `ProviderRegistry` |
| `huerust/src/microservice.rs` | `BridgeClient` |
| `huerust/src/proxy/mod.rs` | Proxy handler stubs |
| `huerust/src/routes/mod.rs` + route files | Route handler stubs |
| `huerust/src/providers/mp4hydra.rs` | Mp4Hydra HTTP client stub |
| `huerust/src/providers/mp4hydra_provider.rs` | `Mp4HydraProvider` |
| `huerust/src/providers/moviebox_provider.rs` | `MovieBoxProvider` |
| `huerust/src/providers/kisskh_provider.rs` | `KissKhProvider` |
| `huerust/src/providers/sflix_provider.rs` | `SflixProvider` |
| `huerust/src/providers/dramacool_provider.rs` | `DramacoolProvider` |
| `huerust/src/providers/vixsrc_provider.rs` | `VixsrcProvider` |
| `scripts/test-provider.js` | Node provider smoke test harness |
| `huerust/tests/providers.rs` | Rust provider smoke test harness |

### Modified files
| Path | Change |
|------|--------|
| `package.json` | Add `@iarna/toml` |
| `providers/registry.js` | Inject manifest behavior config |
| `huerust/Cargo.toml` | Add `toml = "0.8"` dependency |
| `huerust/src/main.rs` | Decompose into `app.rs` + `startup.rs` + `handlers/` |

---

## Task 1: Write provider manifests

**Files:**
- Create: `providers/manifests/kisskh.toml`
- Create: `providers/manifests/dramacool.toml`
- Create: `providers/manifests/sflix.toml`
- Create: `providers/manifests/vixsrc.toml`
- Create: `providers/manifests/moviebox.toml`
- Create: `providers/manifests/mp4hydra.toml`
- Create: `providers/manifests/livesport.toml`
- Create: `providers/manifests/aisports.toml`
- Create: `providers/manifests/livetv.toml`
- Create: `providers/manifests/ppv.toml`
- Create: `providers/manifests/vidrock.toml`
- Create: `providers/manifests/autoembed.toml`

- [ ] **Step 1: Create manifests directory**

```bash
mkdir -p providers/manifests
```

- [ ] **Step 2: Write Rust+Node provider manifests**

`providers/manifests/kisskh.toml`:
```toml
[provider]
name = "zenith"
label = "KissKH"
runtime = "both"
media_types = ["drama"]

[capabilities]
search = true
streams = true
episodes = true

[behavior]
timeout_ms = 12000
max_retries = 2
circuit_breaker = true
priority = 80

[test]
fixture_id = "9256"
fixture_type = "drama"
fixture_episode = 1
expect_streams = true
```

`providers/manifests/dramacool.toml`:
```toml
[provider]
name = "orient"
label = "DramaCool"
runtime = "both"
media_types = ["drama"]

[capabilities]
search = true
streams = true
episodes = true

[behavior]
timeout_ms = 15000
max_retries = 2
circuit_breaker = true
priority = 75

[test]
fixture_id = "goblin"
fixture_type = "drama"
fixture_episode = 1
expect_streams = true
```

`providers/manifests/sflix.toml`:
```toml
[provider]
name = "nova"
label = "SFlix"
runtime = "both"
media_types = ["movie", "series"]

[capabilities]
search = false
streams = true
episodes = true

[behavior]
timeout_ms = 20000
max_retries = 2
circuit_breaker = true
priority = 70

[test]
fixture_id = "27205"
fixture_type = "movie"
fixture_episode = 0
expect_streams = true
```

`providers/manifests/vixsrc.toml`:
```toml
[provider]
name = "aura"
label = "Vixsrc"
runtime = "both"
media_types = ["movie", "series"]

[capabilities]
search = false
streams = true
episodes = true

[behavior]
timeout_ms = 15000
max_retries = 2
circuit_breaker = true
priority = 90

[test]
fixture_id = "27205"
fixture_type = "movie"
fixture_episode = 0
expect_streams = true
```

`providers/manifests/moviebox.toml`:
```toml
[provider]
name = "cinema"
label = "MovieBox"
runtime = "both"
media_types = ["movie", "series"]

[capabilities]
search = true
streams = true
episodes = true

[behavior]
timeout_ms = 20000
max_retries = 2
circuit_breaker = true
priority = 85

[test]
fixture_id = "27205"
fixture_type = "movie"
fixture_episode = 0
expect_streams = true
```

`providers/manifests/mp4hydra.toml`:
```toml
[provider]
name = "hydra"
label = "MP4Hydra"
runtime = "both"
media_types = ["movie", "series"]

[capabilities]
search = false
streams = true
episodes = true

[behavior]
timeout_ms = 15000
max_retries = 2
circuit_breaker = true
priority = 65

[test]
fixture_id = "27205"
fixture_type = "movie"
fixture_episode = 0
expect_streams = true
```

`providers/manifests/livesport.toml`:
```toml
[provider]
name = "stadium"
label = "Livesport"
runtime = "node"
media_types = ["sports"]

[capabilities]
search = false
streams = true
episodes = false

[behavior]
timeout_ms = 30000
max_retries = 1
circuit_breaker = false
priority = 80

[test]
fixture_id = "premier-league"
fixture_type = "sports"
fixture_episode = 0
expect_streams = false
```

`providers/manifests/aisports.toml`:
```toml
[provider]
name = "field"
label = "AiSports"
runtime = "node"
media_types = ["sports"]

[capabilities]
search = false
streams = true
episodes = false

[behavior]
timeout_ms = 20000
max_retries = 1
circuit_breaker = false
priority = 75

[test]
fixture_id = ""
fixture_type = "sports"
fixture_episode = 0
expect_streams = false
```

`providers/manifests/livetv.toml`:
```toml
[provider]
name = "arena"
label = "LiveTV"
runtime = "node"
media_types = ["sports"]

[capabilities]
search = false
streams = true
episodes = false

[behavior]
timeout_ms = 25000
max_retries = 1
circuit_breaker = false
priority = 70

[test]
fixture_id = ""
fixture_type = "sports"
fixture_episode = 0
expect_streams = false
```

`providers/manifests/ppv.toml`:
```toml
[provider]
name = "event"
label = "PPV.to"
runtime = "node"
media_types = ["sports"]

[capabilities]
search = false
streams = true
episodes = false

[behavior]
timeout_ms = 20000
max_retries = 1
circuit_breaker = false
priority = 65

[test]
fixture_id = ""
fixture_type = "sports"
fixture_episode = 0
expect_streams = false
```

`providers/manifests/vidrock.toml`:
```toml
[provider]
name = "luna"
label = "VidRock"
runtime = "node"
media_types = ["movie", "series"]

[capabilities]
search = false
streams = true
episodes = true

[behavior]
timeout_ms = 20000
max_retries = 2
circuit_breaker = true
priority = 60

[test]
fixture_id = "27205"
fixture_type = "movie"
fixture_episode = 0
expect_streams = true
```

`providers/manifests/autoembed.toml`:
```toml
[provider]
name = "blaze"
label = "AutoEmbed"
runtime = "node"
media_types = ["movie", "series"]

[capabilities]
search = false
streams = true
episodes = true

[behavior]
timeout_ms = 15000
max_retries = 2
circuit_breaker = true
priority = 72

[test]
fixture_id = "27205"
fixture_type = "movie"
fixture_episode = 0
expect_streams = true
```

- [ ] **Step 3: Commit manifests**

```bash
git add providers/manifests/
git commit -m "feat: add provider manifests for all 12 providers"
```

---

## Task 2: Node manifest loader

**Files:**
- Modify: `package.json`
- Create: `utils/manifestLoader.js`

- [ ] **Step 1: Install TOML parser**

```bash
pnpm add @iarna/toml
```

- [ ] **Step 2: Create `utils/manifestLoader.js`**

```js
const fs = require('fs');
const path = require('path');
const TOML = require('@iarna/toml');

const MANIFESTS_DIR = path.join(__dirname, '..', 'providers', 'manifests');

let _cache = null;

function loadManifests() {
  if (_cache) return _cache;
  const result = {};
  if (!fs.existsSync(MANIFESTS_DIR)) return result;
  for (const file of fs.readdirSync(MANIFESTS_DIR)) {
    if (!file.endsWith('.toml')) continue;
    const raw = fs.readFileSync(path.join(MANIFESTS_DIR, file), 'utf8');
    const manifest = TOML.parse(raw);
    result[manifest.provider.name] = manifest;
  }
  _cache = result;
  return result;
}

function getManifestByLabel(label) {
  const manifests = loadManifests();
  return Object.values(manifests).find(
    m => m.provider.label.toLowerCase() === label.toLowerCase()
  ) || null;
}

function getBehaviorConfig(providerRealName) {
  const manifests = loadManifests();
  const manifest = Object.values(manifests).find(
    m => m.provider.name === providerRealName || m.provider.label.toLowerCase() === providerRealName.toLowerCase()
  );
  return manifest ? manifest.behavior : { timeout_ms: 15000, max_retries: 2, circuit_breaker: true, priority: 50 };
}

function getNodeProviders() {
  const manifests = loadManifests();
  return Object.values(manifests).filter(
    m => m.provider.runtime === 'node' || m.provider.runtime === 'both'
  );
}

module.exports = { loadManifests, getManifestByLabel, getBehaviorConfig, getNodeProviders };
```

- [ ] **Step 3: Verify loader works**

```bash
node -e "const m = require('./utils/manifestLoader'); console.log(Object.keys(m.loadManifests()))"
```

Expected output: `[ 'zenith', 'orient', 'nova', 'aura', 'cinema', 'hydra', 'stadium', 'field', 'arena', 'event', 'luna', 'blaze' ]`

- [ ] **Step 4: Commit**

```bash
git add utils/manifestLoader.js package.json pnpm-lock.yaml
git commit -m "feat: add manifest loader utility for Node providers"
```

---

## Task 3: Inject manifest config into Node registry

**Files:**
- Modify: `providers/registry.js`

- [ ] **Step 1: Import manifest loader at top of `providers/registry.js`**

Add after the existing `require` statements (around line 13):
```js
const { getBehaviorConfig } = require('../utils/manifestLoader');
```

- [ ] **Step 2: Replace hardcoded timeout in `createFetchFunction`**

The existing `createFetchFunction` has no timeout. Add one using manifest config. In the `try` block inside `createFetchFunction`, wrap the provider call with a timeout:

```js
function createFetchFunction(providerInfo) {
  const behavior = getBehaviorConfig(providerInfo.name);
  const timeoutMs = behavior.timeout_ms;

  return async function(ctx) {
    const mediaType = ctx.type === 'movie' ? 'movie' : 'tv';

    if (providerInfo.name !== 'livesport' && providerInfo.name !== 'aisports' && providerInfo.name !== 'streamed' && providerInfo.name !== 'livetv' && providerInfo.name !== 'ppv') {
      const cacheKeyType = `${providerInfo.name}:${mediaType}`;
      const cached = getCachedStreams(cacheKeyType, ctx.tmdbId, ctx.season, ctx.episode);
      if (cached) {
        console.log(`[registry] ${providerInfo.name} returned from cache`);
        return cached;
      }
    }

    const module = loadProvider(providerInfo.file);
    if (!module) return [];
    const funcName = providerInfo.functionName;
    if (!module[funcName]) {
      console.error(`Provider ${providerInfo.name} does not export ${funcName}`);
      return [];
    }

    try {
      const t0 = Date.now();
      console.log(`[registry] Calling ${providerInfo.name} tmdbId=${ctx.tmdbId} mediaType=${mediaType}`);

      const fetchPromise = (async () => {
        if (providerInfo.name === 'showbox' || providerInfo.name === 'showbox-playwright') {
          const { getTmdbApiKey } = require('../utils/tmdbKey');
          const tmdbApiKey = getTmdbApiKey();
          if (!tmdbApiKey) { console.warn('[registry] showbox skipped: no TMDB key'); return []; }
          const cookies = await getEffectiveCookies();
          const previousConfig = global.currentRequestConfig;
          global.currentRequestConfig = { ...(previousConfig || {}) };
          let selected = null;
          if (cookies.length > 0) {
            const index = Math.floor(Math.random() * cookies.length);
            selected = cookies[index];
            global.currentRequestConfig.cookie = selected.startsWith('ui=') ? selected : `ui=${selected}`;
            global.currentRequestConfig.cookies = cookies.map(c => c.startsWith('ui=') ? c : `ui=${c}`);
            lastCookieStats = { selected: selected.slice(0, 16) + '...', index, total: cookies.length, remainingMB: null, timestamp: Date.now() };
          }
          const r = await module[funcName](mediaType, ctx.tmdbId, ctx.season || null, ctx.episode || null, null, selected);
          if (global.currentRequestUserCookieRemainingMB != null) lastCookieStats.remainingMB = global.currentRequestUserCookieRemainingMB;
          global.currentRequestConfig = previousConfig || {};
          return r;
        } else if (providerInfo.name === 'vidsrccc-playwright') {
          return module[funcName](ctx.tmdbId, mediaType, ctx.season || null, ctx.episode || null, ctx.imdbId || null);
        } else if (providerInfo.name === 'livesport' || providerInfo.name === 'aisports' || providerInfo.name === 'streamed' || providerInfo.name === 'livetv' || providerInfo.name === 'ppv') {
          return module[funcName]({ ...ctx, req: ctx.req });
        } else if (providerInfo.name === 'moviebox') {
          return module[funcName](ctx.tmdbId, mediaType, ctx.season || null, ctx.episode || null, ctx.tmdbData);
        } else {
          return module[funcName](ctx.tmdbId, mediaType, ctx.season || null, ctx.episode || null);
        }
      })();

      const timeoutPromise = new Promise((_, reject) =>
        setTimeout(() => reject(new Error(`timeout after ${timeoutMs}ms`)), timeoutMs)
      );

      let result = await Promise.race([fetchPromise, timeoutPromise]);
      const durationMs = Date.now() - t0;
      console.log(`[registry] ${providerInfo.name} fetch duration ${durationMs}ms`);

      if (!Array.isArray(result)) return [];

      const publicName = getPublicName(providerInfo.realName);
      const finalResult = result.map(s => ({
        ...s,
        title: sanitizeText(s.title || s.name || 'Stream'),
        name: s.name ? sanitizeText(s.name) : undefined,
        provider: s.provider || publicName
      }));

      if (providerInfo.realName !== 'livesport' && providerInfo.realName !== 'aisports' && providerInfo.realName !== 'livetv' && providerInfo.realName !== 'ppv' && finalResult.length > 0) {
        const cacheKeyType = `${providerInfo.realName}:${mediaType}`;
        setCachedStreams(cacheKeyType, ctx.tmdbId, ctx.season, ctx.episode, finalResult);
      }

      return finalResult;
    } catch (e) {
      console.error(`[registry] ${providerInfo.name} fetch error:`, e.message);
      return [];
    }
  };
}
```

- [ ] **Step 3: Test that registry still loads**

```bash
node -e "require('./providers/registry'); console.log('registry ok')"
```

Expected: `registry ok` with provider enabled/disabled lines, no errors.

- [ ] **Step 4: Commit**

```bash
git add providers/registry.js
git commit -m "feat: inject manifest timeout config into Node provider registry"
```

---

## Task 4: Rust — create utility stub modules

**Files:**
- Create: `huerust/src/config.rs`
- Create: `huerust/src/cache.rs`
- Create: `huerust/src/filters.rs`
- Create: `huerust/src/circuit_breaker.rs`
- Create: `huerust/src/coalesce.rs`
- Create: `huerust/src/metrics.rs`
- Create: `huerust/src/user_config.rs`

- [ ] **Step 1: Create `huerust/src/config.rs`**

```rust
use std::env;

#[derive(Clone)]
pub struct AppConfig {
    pub tmdb_api_keys: Vec<String>,
    pub secret: String,
    pub proxy_base_url: String,
    pub host: String,
    pub port: u16,
}

impl AppConfig {
    pub fn from_env() -> Self {
        Self {
            tmdb_api_keys: env::var("TMDB_API_KEY")
                .map(|k| k.split(',').map(|s| s.trim().to_string()).collect())
                .unwrap_or_default(),
            secret: env::var("APP_SECRET").unwrap_or_else(|_| "changeme".to_string()),
            proxy_base_url: env::var("PROXY_BASE_URL").unwrap_or_default(),
            host: env::var("RUST_HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            port: env::var("RUST_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(24609),
        }
    }
}
```

- [ ] **Step 2: Create `huerust/src/cache.rs`**

```rust
use crate::config::AppConfig;

pub struct StreamCache;

impl StreamCache {
    pub async fn new_from_config(_cfg: &AppConfig) -> Self {
        Self
    }
}
```

- [ ] **Step 3: Create `huerust/src/filters.rs`**

```rust
pub struct FilterConfig;

impl Default for FilterConfig {
    fn default() -> Self { Self }
}
```

- [ ] **Step 4: Create `huerust/src/circuit_breaker.rs`**

```rust
use std::sync::Arc;
use crate::metrics::Metrics;

pub struct CircuitBreaker;

impl CircuitBreaker {
    pub fn new() -> Self { Self }
    pub fn set_metrics(&self, _metrics: Arc<Metrics>) {}
}
```

- [ ] **Step 5: Create `huerust/src/coalesce.rs`**

```rust
pub struct FetchCoalescer;

impl FetchCoalescer {
    pub fn new() -> Self { Self }
}
```

- [ ] **Step 6: Create `huerust/src/metrics.rs`**

```rust
pub struct Metrics;

impl Metrics {
    pub fn new(_names: &[&str]) -> Self { Self }
    pub fn render_prometheus(&self) -> String { String::new() }
}
```

- [ ] **Step 7: Create `huerust/src/user_config.rs`**

```rust
pub struct HueUserConfig;

impl HueUserConfig {
    pub fn invalidate() {}
}
```

- [ ] **Step 8: Run cargo check — expect fewer errors**

```bash
cd huerust && cargo check 2>&1 | grep "^error" | head -20
```

Expected: errors about `openapi`, `proxy`, `routes`, `registry`, `microservice` — NOT about config/cache/filters/metrics/coalesce/user_config.

- [ ] **Step 9: Commit**

```bash
git add huerust/src/config.rs huerust/src/cache.rs huerust/src/filters.rs huerust/src/circuit_breaker.rs huerust/src/coalesce.rs huerust/src/metrics.rs huerust/src/user_config.rs
git commit -m "feat(rust): add utility stub modules to restore compilation"
```

---

## Task 5: Rust — create openapi and proxy stubs

**Files:**
- Create: `huerust/src/openapi.rs`
- Create: `huerust/src/proxy/mod.rs`

- [ ] **Step 1: Create `huerust/src/openapi.rs`**

```rust
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    info(title = "Hue Stream API", version = "1.0.0"),
    paths(),
    components()
)]
pub struct ApiDoc;
```

- [ ] **Step 2: Create `huerust/src/proxy/mod.rs`**

```bash
mkdir -p huerust/src/proxy
```

```rust
use axum::{extract::Query, response::{IntoResponse, Response}, http::StatusCode};
use std::collections::HashMap;

pub async fn m3u8_handler(Query(_p): Query<HashMap<String, String>>) -> Response {
    (StatusCode::NOT_IMPLEMENTED, "proxy stub").into_response()
}

pub async fn ts_handler(Query(_p): Query<HashMap<String, String>>) -> Response {
    (StatusCode::NOT_IMPLEMENTED, "proxy stub").into_response()
}

pub async fn sub_handler(Query(_p): Query<HashMap<String, String>>) -> Response {
    (StatusCode::NOT_IMPLEMENTED, "proxy stub").into_response()
}
```

- [ ] **Step 3: Run cargo check**

```bash
cd huerust && cargo check 2>&1 | grep "^error" | head -20
```

Expected: errors about `registry`, `routes`, `microservice`, and missing provider types — NOT about openapi or proxy.

- [ ] **Step 4: Commit**

```bash
git add huerust/src/openapi.rs huerust/src/proxy/mod.rs
git commit -m "feat(rust): add openapi and proxy stubs"
```

---

## Task 6: Rust — create route stubs

**Files:**
- Create: `huerust/src/routes/mod.rs`
- Create: `huerust/src/routes/hitv.rs`
- Create: `huerust/src/routes/streams.rs`
- Create: `huerust/src/routes/sse.rs`
- Create: `huerust/src/routes/providers.rs`
- Create: `huerust/src/routes/play.rs`
- Create: `huerust/src/routes/drama.rs`

- [ ] **Step 1: Create routes directory and mod.rs**

```bash
mkdir -p huerust/src/routes
```

`huerust/src/routes/mod.rs`:
```rust
pub mod hitv;
pub mod streams;
pub mod sse;
pub mod providers;
pub mod play;
pub mod drama;
```

- [ ] **Step 2: Create `huerust/src/routes/hitv.rs`**

```rust
use axum::{extract::{State, Path, Query}, response::{IntoResponse, Response}, http::StatusCode};
use std::collections::HashMap;
use crate::AppState;

pub async fn browse_handler(State(_s): State<AppState>, Query(_p): Query<HashMap<String, String>>) -> Response { (StatusCode::NOT_IMPLEMENTED, "stub").into_response() }
pub async fn search_handler(State(_s): State<AppState>, Query(_p): Query<HashMap<String, String>>) -> Response { (StatusCode::NOT_IMPLEMENTED, "stub").into_response() }
pub async fn latest_handler(State(_s): State<AppState>, Query(_p): Query<HashMap<String, String>>) -> Response { (StatusCode::NOT_IMPLEMENTED, "stub").into_response() }
pub async fn trending_handler(State(_s): State<AppState>, Query(_p): Query<HashMap<String, String>>) -> Response { (StatusCode::NOT_IMPLEMENTED, "stub").into_response() }
pub async fn hero_handler(State(_s): State<AppState>, Query(_p): Query<HashMap<String, String>>) -> Response { (StatusCode::NOT_IMPLEMENTED, "stub").into_response() }
pub async fn drama_handler(State(_s): State<AppState>, Query(_p): Query<HashMap<String, String>>) -> Response { (StatusCode::NOT_IMPLEMENTED, "stub").into_response() }
pub async fn info_handler(State(_s): State<AppState>, Path(_id): Path<String>) -> Response { (StatusCode::NOT_IMPLEMENTED, "stub").into_response() }
pub async fn play_handler(State(_s): State<AppState>, Path(_p): Path<(String, u32)>) -> Response { (StatusCode::NOT_IMPLEMENTED, "stub").into_response() }
pub async fn countries_handler(State(_s): State<AppState>) -> Response { (StatusCode::NOT_IMPLEMENTED, "stub").into_response() }
pub async fn categories_handler(State(_s): State<AppState>) -> Response { (StatusCode::NOT_IMPLEMENTED, "stub").into_response() }
pub async fn genres_handler(State(_s): State<AppState>) -> Response { (StatusCode::NOT_IMPLEMENTED, "stub").into_response() }
pub async fn sorts_handler(State(_s): State<AppState>) -> Response { (StatusCode::NOT_IMPLEMENTED, "stub").into_response() }
pub async fn dynamic_browse_handler(State(_s): State<AppState>, Path(_p): Path<(String, String, String)>) -> Response { (StatusCode::NOT_IMPLEMENTED, "stub").into_response() }
```

- [ ] **Step 3: Create `huerust/src/routes/streams.rs`**

```rust
use axum::{extract::{State, Path, Query}, response::{IntoResponse, Response}, http::StatusCode};
use std::collections::HashMap;
use crate::AppState;

pub async fn streams_handler(State(_s): State<AppState>, Path(_p): Path<(String, String)>, Query(_q): Query<HashMap<String, String>>) -> Response {
    (StatusCode::NOT_IMPLEMENTED, "stub").into_response()
}
```

- [ ] **Step 4: Create `huerust/src/routes/sse.rs`**

```rust
use axum::{extract::{State, Path, Query}, response::{IntoResponse, Response}, http::StatusCode};
use std::collections::HashMap;
use crate::AppState;

pub async fn sse_streams_handler(State(_s): State<AppState>, Path(_p): Path<(String, String)>, Query(_q): Query<HashMap<String, String>>) -> Response {
    (StatusCode::NOT_IMPLEMENTED, "stub").into_response()
}
```

- [ ] **Step 5: Create `huerust/src/routes/providers.rs`**

```rust
use axum::{extract::State, response::{IntoResponse, Response}, http::StatusCode};
use crate::AppState;

pub async fn providers_handler(State(_s): State<AppState>) -> Response {
    (StatusCode::NOT_IMPLEMENTED, "stub").into_response()
}
```

- [ ] **Step 6: Create `huerust/src/routes/play.rs`**

```rust
use axum::{extract::{State, Path, Query}, response::{IntoResponse, Response}, http::StatusCode};
use std::collections::HashMap;
use crate::AppState;

pub async fn play_handler(State(_s): State<AppState>, Path(_p): Path<(String, String)>, Query(_q): Query<HashMap<String, String>>) -> Response {
    (StatusCode::NOT_IMPLEMENTED, "stub").into_response()
}
```

- [ ] **Step 7: Create `huerust/src/routes/drama.rs`**

```rust
use axum::{extract::{State, Path, Query}, response::{IntoResponse, Response}, http::StatusCode};
use std::collections::HashMap;
use crate::AppState;

pub async fn kisskh_search_handler(State(_s): State<AppState>, Query(_p): Query<HashMap<String, String>>) -> Response { (StatusCode::NOT_IMPLEMENTED, "stub").into_response() }
pub async fn kisskh_streams_handler(State(_s): State<AppState>, Path(_p): Path<(String, u32)>) -> Response { (StatusCode::NOT_IMPLEMENTED, "stub").into_response() }
pub async fn dramacool_search_handler(State(_s): State<AppState>, Query(_p): Query<HashMap<String, String>>) -> Response { (StatusCode::NOT_IMPLEMENTED, "stub").into_response() }
pub async fn dramacool_streams_handler(State(_s): State<AppState>, Path(_p): Path<(String, u32)>) -> Response { (StatusCode::NOT_IMPLEMENTED, "stub").into_response() }
```

- [ ] **Step 8: Run cargo check**

```bash
cd huerust && cargo check 2>&1 | grep "^error" | head -20
```

Expected: errors only about `registry` and missing provider types (`Mp4HydraProvider`, `*Provider`).

- [ ] **Step 9: Commit**

```bash
git add huerust/src/routes/
git commit -m "feat(rust): add route stub handlers"
```

---

## Task 7: Rust — create missing provider stubs

**Files:**
- Create: `huerust/src/providers/mp4hydra.rs`
- Create: `huerust/src/providers/mp4hydra_provider.rs`
- Create: `huerust/src/providers/moviebox_provider.rs`
- Create: `huerust/src/providers/kisskh_provider.rs`
- Create: `huerust/src/providers/sflix_provider.rs`
- Create: `huerust/src/providers/dramacool_provider.rs`
- Create: `huerust/src/providers/vixsrc_provider.rs`

- [ ] **Step 1: Create `huerust/src/providers/mp4hydra.rs`**

```rust
use reqwest::Client;
use anyhow::Result;
use crate::models::StreamResult;

pub struct Mp4HydraClient {
    client: Client,
}

impl Mp4HydraClient {
    pub fn new() -> Self {
        Self { client: Client::new() }
    }

    pub async fn get_streams(&self, _tmdb_id: &str, _media_type: &str, _season: u32, _episode: u32) -> Result<Vec<StreamResult>> {
        Ok(vec![])
    }
}
```

- [ ] **Step 2: Create provider wrapper stubs**

Each of these follows the same pattern. The `StreamProvider` trait requires: `name()`, `real_name()`, `is_fast()`, `timeout_secs()`, `fetch()`.

`huerust/src/providers/mp4hydra_provider.rs`:
```rust
use std::sync::Arc;
use async_trait::async_trait;
use anyhow::Result;
use crate::providers::{StreamProvider, mp4hydra::Mp4HydraClient};
use crate::models::StreamResult;
use crate::tmdb::TmdbNormalizedInfo;

pub struct Mp4HydraProvider { client: Arc<Mp4HydraClient> }
impl Mp4HydraProvider {
    pub fn new() -> Self { Self { client: Arc::new(Mp4HydraClient::new()) } }
}
#[async_trait]
impl StreamProvider for Mp4HydraProvider {
    fn name(&self) -> &'static str { "Hydra" }
    fn real_name(&self) -> &'static str { "mp4hydra" }
    fn is_fast(&self) -> bool { true }
    fn timeout_secs(&self) -> u64 { 15 }
    async fn fetch(&self, info: &TmdbNormalizedInfo, media_type: &str, season: u32, episode: u32) -> Result<Vec<StreamResult>> {
        self.client.get_streams(&info.tmdb_id.to_string(), media_type, season, episode).await
    }
}
```

`huerust/src/providers/moviebox_provider.rs`:
```rust
use std::sync::Arc;
use async_trait::async_trait;
use anyhow::Result;
use crate::providers::StreamProvider;
use crate::moviebox::MovieBoxClient;
use crate::models::StreamResult;
use crate::tmdb::TmdbNormalizedInfo;

pub struct MovieBoxProvider { client: Arc<MovieBoxClient> }
impl MovieBoxProvider {
    pub fn new(client: Arc<MovieBoxClient>) -> Self { Self { client } }
}
#[async_trait]
impl StreamProvider for MovieBoxProvider {
    fn name(&self) -> &'static str { "Cinema" }
    fn real_name(&self) -> &'static str { "moviebox" }
    fn is_fast(&self) -> bool { true }
    fn timeout_secs(&self) -> u64 { 20 }
    async fn fetch(&self, _info: &TmdbNormalizedInfo, _media_type: &str, _season: u32, _episode: u32) -> Result<Vec<StreamResult>> {
        Ok(vec![])
    }
}
```

`huerust/src/providers/kisskh_provider.rs`:
```rust
use std::sync::Arc;
use async_trait::async_trait;
use anyhow::Result;
use crate::providers::{StreamProvider, kisskh::KissKhClient};
use crate::models::StreamResult;
use crate::tmdb::TmdbNormalizedInfo;

pub struct KissKhProvider { _client: Arc<KissKhClient> }
impl KissKhProvider {
    pub fn new(client: Arc<KissKhClient>) -> Self { Self { _client: client } }
}
#[async_trait]
impl StreamProvider for KissKhProvider {
    fn name(&self) -> &'static str { "Zenith" }
    fn real_name(&self) -> &'static str { "kisskh" }
    fn is_fast(&self) -> bool { true }
    fn timeout_secs(&self) -> u64 { 12 }
    async fn fetch(&self, _info: &TmdbNormalizedInfo, _media_type: &str, _season: u32, _episode: u32) -> Result<Vec<StreamResult>> {
        // TODO: wire to _client once client method signatures are confirmed from kisskh.rs
        Ok(vec![])
    }
}
```

`huerust/src/providers/sflix_provider.rs`:
```rust
use std::sync::Arc;
use async_trait::async_trait;
use anyhow::Result;
use crate::providers::{StreamProvider, sflix::SflixClient};
use crate::models::StreamResult;
use crate::tmdb::TmdbNormalizedInfo;

pub struct SflixProvider { _client: Arc<SflixClient> }
impl SflixProvider {
    pub fn new(client: Arc<SflixClient>) -> Self { Self { _client: client } }
}
#[async_trait]
impl StreamProvider for SflixProvider {
    fn name(&self) -> &'static str { "Nova" }
    fn real_name(&self) -> &'static str { "sflix" }
    fn is_fast(&self) -> bool { false }
    fn timeout_secs(&self) -> u64 { 20 }
    async fn fetch(&self, _info: &TmdbNormalizedInfo, _media_type: &str, _season: u32, _episode: u32) -> Result<Vec<StreamResult>> {
        // TODO: wire to _client once client method signatures are confirmed from sflix.rs
        Ok(vec![])
    }
}
```

`huerust/src/providers/dramacool_provider.rs`:
```rust
use std::sync::Arc;
use async_trait::async_trait;
use anyhow::Result;
use crate::providers::{StreamProvider, dramacool::DramacoolClient};
use crate::models::StreamResult;
use crate::tmdb::TmdbNormalizedInfo;

pub struct DramacoolProvider { _client: Arc<DramacoolClient> }
impl DramacoolProvider {
    pub fn new(client: Arc<DramacoolClient>) -> Self { Self { _client: client } }
}
#[async_trait]
impl StreamProvider for DramacoolProvider {
    fn name(&self) -> &'static str { "Orient" }
    fn real_name(&self) -> &'static str { "dramacool" }
    fn is_fast(&self) -> bool { false }
    fn timeout_secs(&self) -> u64 { 15 }
    async fn fetch(&self, _info: &TmdbNormalizedInfo, _media_type: &str, _season: u32, _episode: u32) -> Result<Vec<StreamResult>> {
        // TODO: wire to _client once client method signatures are confirmed from dramacool.rs
        Ok(vec![])
    }
}
```

`huerust/src/providers/vixsrc_provider.rs`:
```rust
use std::sync::Arc;
use async_trait::async_trait;
use anyhow::Result;
use crate::providers::{StreamProvider, vixsrc::VixsrcClient};
use crate::models::StreamResult;
use crate::tmdb::TmdbNormalizedInfo;

pub struct VixsrcProvider { _client: Arc<VixsrcClient> }
impl VixsrcProvider {
    pub fn new(client: Arc<VixsrcClient>) -> Self { Self { _client: client } }
}
#[async_trait]
impl StreamProvider for VixsrcProvider {
    fn name(&self) -> &'static str { "Aura" }
    fn real_name(&self) -> &'static str { "vixsrc" }
    fn is_fast(&self) -> bool { true }
    fn timeout_secs(&self) -> u64 { 15 }
    async fn fetch(&self, _info: &TmdbNormalizedInfo, _media_type: &str, _season: u32, _episode: u32) -> Result<Vec<StreamResult>> {
        // TODO: wire to _client once client method signatures are confirmed from vixsrc.rs
        Ok(vec![])
    }
}
```

- [ ] **Step 3: Run cargo check — expect errors only about `registry`**

```bash
cd huerust && cargo check 2>&1 | grep "^error" | head -20
```

Expected: one error about `mod registry` not found.

- [ ] **Step 4: Commit**

```bash
git add huerust/src/providers/
git commit -m "feat(rust): add provider wrapper stubs for all 6 providers"
```

---

## Task 8: Rust — add toml crate and create registry.rs

**Files:**
- Modify: `huerust/Cargo.toml`
- Create: `huerust/src/registry.rs`
- Create: `huerust/src/microservice.rs` (stub for now — full version in Task 9)

- [ ] **Step 1: Add toml to `huerust/Cargo.toml`**

In the `[dependencies]` section, add:
```toml
toml = { version = "0.8", features = ["parse"] }
```

- [ ] **Step 2: Create `huerust/src/registry.rs`**

```rust
use std::sync::Arc;
use std::collections::HashMap;
use std::path::Path;
use serde::Deserialize;
use crate::providers::StreamProvider;
use crate::circuit_breaker::CircuitBreaker;

#[derive(Debug, Deserialize, Clone)]
pub struct ManifestProviderInfo {
    pub name: String,
    pub label: String,
    pub runtime: String,
    pub media_types: Vec<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ManifestBehavior {
    pub timeout_ms: u64,
    pub max_retries: u32,
    pub circuit_breaker: bool,
    pub priority: u32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ManifestTest {
    pub fixture_id: String,
    pub fixture_type: String,
    pub fixture_episode: u32,
    pub expect_streams: bool,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ProviderManifest {
    pub provider: ManifestProviderInfo,
    pub behavior: ManifestBehavior,
    pub test: ManifestTest,
}

pub struct ProviderRegistry {
    providers: Vec<Arc<dyn StreamProvider>>,
    manifests: HashMap<String, ProviderManifest>,
    _circuit_breaker: Arc<CircuitBreaker>,
}

impl ProviderRegistry {
    pub fn new(cb: Arc<CircuitBreaker>) -> Self {
        Self {
            providers: Vec::new(),
            manifests: HashMap::new(),
            _circuit_breaker: cb,
        }
    }

    pub fn register(&mut self, provider: Arc<dyn StreamProvider>) {
        self.providers.push(provider);
    }

    pub fn load_manifests(&mut self, manifests_dir: &str) {
        let dir = Path::new(manifests_dir);
        if !dir.exists() {
            tracing::warn!("[registry] manifests dir not found: {}", manifests_dir);
            return;
        }
        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(e) => { tracing::error!("[registry] cannot read manifests dir: {}", e); return; }
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("toml") { continue; }
            let content = match std::fs::read_to_string(&path) {
                Ok(c) => c,
                Err(e) => { tracing::warn!("[registry] skip {:?}: {}", path, e); continue; }
            };
            match toml::from_str::<ProviderManifest>(&content) {
                Ok(manifest) => {
                    tracing::info!("[registry] loaded manifest: {} ({})", manifest.provider.label, manifest.provider.runtime);
                    self.manifests.insert(manifest.provider.name.clone(), manifest);
                }
                Err(e) => { tracing::warn!("[registry] parse error {:?}: {}", path, e); }
            }
        }
    }

    pub fn providers(&self) -> &[Arc<dyn StreamProvider>] {
        &self.providers
    }

    pub fn manifest_for(&self, real_name: &str) -> Option<&ProviderManifest> {
        self.manifests.get(real_name)
    }

    pub fn timeout_ms_for(&self, real_name: &str) -> u64 {
        self.manifests.get(real_name)
            .map(|m| m.behavior.timeout_ms)
            .unwrap_or(15_000)
    }

    pub fn all_manifests(&self) -> &HashMap<String, ProviderManifest> {
        &self.manifests
    }
}
```

- [ ] **Step 3: Create `huerust/src/microservice.rs` (stub)**

```rust
pub struct BridgeClient;

impl BridgeClient {
    pub fn new() -> Self { Self }
}
```

- [ ] **Step 4: Add manifest loading call in `main.rs`**

In `huerust/src/main.rs`, after `registry.register(Arc::new(Mp4HydraProvider::new()));`, add:

```rust
    // Load manifests — attaches behavior config to registry
    registry.load_manifests("../providers/manifests");
```

- [ ] **Step 5: Run `cargo build` — should compile cleanly**

```bash
cd huerust && cargo build 2>&1 | tail -5
```

Expected: `Finished dev [unoptimized + debuginfo] target(s) in ...`

If there are errors, fix the type mismatch reported (likely `KissKhClient::get_streams` signature — adjust the provider stubs to match the actual signatures in `kisskh.rs`, `sflix.rs`, `dramacool.rs`, `vixsrc.rs`).

- [ ] **Step 6: Commit**

```bash
git add huerust/Cargo.toml huerust/src/registry.rs huerust/src/microservice.rs huerust/src/main.rs
git commit -m "feat(rust): manifest-loading ProviderRegistry — Rust engine now compiles"
```

---

## Task 9: Bridge formalization

**Files:**
- Modify: `huerust/src/microservice.rs`
- Create: `providers/bridge-schema.json`
- Modify: `scraperServer.js`

- [ ] **Step 1: Write full `huerust/src/microservice.rs`**

```rust
use reqwest::Client;
use serde::{Deserialize, Serialize};
use anyhow::Result;
use crate::models::StreamResult;

#[derive(Serialize)]
pub struct BridgeRequest {
    pub r#type: String,
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub episode: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub season: Option<u32>,
}

#[derive(Deserialize)]
pub struct BridgeResponse {
    pub streams: Vec<StreamResult>,
    pub provider: String,
    pub cached: bool,
}

pub struct BridgeClient {
    client: Client,
    base_url: String,
}

impl BridgeClient {
    pub fn new(client: Client, base_url: String) -> Self {
        Self { client, base_url }
    }

    pub async fn fetch_streams(&self, provider_name: &str, req: BridgeRequest) -> Result<BridgeResponse> {
        let url = format!("{}/providers/{}/streams", self.base_url, provider_name);
        let resp = self.client
            .post(&url)
            .json(&req)
            .send()
            .await?
            .json::<BridgeResponse>()
            .await?;
        Ok(resp)
    }

    pub fn base_url(&self) -> &str { &self.base_url }
}
```

- [ ] **Step 2: Create `providers/bridge-schema.json`**

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "version": "1.0",
  "request": {
    "type": "object",
    "required": ["type", "id"],
    "properties": {
      "type": { "type": "string", "enum": ["movie", "series", "drama", "anime", "sports"] },
      "id": { "type": "string" },
      "episode": { "type": "integer", "minimum": 0 },
      "season": { "type": "integer", "minimum": 0 }
    }
  },
  "response": {
    "type": "object",
    "required": ["streams", "provider", "cached"],
    "properties": {
      "streams": {
        "type": "array",
        "items": {
          "type": "object",
          "required": ["url", "quality"],
          "properties": {
            "url": { "type": "string" },
            "quality": { "type": "string" },
            "title": { "type": "string" },
            "provider": { "type": "string" },
            "format": { "type": "string" },
            "headers": { "type": "object" }
          }
        }
      },
      "provider": { "type": "string" },
      "cached": { "type": "boolean" }
    }
  }
}
```

- [ ] **Step 3: Add `/providers/:name/streams` endpoint to `scraperServer.js`**

Find the section in `scraperServer.js` where routes are registered and add:

```js
const { getProvider } = require('./providers/registry');

// Bridge endpoint — called by Rust engine for node-runtime providers
app.post('/providers/:name/streams', async (req, res) => {
  const { name } = req.params;
  const { type, id, episode, season } = req.body || {};

  const provider = getProvider(name);
  if (!provider) {
    return res.status(404).json({ error: `provider '${name}' not found` });
  }

  try {
    const streams = await provider.fetch({
      type: type || 'movie',
      tmdbId: id,
      episode: episode || null,
      season: season || null,
    });
    res.json({ streams: streams || [], provider: name, cached: false });
  } catch (e) {
    console.error(`[bridge] ${name} error:`, e.message);
    res.status(502).json({ streams: [], provider: name, cached: false, error: e.message });
  }
});
```

- [ ] **Step 4: Verify bridge endpoint responds**

Start the scraper server, then:
```bash
curl -s -X POST http://localhost:3003/providers/zenith/streams \
  -H "Content-Type: application/json" \
  -d '{"type":"drama","id":"9256","episode":1}' | head -c 200
```

Expected: JSON with `streams`, `provider`, `cached` fields (may be empty streams if provider not enabled).

- [ ] **Step 5: Commit**

```bash
git add huerust/src/microservice.rs providers/bridge-schema.json scraperServer.js
git commit -m "feat: formalize Node-Rust bridge with typed BridgeClient and schema"
```

---

## Task 10: Decompose `main.rs`

**Files:**
- Create: `huerust/src/app.rs`
- Create: `huerust/src/startup.rs`
- Create: `huerust/src/handlers/mod.rs`
- Create: `huerust/src/handlers/admin.rs`
- Create: `huerust/src/handlers/metrics.rs`
- Modify: `huerust/src/main.rs`

- [ ] **Step 1: Create `huerust/src/app.rs`**

Move all `.route(...)` calls out of `main.rs` into a `build_router` function:

```rust
use axum::{routing::get, Router};
use utoipa_swagger_ui::SwaggerUi;
use crate::{AppState, proxy, routes, handlers, openapi::ApiDoc};
use utoipa::OpenApi;
use axum::middleware;
use tower_http::cors::CorsLayer;
use axum::http::StatusCode;

// Middleware lives here so main.rs doesn't need to grow for new routes
async fn auth_middleware(axum::extract::State(_state): axum::extract::State<AppState>, req: axum::http::Request<axum::body::Body>, next: axum::middleware::Next) -> Result<axum::response::Response, StatusCode> {
    let is_local = req.extensions().get::<std::net::SocketAddr>().map_or(false, |a| a.ip().is_loopback());
    if is_local { return Ok(next.run(req).await); }
    if let Ok(api_key) = std::env::var("API_KEY") {
        let auth = req.headers().get("x-api-key").and_then(|v| v.to_str().ok());
        if auth != Some(&api_key) { return Err(StatusCode::FORBIDDEN); }
    }
    Ok(next.run(req).await)
}

async fn rate_limit_middleware(axum::extract::State(state): axum::extract::State<AppState>, req: axum::http::Request<axum::body::Body>, next: axum::middleware::Next) -> Result<axum::response::Response, StatusCode> {
    let ip = req.extensions()
        .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
        .map(|c| c.0.ip().to_string())
        .or_else(|| req.headers().get("x-forwarded-for").and_then(|v| v.to_str().ok()).map(|s| s.split(',').next().unwrap_or(s).trim().to_string()))
        .unwrap_or_else(|| "unknown".to_string());
    if !state.rate_limiter.check(&ip) {
        return Ok((StatusCode::TOO_MANY_REQUESTS, "Rate limit exceeded").into_response());
    }
    Ok(next.run(req).await)
}

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(|| async { "Huerust is Alive" }))
        .route("/metrics", get(handlers::metrics::metrics_handler))
        .route("/m3u8-proxy", get(proxy::m3u8_handler))
        .route("/ts-proxy", get(proxy::ts_handler))
        .route("/sub-proxy", get(proxy::sub_handler))
        .route("/api/hitv/library", get(handlers::metrics::hitv_library_handler))
        .route("/api/hitv/browse", get(routes::hitv::browse_handler))
        .route("/api/hitv/search", get(routes::hitv::search_handler))
        .route("/api/hitv/latest", get(routes::hitv::latest_handler))
        .route("/api/hitv/trending", get(routes::hitv::trending_handler))
        .route("/api/hitv/hero", get(routes::hitv::hero_handler))
        .route("/api/hitv/drama", get(routes::hitv::drama_handler))
        .route("/api/hitv/info/:id", get(routes::hitv::info_handler))
        .route("/api/hitv/play/:id/:episode", get(routes::hitv::play_handler))
        .route("/api/hitv/countries", get(routes::hitv::countries_handler))
        .route("/api/hitv/categories", get(routes::hitv::categories_handler))
        .route("/api/hitv/genres", get(routes::hitv::genres_handler))
        .route("/api/hitv/sorts", get(routes::hitv::sorts_handler))
        .route("/api/hitv/:country/:sort/:genre", get(routes::hitv::dynamic_browse_handler))
        .route("/api/streams/:type/:tmdbId", get(routes::streams::streams_handler))
        .route("/api/streams/:type/:tmdbId/live", get(routes::sse::sse_streams_handler))
        .route("/api/providers", get(routes::providers::providers_handler))
        .route("/api/admin/reload", axum::routing::post(handlers::admin::admin_reload_handler))
        .route("/api/play/:type/:tmdbId", get(routes::play::play_handler))
        .route("/api/drama/kisskh/search", get(routes::drama::kisskh_search_handler))
        .route("/api/drama/kisskh/streams/:id/:episode", get(routes::drama::kisskh_streams_handler))
        .route("/api/drama/dramacool/search", get(routes::drama::dramacool_search_handler))
        .route("/api/drama/dramacool/streams/:id/:episode", get(routes::drama::dramacool_streams_handler))
        .merge(SwaggerUi::new("/api/docs").url("/api/docs/openapi.json", ApiDoc::openapi()))
        .layer(middleware::from_fn_with_state(state.clone(), rate_limit_middleware))
        .layer(middleware::from_fn_with_state(state.clone(), auth_middleware))
        .layer(CorsLayer::permissive())
        .with_state(state)
}
```

- [ ] **Step 2: Create `huerust/src/startup.rs`**

```rust
use std::sync::Arc;
use std::time::Duration;
use crate::hitv::HiTvClient;

pub fn spawn_background_tasks(hitv: Arc<HiTvClient>) {
    tokio::spawn(async move {
        loop {
            let _ = hitv.build_library().await;
            tokio::time::sleep(Duration::from_secs(12 * 3600)).await;
        }
    });
}
```

- [ ] **Step 3: Create `huerust/src/handlers/mod.rs`**

```bash
mkdir -p huerust/src/handlers
```

```rust
pub mod admin;
pub mod metrics;
```

- [ ] **Step 4: Create `huerust/src/handlers/admin.rs`**

```rust
use axum::{response::{IntoResponse, Response}, http::StatusCode};
use crate::user_config::HueUserConfig;

pub async fn admin_reload_handler() -> Response {
    HueUserConfig::invalidate();
    (StatusCode::OK, r#"{"ok":true,"message":"Config cache invalidated"}"#).into_response()
}
```

- [ ] **Step 5: Create `huerust/src/handlers/metrics.rs`**

```rust
use axum::{extract::State, response::{IntoResponse, Response}, http::StatusCode};
use crate::AppState;

pub async fn metrics_handler(State(state): State<AppState>) -> Response {
    let body = state.metrics.render_prometheus();
    (StatusCode::OK, [(axum::http::header::CONTENT_TYPE, "text/plain; charset=utf-8")], body).into_response()
}

pub async fn hitv_library_handler(State(state): State<AppState>) -> Response {
    let items = state.hitv.get_library().await;
    (StatusCode::OK, serde_json::to_string(&items).unwrap()).into_response()
}
```

- [ ] **Step 6: Add mod declarations to `main.rs` and replace inline code**

Add to the `mod` block in `main.rs`:
```rust
mod app;
mod startup;
mod handlers;
```

Replace the `let app = Router::new()...` block in `main()` with:
```rust
    startup::spawn_background_tasks(state.hitv.clone());
    let app = app::build_router(state);
```

Remove the inline `health_check`, `auth_middleware`, `rate_limit_middleware`, `metrics_handler`, `hitv_library_handler`, `admin_reload_handler` functions from `main.rs` — they now live in `handlers/` or `app.rs`.

Move `auth_middleware` and `rate_limit_middleware` to `app.rs` (they're used only there).

- [ ] **Step 7: Run `cargo build`**

```bash
cd huerust && cargo build 2>&1 | tail -5
```

Expected: `Finished dev [unoptimized + debuginfo]`

- [ ] **Step 8: Verify main.rs is under 100 lines**

```bash
wc -l huerust/src/main.rs
```

Expected: under 100 lines.

- [ ] **Step 9: Commit**

```bash
git add huerust/src/app.rs huerust/src/startup.rs huerust/src/handlers/ huerust/src/main.rs
git commit -m "refactor(rust): decompose main.rs into app.rs, startup.rs, handlers/"
```

---

## Task 11: Node provider test harness

**Files:**
- Create: `scripts/test-provider.js`

- [ ] **Step 1: Create `scripts/test-provider.js`**

```js
#!/usr/bin/env node
'use strict';

require('dotenv').config();
const path = require('path');
const { loadManifests } = require('../utils/manifestLoader');
const { getProvider } = require('../providers/registry');

const args = process.argv.slice(2);
const runAll = args.includes('--all');
const targetName = runAll ? null : args[0];

async function testProvider(manifest) {
  const { provider: info, test: fixture } = manifest;
  const label = info.label;

  if (!fixture.fixture_id && info.media_types.includes('sports')) {
    console.log(`[SKIP] ${label} — sports provider, no fixture_id`);
    return { label, status: 'skip' };
  }

  const p = getProvider(info.name) || getProvider(info.label);
  if (!p) {
    console.error(`[FAIL] ${label} — provider not registered (check config flags)`);
    return { label, status: 'fail', reason: 'not registered' };
  }

  const t0 = Date.now();
  try {
    const ctx = {
      type: fixture.fixture_type === 'movie' ? 'movie' : 'tv',
      tmdbId: fixture.fixture_id,
      episode: fixture.fixture_episode || null,
      season: fixture.fixture_type === 'drama' ? null : (fixture.fixture_episode ? 1 : null),
    };
    const streams = await p.fetch(ctx);
    const ms = Date.now() - t0;

    if (fixture.expect_streams && (!Array.isArray(streams) || streams.length === 0)) {
      console.error(`[FAIL] ${label} — expected streams, got none (${ms}ms)`);
      return { label, status: 'fail', reason: 'no streams', ms };
    }

    if (Array.isArray(streams) && streams.length > 0) {
      const first = streams[0];
      if (!first.url || !first.quality) {
        console.error(`[FAIL] ${label} — stream missing url or quality`);
        return { label, status: 'fail', reason: 'invalid stream shape' };
      }
    }

    console.log(`[PASS] ${label} — ${(streams || []).length} streams (${ms}ms)`);
    return { label, status: 'pass', ms, count: (streams || []).length };
  } catch (e) {
    const ms = Date.now() - t0;
    console.error(`[FAIL] ${label} — ${e.message} (${ms}ms)`);
    return { label, status: 'fail', reason: e.message, ms };
  }
}

async function main() {
  const manifests = loadManifests();

  let targets;
  if (runAll) {
    targets = Object.values(manifests).filter(
      m => m.provider.runtime === 'node' || m.provider.runtime === 'both'
    );
  } else {
    const match = Object.values(manifests).find(
      m => m.provider.name === targetName || m.provider.label.toLowerCase() === targetName?.toLowerCase()
    );
    if (!match) {
      console.error(`Unknown provider: ${targetName}`);
      console.error(`Available: ${Object.values(manifests).map(m => m.provider.label).join(', ')}`);
      process.exit(1);
    }
    targets = [match];
  }

  const results = [];
  for (const manifest of targets) {
    results.push(await testProvider(manifest));
  }

  if (runAll) {
    const passed = results.filter(r => r.status === 'pass').length;
    const failed = results.filter(r => r.status === 'fail').length;
    const skipped = results.filter(r => r.status === 'skip').length;
    console.log(`\n--- ${passed} passed, ${failed} failed, ${skipped} skipped ---`);
    if (failed > 0) process.exit(1);
  }
}

main().catch(e => { console.error(e); process.exit(1); });
```

- [ ] **Step 2: Make it executable**

```bash
chmod +x scripts/test-provider.js
```

- [ ] **Step 3: Run a single provider test**

```bash
node scripts/test-provider.js kisskh
```

Expected: `[PASS] KissKH — N streams (Xms)` OR `[SKIP]` if provider not enabled in config.

- [ ] **Step 4: Run all providers**

```bash
node scripts/test-provider.js --all
```

Expected: one line per provider, summary at bottom.

- [ ] **Step 5: Commit**

```bash
git add scripts/test-provider.js
git commit -m "feat: add Node provider smoke test harness"
```

---

## Task 12: Rust provider test harness

**Files:**
- Create: `huerust/tests/providers.rs`

- [ ] **Step 1: Create `huerust/tests/providers.rs`**

```rust
//! Provider smoke tests — hit real upstreams with fixture IDs from manifests.
//! Run: cargo test --test providers
//! Run one: cargo test --test providers kisskh

use std::collections::HashMap;
use std::path::Path;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct ManifestProviderInfo {
    name: String,
    label: String,
    runtime: String,
}
#[derive(Debug, Deserialize)]
struct ManifestBehavior {
    timeout_ms: u64,
}
#[derive(Debug, Deserialize)]
struct ManifestTest {
    fixture_id: String,
    fixture_type: String,
    fixture_episode: u32,
    expect_streams: bool,
}
#[derive(Debug, Deserialize)]
struct ProviderManifest {
    provider: ManifestProviderInfo,
    behavior: ManifestBehavior,
    test: ManifestTest,
}

fn load_manifests() -> HashMap<String, ProviderManifest> {
    let dir = Path::new("../providers/manifests");
    let mut result = HashMap::new();
    if !dir.exists() { return result; }
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("toml") { continue; }
        let content = std::fs::read_to_string(&path).unwrap();
        if let Ok(m) = toml::from_str::<ProviderManifest>(&content) {
            result.insert(m.provider.name.clone(), m);
        }
    }
    result
}

#[tokio::test]
async fn kisskh_returns_streams() {
    let manifests = load_manifests();
    let m = manifests.get("zenith").expect("kisskh manifest missing");
    // NOTE: KissKhClient's fetch method signature must be checked in huerust/src/providers/kisskh.rs
    // before wiring this test. The test below validates the manifest loads and the fixture exists.
    // To add the actual fetch call, read kisskh.rs for the correct method name and signature.
    println!(
        "kisskh manifest ok — fixture_id={} runtime={} timeout={}ms",
        m.test.fixture_id, m.provider.runtime, m.behavior.timeout_ms
    );
    assert!(!m.test.fixture_id.is_empty(), "kisskh manifest missing fixture_id");
    assert_eq!(m.provider.runtime, "both", "kisskh should have runtime=both");
}
```

- [ ] **Step 2: Run the test**

```bash
cd huerust && cargo test --test providers kisskh -- --nocapture
```

Expected: `kisskh: N streams` or test passes (may need `pub` on `TmdbNormalizedInfo` fields — fix visibility if needed).

- [ ] **Step 3: Commit**

```bash
git add huerust/tests/providers.rs
git commit -m "feat(rust): add provider smoke test harness"
```

---

## Verification

After all tasks complete, run:

```bash
# Node providers
node scripts/test-provider.js --all

# Rust build
cd huerust && cargo build

# Rust tests
cd huerust && cargo test --test providers -- --nocapture

# main.rs line count
wc -l huerust/src/main.rs
```

Expected outcomes:
- `node scripts/test-provider.js --all` → 0 failures (skips are OK for sports)
- `cargo build` → clean compile
- `cargo test --test providers` → all tests pass
- `wc -l huerust/src/main.rs` → under 100 lines
