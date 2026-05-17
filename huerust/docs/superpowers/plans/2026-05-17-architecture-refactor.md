# huerust Architecture Refactor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Restructure huerust's module layout into `infra/`, `clients/`, updated `providers/`, and `proxy/` groups; slim `main.rs` to ~60 lines; introduce Axum `FromRef` sub-states so route handlers receive only what they need.

**Architecture:** Pure structural refactor — every function and type moves to a better-organised location, nothing changes. The final layout groups infrastructure (`cache`, `metrics`, `circuit_breaker`, `coalesce`, `rate_limit`) under `infra/`, domain scrapers (`tmdb`, `hitv`, `hianime`, `moviebox`, `vidrock`) under `clients/`, and puts `ProviderRegistry`, the dispatch loop, and proxy-wrap logic each in their logical home. A single `AppState` survives with Axum `FromRef` projections so each route module only sees its slice.

**Tech Stack:** Rust 2021, axum 0.7 (`FromRef`), tokio, reqwest, moka, dashmap, tracing, utoipa

---

## File Map

| Action | Path |
|--------|------|
| Create | `src/infra/mod.rs` |
| Move   | `src/cache.rs` → `src/infra/cache.rs` |
| Move   | `src/circuit_breaker.rs` → `src/infra/circuit_breaker.rs` |
| Move   | `src/coalesce.rs` → `src/infra/coalesce.rs` |
| Move   | `src/metrics.rs` → `src/infra/metrics.rs` |
| Create | `src/infra/rate_limit.rs` |
| Create | `src/clients/mod.rs` |
| Move   | `src/tmdb.rs` → `src/clients/tmdb.rs` |
| Move   | `src/hitv.rs` → `src/clients/hitv.rs` |
| Move   | `src/hianime.rs` → `src/clients/hianime.rs` |
| Move   | `src/moviebox.rs` → `src/clients/moviebox.rs` |
| Move   | `src/vidrock.rs` → `src/clients/vidrock.rs` |
| Move   | `src/registry.rs` → `src/providers/registry.rs` |
| Create | `src/providers/dispatch.rs` |
| Create | `src/proxy/wrap.rs` |
| Create | `src/routes/system.rs` |
| Create | `src/app.rs` |
| Modify | `src/main.rs` |
| Modify | `src/providers/mod.rs` |
| Modify | `src/proxy/mod.rs` |
| Modify | `src/routes/mod.rs` |
| Modify | `src/routes/streams.rs` |
| Modify | `src/routes/sse.rs` |
| Modify | `src/routes/drama.rs` |
| Modify | `src/routes/hitv.rs` |
| Modify | `src/routes/anime.rs` |
| Modify | `src/routes/moviebox.rs` |
| Modify | `src/routes/play.rs` |
| Modify | `src/routes/providers.rs` |
| Modify | `src/providers/moviebox_provider.rs` |
| Modify | `src/providers/kisskh_provider.rs` |
| Modify | `src/providers/sflix_provider.rs` |
| Modify | `src/providers/dramacool_provider.rs` |
| Modify | `src/providers/vixsrc_provider.rs` |

---

## Task 1: Create `infra/` module

Move `cache`, `circuit_breaker`, `coalesce`, `metrics` into `src/infra/`. Two of these import each other (`circuit_breaker` → `metrics`, `coalesce` → `cache`), so move all four atomically.

**Files:**
- Create: `src/infra/mod.rs`
- Create: `src/infra/cache.rs` (content from `src/cache.rs`)
- Create: `src/infra/circuit_breaker.rs` (content from `src/circuit_breaker.rs`)
- Create: `src/infra/coalesce.rs` (content from `src/coalesce.rs`)
- Create: `src/infra/metrics.rs` (content from `src/metrics.rs`)
- Delete: `src/cache.rs`, `src/circuit_breaker.rs`, `src/coalesce.rs`, `src/metrics.rs`
- Modify: `src/main.rs`
- Modify: `src/infra/circuit_breaker.rs` (one import line)
- Modify: `src/infra/coalesce.rs` (one import line)
- Modify: `src/routes/streams.rs` (two import lines)

- [ ] **Step 1: Create `src/infra/mod.rs`**

```rust
pub mod cache;
pub mod circuit_breaker;
pub mod coalesce;
pub mod metrics;
```

- [ ] **Step 2: Copy the four files to their new locations**

```bash
cp src/cache.rs src/infra/cache.rs
cp src/circuit_breaker.rs src/infra/circuit_breaker.rs
cp src/coalesce.rs src/infra/coalesce.rs
cp src/metrics.rs src/infra/metrics.rs
```

- [ ] **Step 3: Fix cross-imports inside `infra/`**

`src/infra/circuit_breaker.rs` currently has `use crate::metrics::Metrics;`.
Change it to:
```rust
use crate::infra::metrics::Metrics;
```

`src/infra/coalesce.rs` currently has `use crate::cache::*` (or similar).
Change every `use crate::cache::` to `use crate::infra::cache::`.

- [ ] **Step 4: Update `src/main.rs` module declarations**

Remove:
```rust
mod cache;
mod circuit_breaker;
mod coalesce;
mod metrics;
```

Add (before the other mods):
```rust
mod infra;
```

Update the `use` statements in `main.rs` that reference these modules:
```rust
// Before
use crate::cache::StreamCache;
use crate::coalesce::FetchCoalescer;
use crate::metrics::Metrics;
// (circuit_breaker used only for construction — update inline)

// After
use crate::infra::cache::StreamCache;
use crate::infra::coalesce::FetchCoalescer;
use crate::infra::metrics::Metrics;
```

Also update any `crate::circuit_breaker::CircuitBreaker` references in `main.rs` to `crate::infra::circuit_breaker::CircuitBreaker`.

- [ ] **Step 5: Update `src/routes/streams.rs` imports**

```rust
// Before
use crate::cache::CacheKey;
use crate::metrics::Metrics;

// After
use crate::infra::cache::CacheKey;
use crate::infra::metrics::Metrics;
```

- [ ] **Step 6: Delete the old root-level files**

```bash
rm src/cache.rs src/circuit_breaker.rs src/coalesce.rs src/metrics.rs
```

- [ ] **Step 7: Verify build**

```bash
cargo build 2>&1 | grep -E "^error"
```
Expected: no output (zero errors).

- [ ] **Step 8: Run tests**

```bash
cargo test 2>&1 | tail -5
```
Expected: `test result: ok. N passed; 0 failed`

- [ ] **Step 9: Commit**

```bash
git add src/infra/ src/main.rs src/routes/streams.rs
git rm src/cache.rs src/circuit_breaker.rs src/coalesce.rs src/metrics.rs
git commit -m "refactor: move cache/circuit_breaker/coalesce/metrics into infra/"
```

---

## Task 2: Create `clients/` module

Move the five domain scraper clients (`tmdb`, `hitv`, `hianime`, `moviebox`, `vidrock`) into `src/clients/`. These are only sourced — nothing they import from each other — so each consumer just needs its `use` path updated.

**Files:**
- Create: `src/clients/mod.rs`
- Create: `src/clients/tmdb.rs` (from `src/tmdb.rs`)
- Create: `src/clients/hitv.rs` (from `src/hitv.rs`)
- Create: `src/clients/hianime.rs` (from `src/hianime.rs`)
- Create: `src/clients/moviebox.rs` (from `src/moviebox.rs`)
- Create: `src/clients/vidrock.rs` (from `src/vidrock.rs`)
- Delete: the five root-level files
- Modify: `src/main.rs`, `src/routes/hitv.rs`, `src/routes/anime.rs`, `src/routes/moviebox.rs`, `src/routes/drama.rs`, `src/providers/moviebox_provider.rs`

- [ ] **Step 1: Create `src/clients/mod.rs`**

```rust
pub mod tmdb;
pub mod hitv;
pub mod hianime;
pub mod moviebox;
pub mod vidrock;
```

- [ ] **Step 2: Copy the five client files**

```bash
cp src/tmdb.rs src/clients/tmdb.rs
cp src/hitv.rs src/clients/hitv.rs
cp src/hianime.rs src/clients/hianime.rs
cp src/moviebox.rs src/clients/moviebox.rs
cp src/vidrock.rs src/clients/vidrock.rs
```

- [ ] **Step 3: Update `src/main.rs`**

Remove:
```rust
mod tmdb;
mod hitv;
mod hianime;
mod moviebox;
mod vidrock;
```

Add:
```rust
mod clients;
```

Update every `use crate::tmdb::`, `use crate::hitv::`, `use crate::hianime::`, `use crate::moviebox::`, `use crate::vidrock::` in `main.rs` to the `clients::` path, e.g.:

```rust
// Before
use crate::tmdb::TmdbClient;
use crate::moviebox::MovieBoxClient;
use crate::hitv::HiTvClient;
use crate::providers::kisskh::KissKhClient;  // unchanged — stays in providers/
use crate::providers::dramacool::DramacoolClient;  // unchanged
// (hianime used via crate::hianime::HianimeClient)

// After
use crate::clients::tmdb::TmdbClient;
use crate::clients::moviebox::MovieBoxClient;
use crate::clients::hitv::HiTvClient;
// (hianime)
// In AppState field: hianime: Arc<crate::clients::hianime::HianimeClient>
```

Also update the inline path in `AppState`:
```rust
// Before
hianime: Arc<crate::hianime::HianimeClient>,
// After
hianime: Arc<crate::clients::hianime::HianimeClient>,
```

- [ ] **Step 4: Update `src/routes/hitv.rs`**

```rust
// Before
use crate::{
    AppState,
    hitv::HiTvItem,
};

// After
use crate::{
    AppState,
    clients::hitv::HiTvItem,
};
```

- [ ] **Step 5: Update `src/routes/anime.rs`**

The file uses `crate::AppState`. If it has any direct `crate::hianime::` imports, change them to `crate::clients::hianime::`. (Check the full file — if only AppState is used and hianime is accessed via `state.hianime`, no import change is needed here.)

- [ ] **Step 6: Update `src/routes/drama.rs`**

If drama.rs has any `crate::moviebox::` or `crate::kisskh::` direct imports (beyond what's in AppState), update them. KissKhClient stays at `crate::providers::kisskh::KissKhClient` (providers/ is not moving).

- [ ] **Step 7: Update `src/providers/moviebox_provider.rs`**

```rust
// Before
use crate::moviebox::MovieBoxClient;

// After
use crate::clients::moviebox::MovieBoxClient;
```

- [ ] **Step 8: Delete the old root-level files**

```bash
rm src/tmdb.rs src/hitv.rs src/hianime.rs src/moviebox.rs src/vidrock.rs
```

- [ ] **Step 9: Verify build and test**

```bash
cargo build 2>&1 | grep -E "^error"
cargo test 2>&1 | tail -5
```
Expected: zero errors, all tests pass.

- [ ] **Step 10: Commit**

```bash
git add src/clients/ src/main.rs src/routes/ src/providers/
git rm src/tmdb.rs src/hitv.rs src/hianime.rs src/moviebox.rs src/vidrock.rs
git commit -m "refactor: move domain clients into clients/"
```

---

## Task 3: Move `ProviderRegistry` into `providers/`

`src/registry.rs` is a provider-registry — it belongs in `src/providers/registry.rs`.

**Files:**
- Create: `src/providers/registry.rs` (content from `src/registry.rs`)
- Delete: `src/registry.rs`
- Modify: `src/providers/mod.rs`, `src/main.rs`, `src/routes/streams.rs`

- [ ] **Step 1: Copy the file**

```bash
cp src/registry.rs src/providers/registry.rs
```

- [ ] **Step 2: Update `src/providers/registry.rs` imports**

The file imports `crate::infra::circuit_breaker::CircuitBreaker` (post-Task 1). Verify this import is already correct — if it still says `crate::circuit_breaker`, fix it:

```rust
// Before
use crate::circuit_breaker::CircuitBreaker;
// After
use crate::infra::circuit_breaker::CircuitBreaker;
```

- [ ] **Step 3: Add `registry` to `src/providers/mod.rs`**

```rust
pub mod vixsrc;
pub mod kisskh;
pub mod sflix;
pub mod dramacool;
pub mod mp4hydra;
pub mod moviebox_provider;
pub mod kisskh_provider;
pub mod sflix_provider;
pub mod dramacool_provider;
pub mod vixsrc_provider;
pub mod mp4hydra_provider;
pub mod registry;   // ← add this line
```

- [ ] **Step 4: Update `src/main.rs`**

Remove `mod registry;`.

Update:
```rust
// Before
use crate::registry::ProviderRegistry;
// After
use crate::providers::registry::ProviderRegistry;
```

- [ ] **Step 5: Update `src/routes/streams.rs`**

```rust
// Before
use crate::registry::ProviderRegistry;
// After
use crate::providers::registry::ProviderRegistry;
```

- [ ] **Step 6: Delete old file**

```bash
rm src/registry.rs
```

- [ ] **Step 7: Verify build and test**

```bash
cargo build 2>&1 | grep -E "^error"
cargo test 2>&1 | tail -5
```

- [ ] **Step 8: Commit**

```bash
git add src/providers/ src/main.rs src/routes/streams.rs
git rm src/registry.rs
git commit -m "refactor: move ProviderRegistry into providers/registry.rs"
```

---

## Task 4: Extract `RateLimitState` into `infra/rate_limit.rs`

`RateLimitState` and its `check()` method are defined inline in `main.rs`. Extract to `src/infra/rate_limit.rs`.

**Files:**
- Create: `src/infra/rate_limit.rs`
- Modify: `src/infra/mod.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Create `src/infra/rate_limit.rs`**

```rust
use dashmap::DashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Clone)]
pub struct RateLimitState {
    pub store: Arc<DashMap<String, (usize, Instant)>>,
    pub max_requests: usize,
    pub window_secs: u64,
}

impl RateLimitState {
    pub fn new(max_requests: usize, window_secs: u64) -> Self {
        Self {
            store: Arc::new(DashMap::new()),
            max_requests,
            window_secs,
        }
    }

    pub fn check(&self, key: &str) -> bool {
        let now = Instant::now();
        let window = Duration::from_secs(self.window_secs);

        if let Some(mut entry) = self.store.get_mut(key) {
            let (count, start) = *entry.value();
            if now.duration_since(start) > window {
                *entry.value_mut() = (1, now);
                return true;
            }
            if count >= self.max_requests {
                return false;
            }
            *entry.value_mut() = (count + 1, start);
            true
        } else {
            self.store.insert(key.to_string(), (1, now));
            true
        }
    }
}
```

- [ ] **Step 2: Add to `src/infra/mod.rs`**

```rust
pub mod cache;
pub mod circuit_breaker;
pub mod coalesce;
pub mod metrics;
pub mod rate_limit;   // ← add
```

- [ ] **Step 3: Update `src/main.rs`**

Remove the entire `RateLimitState` struct and `impl RateLimitState` block (lines ~74–110 in the original).

Add import:
```rust
use crate::infra::rate_limit::RateLimitState;
```

The `AppState` field and all usages of `rate_limiter` remain unchanged.

- [ ] **Step 4: Verify build and test**

```bash
cargo build 2>&1 | grep -E "^error"
cargo test 2>&1 | tail -5
```

- [ ] **Step 5: Commit**

```bash
git add src/infra/ src/main.rs
git commit -m "refactor: extract RateLimitState into infra/rate_limit.rs"
```

---

## Task 5: Move `wrap_with_proxy` to `proxy/wrap.rs`

`wrap_with_proxy` is 100 lines of proxy URL construction living in `routes/streams.rs`. It belongs in the proxy module. `routes/drama.rs` also imports it directly from `routes::streams`.

**Files:**
- Create: `src/proxy/wrap.rs`
- Modify: `src/proxy/mod.rs`
- Modify: `src/routes/streams.rs`
- Modify: `src/routes/drama.rs`

- [ ] **Step 1: Create `src/proxy/wrap.rs`**

Extract the `wrap_with_proxy` function verbatim from `src/routes/streams.rs` (currently the last ~100 lines). Add the required imports at the top of the new file:

```rust
use std::collections::HashMap;
use url::Url;
use crate::models::StreamResult;
use crate::security::encrypt_url;

pub fn wrap_with_proxy(streams: Vec<StreamResult>, proxy_base: &str, secret: &str) -> Vec<StreamResult> {
    streams.into_iter().map(|mut s| {
        // ... (paste the full function body verbatim from routes/streams.rs)
    }).collect()
}
```

The function body is identical to the current `pub(crate) fn wrap_with_proxy` in `routes/streams.rs`.

- [ ] **Step 2: Add `wrap` module to `src/proxy/mod.rs`**

Add at the top of `src/proxy/mod.rs`:
```rust
pub mod wrap;
```

- [ ] **Step 3: Remove `wrap_with_proxy` from `src/routes/streams.rs`**

Delete the `pub(crate) fn wrap_with_proxy` function body from `routes/streams.rs`.

Add import at the top of `routes/streams.rs`:
```rust
use crate::proxy::wrap::wrap_with_proxy;
```

The two call sites in `streams_handler` (`wrap_with_proxy(filtered, ...)`) remain unchanged.

- [ ] **Step 4: Fix `src/routes/drama.rs` import**

```rust
// Before
use crate::routes::streams::wrap_with_proxy;
// After
use crate::proxy::wrap::wrap_with_proxy;
```

- [ ] **Step 5: Verify build and test**

```bash
cargo build 2>&1 | grep -E "^error"
cargo test 2>&1 | tail -5
```

- [ ] **Step 6: Commit**

```bash
git add src/proxy/ src/routes/streams.rs src/routes/drama.rs
git commit -m "refactor: move wrap_with_proxy into proxy/wrap.rs"
```

---

## Task 6: Move `fetch_from_providers` to `providers/dispatch.rs`

The 125-line parallel dispatch loop lives in `routes/streams.rs` as a private function. It's shared logic (used by `sse.rs` too) and belongs in the providers layer.

**Files:**
- Create: `src/providers/dispatch.rs`
- Modify: `src/providers/mod.rs`
- Modify: `src/routes/streams.rs`
- Modify: `src/routes/sse.rs` (update call path if it uses `fetch_from_providers`)

- [ ] **Step 1: Create `src/providers/dispatch.rs`**

Move `ProviderTiming` here (out of `routes/streams.rs`) to avoid a mutual-import cycle. Then define `fetch_from_providers` in the same file:

```rust
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;
use tracing::{info, warn};
use serde::Serialize;
use utoipa::ToSchema;

use crate::clients::tmdb::TmdbNormalizedInfo;
use crate::models::StreamResult;
use crate::infra::metrics::Metrics;
use crate::providers::registry::ProviderRegistry;

#[derive(Serialize, Clone, ToSchema)]
pub struct ProviderTiming {
    pub provider: String,
    pub elapsed_ms: u64,
    pub streams_found: usize,
    pub cancelled: bool,
}

pub async fn fetch_from_providers(
    registry: Arc<ProviderRegistry>,
    metrics: Arc<Metrics>,
    tmdb_info: &TmdbNormalizedInfo,
    media_type: &str,
    season: u32,
    episode: u32,
    fast_only: bool,
    min_streams_threshold: usize,
    aggregate_deadline: Duration,
) -> (Vec<StreamResult>, Vec<ProviderTiming>) {
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use tokio::time::Instant;

    // paste the full function body verbatim from routes/streams.rs
    // (the body is ~120 lines starting with `let providers: Vec<_> = ...`)
}
```

- [ ] **Step 2: Add `dispatch` to `src/providers/mod.rs`**

```rust
pub mod registry;
pub mod dispatch;   // ← add
// ... existing mods
```

- [ ] **Step 3: Update `src/routes/streams.rs`**

Delete the `async fn fetch_from_providers(...)` body and the `ProviderTiming` struct (both now live in `providers/dispatch.rs`).

Add at the top:
```rust
use crate::providers::dispatch::{fetch_from_providers, ProviderTiming};
```

All call sites in `streams_handler` remain unchanged.

- [ ] **Step 4: Check `src/routes/sse.rs`**

Open `src/routes/sse.rs`. If it calls `crate::routes::streams::fetch_from_providers`, update to:
```rust
use crate::providers::dispatch::fetch_from_providers;
```

- [ ] **Step 5: Verify build and test**

```bash
cargo build 2>&1 | grep -E "^error"
cargo test 2>&1 | tail -5
```

- [ ] **Step 6: Commit**

```bash
git add src/providers/ src/routes/streams.rs src/routes/sse.rs
git commit -m "refactor: move fetch_from_providers into providers/dispatch.rs"
```

---

## Task 7: Create `routes/system.rs`

Six handlers currently live in `main.rs`: `health_check`, `metrics_handler`, `hitv_library_handler`, `status_handler`, `admin_reload_handler`, `admin_refresh_tokens_handler`. Move them to `routes/system.rs`.

**Files:**
- Create: `src/routes/system.rs`
- Modify: `src/routes/mod.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Create `src/routes/system.rs`**

```rust
use axum::{
    extract::State,
    response::{IntoResponse, Response},
    http::StatusCode,
};
use tracing::{info, warn};
use crate::AppState;

pub async fn health_check() -> &'static str {
    "Huerust is Alive"
}

pub async fn metrics_handler(State(state): State<AppState>) -> Response {
    let body = state.metrics.render_prometheus();
    (
        StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        body,
    ).into_response()
}

pub async fn hitv_library_handler(State(state): State<AppState>) -> Response {
    let items = state.hitv.get_library().await;
    (StatusCode::OK, serde_json::to_string(&items).unwrap()).into_response()
}

pub async fn admin_reload_handler() -> Response {
    crate::user_config::HueUserConfig::invalidate();
    (StatusCode::OK, "{\"ok\":true,\"message\":\"Config cache invalidated — next request reloads from disk\"}").into_response()
}

pub async fn admin_refresh_tokens_handler(State(state): State<AppState>) -> Response {
    let node_api_url = std::env::var("NODE_API_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:3003".to_string());
    let url = format!("{}/system/refresh-tokens", node_api_url);

    match state.client.post(&url).send().await {
        Ok(resp) => {
            crate::user_config::HueUserConfig::invalidate();
            let status = if resp.status().is_success() { StatusCode::OK } else { StatusCode::BAD_GATEWAY };
            let body = resp.text().await.unwrap_or_default();
            info!("[admin] Token refresh response: status={}, body={}", status, body);
            (status, format!("{{\"ok\":true,\"message\":\"Token refresh triggered\",\"node_response\":{}}}", body)).into_response()
        }
        Err(e) => {
            warn!("[admin] refresh-tokens call to Node.js failed: {}", e);
            (StatusCode::BAD_GATEWAY, format!("{{\"ok\":false,\"error\":\"Node.js unreachable: {}\"}}", e)).into_response()
        }
    }
}

pub async fn status_handler(State(state): State<AppState>) -> Response {
    let uptime_secs = state.start_time.elapsed().as_secs();
    let providers: Vec<serde_json::Value> = state.registry
        .list_all_arc()
        .iter()
        .map(|p| serde_json::json!({ "name": p.real_name(), "enabled": true }))
        .collect();
    let resp = serde_json::json!({
        "success": true,
        "version": "3.1.0",
        "uptime": uptime_secs,
        "metrics": {},
        "providers": providers
    });
    (StatusCode::OK, serde_json::to_string(&resp).unwrap()).into_response()
}
```

- [ ] **Step 2: Add to `src/routes/mod.rs`**

```rust
pub mod hitv;
pub mod play;
pub mod streams;
pub mod drama;
pub mod sse;
pub mod providers;
pub mod moviebox;
pub mod anime;
pub mod system;   // ← add
```

- [ ] **Step 3: Remove the six handlers from `src/main.rs`**

Delete the function bodies for `health_check`, `metrics_handler`, `hitv_library_handler`, `admin_reload_handler`, `admin_refresh_tokens_handler`, `status_handler`.

Update the route registrations in `main.rs` (temporary — they'll move to `app.rs` in Task 8):
```rust
.route("/health", get(routes::system::health_check))
.route("/metrics", get(routes::system::metrics_handler))
.route("/api/hitv/library", get(routes::system::hitv_library_handler))
.route("/api/status", get(routes::system::status_handler))
.route("/api/admin/reload", axum::routing::post(routes::system::admin_reload_handler))
.route("/api/admin/refresh-tokens", axum::routing::post(routes::system::admin_refresh_tokens_handler))
```

- [ ] **Step 4: Verify build and test**

```bash
cargo build 2>&1 | grep -E "^error"
cargo test 2>&1 | tail -5
```

- [ ] **Step 5: Commit**

```bash
git add src/routes/ src/main.rs
git commit -m "refactor: extract system handlers into routes/system.rs"
```

---

## Task 8: Create `app.rs` — AppState, router, background tasks, middleware

Everything except the boot sequence leaves `main.rs` and moves to `app.rs`.

**Files:**
- Create: `src/app.rs`
- Modify: `src/main.rs`
- Modify: all route files (update `use crate::AppState` → `use crate::app::AppState`)

- [ ] **Step 1: Create `src/app.rs`**

Move `AppState`, the router construction, middleware functions, and background task spawning into this file. Below is the complete file:

```rust
use axum::{
    Router,
    routing::get,
    middleware::{self, Next},
    extract::State,
    http::{StatusCode, Request},
    response::Response,
};
use axum::body::Body;
use reqwest::Client;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tower_http::cors::CorsLayer;
use tracing::warn;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;
use std::env;

use crate::infra::cache::StreamCache;
use crate::infra::circuit_breaker::CircuitBreaker;
use crate::infra::coalesce::FetchCoalescer;
use crate::infra::metrics::Metrics;
use crate::infra::rate_limit::RateLimitState;
use crate::clients::tmdb::TmdbClient;
use crate::clients::hitv::HiTvClient;
use crate::clients::hianime::HianimeClient;
use crate::clients::moviebox::MovieBoxClient;
use crate::providers::registry::ProviderRegistry;
use crate::providers::kisskh::KissKhClient;
use crate::providers::dramacool::DramacoolClient;
use crate::providers::vixsrc::VixsrcClient;
use crate::providers::{
    moviebox_provider::MovieBoxProvider,
    kisskh_provider::KissKhProvider,
    sflix_provider::SflixProvider,
    sflix::SflixClient,
    dramacool_provider::DramacoolProvider,
    vixsrc_provider::VixsrcProvider,
    mp4hydra_provider::Mp4HydraProvider,
};
use crate::filters::FilterConfig;
use crate::config::AppConfig;
use crate::openapi;
use crate::routes;
use crate::proxy;

#[derive(Clone)]
pub struct AppState {
    pub client: Client,
    pub tmdb: Arc<TmdbClient>,
    pub registry: Arc<ProviderRegistry>,
    pub cache: Arc<StreamCache>,
    pub filter_config: Arc<FilterConfig>,
    pub hitv: Arc<HiTvClient>,
    pub kisskh: Arc<KissKhClient>,
    pub dramacool: Arc<DramacoolClient>,
    pub moviebox: Arc<MovieBoxClient>,
    pub hianime: Arc<HianimeClient>,
    pub secret: String,
    pub proxy_base_url: String,
    pub config: Arc<AppConfig>,
    pub rate_limiter: Arc<RateLimitState>,
    pub coalescer: Arc<FetchCoalescer>,
    pub metrics: Arc<Metrics>,
    pub start_time: std::time::Instant,
}

pub async fn build_state(cfg: &AppConfig) -> AppState {
    let tmdb = Arc::new(TmdbClient::new(cfg.tmdb_api_keys.clone()));
    let cache = Arc::new(StreamCache::new_from_config(cfg).await);
    let circuit_breaker = Arc::new(CircuitBreaker::new());
    let metrics = Arc::new(Metrics::new(&["Cinema", "Nova", "Zenith", "Orient", "Aura", "Hydra"]));
    circuit_breaker.set_metrics(metrics.clone());
    let filter_config = Arc::new(FilterConfig::default());

    let mut registry = ProviderRegistry::new(circuit_breaker.clone());
    let moviebox_client = Arc::new(MovieBoxClient::new(None));
    let kisskh_client = Arc::new(KissKhClient::new());
    let sflix_client = Arc::new(SflixClient::new());
    let dramacool_client = Arc::new(DramacoolClient::new());
    let hianime_client = Arc::new(HianimeClient::new());
    let vixsrc_client = Arc::new(VixsrcClient::new(
        Client::builder()
            .tcp_keepalive(Duration::from_secs(60))
            .connect_timeout(Duration::from_secs(8))
            .pool_max_idle_per_host(2)
            .pool_idle_timeout(Duration::from_secs(90))
            .timeout(Duration::from_secs(15))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
            .build()
            .unwrap(),
    ));

    registry.register(Arc::new(MovieBoxProvider::new(moviebox_client.clone())));
    registry.register(Arc::new(KissKhProvider::new(kisskh_client.clone())));
    registry.register(Arc::new(SflixProvider::new(sflix_client)));
    registry.register(Arc::new(DramacoolProvider::new(dramacool_client.clone())));
    registry.register(Arc::new(VixsrcProvider::new(vixsrc_client)));
    registry.register(Arc::new(Mp4HydraProvider::new()));

    AppState {
        client: Client::builder()
            .tcp_keepalive(Duration::from_secs(60))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
            .build()
            .unwrap(),
        tmdb: tmdb.clone(),
        registry: Arc::new(registry),
        cache,
        filter_config,
        hitv: Arc::new(HiTvClient::new(tmdb.clone())),
        kisskh: kisskh_client,
        dramacool: dramacool_client,
        moviebox: moviebox_client,
        hianime: hianime_client,
        secret: cfg.secret.clone(),
        proxy_base_url: cfg.proxy_base_url.clone(),
        config: Arc::new(cfg.clone()),
        rate_limiter: Arc::new(RateLimitState::new(120, 60)),
        coalescer: Arc::new(FetchCoalescer::new()),
        metrics,
        start_time: std::time::Instant::now(),
    }
}

pub fn spawn_background_tasks(state: &AppState) {
    let hitv_bg = state.hitv.clone();
    tokio::spawn(async move {
        loop {
            let _ = hitv_bg.build_library().await;
            tokio::time::sleep(Duration::from_secs(12 * 3600)).await;
        }
    });

    let rl_bg = state.rate_limiter.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(300));
        loop {
            interval.tick().await;
            let now = std::time::Instant::now();
            let window = Duration::from_secs(rl_bg.window_secs * 2);
            rl_bg.store.retain(|_, (_, start)| now.duration_since(*start) < window);
        }
    });
}

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/health",           get(routes::system::health_check))
        .route("/metrics",          get(routes::system::metrics_handler))
        .route("/m3u8-proxy",       get(proxy::m3u8_handler))
        .route("/ts-proxy",         get(proxy::ts_handler))
        .route("/sub-proxy",        get(proxy::sub_handler))
        .route("/api/hitv/library", get(routes::system::hitv_library_handler))
        .route("/api/hitv/browse",  get(routes::hitv::browse_handler))
        .route("/api/hitv/search",  get(routes::hitv::search_handler))
        .route("/api/hitv/latest",  get(routes::hitv::latest_handler))
        .route("/api/hitv/trending",get(routes::hitv::trending_handler))
        .route("/api/hitv/hero",    get(routes::hitv::hero_handler))
        .route("/api/hitv/drama",   get(routes::hitv::drama_handler))
        .route("/api/hitv/info/:id",get(routes::hitv::info_handler))
        .route("/api/hitv/play/:id/:episode", get(routes::hitv::play_handler))
        .route("/api/hitv/countries",  get(routes::hitv::countries_handler))
        .route("/api/hitv/categories", get(routes::hitv::categories_handler))
        .route("/api/hitv/genres",     get(routes::hitv::genres_handler))
        .route("/api/hitv/sorts",      get(routes::hitv::sorts_handler))
        .route("/api/hitv/:country/:sort/:genre", get(routes::hitv::dynamic_browse_handler))
        .route("/api/streams/:type/:tmdbId",      get(routes::streams::streams_handler))
        .route("/api/streams/:type/:tmdbId/live", get(routes::sse::sse_streams_handler))
        .route("/api/providers",  get(routes::providers::providers_handler))
        .route("/api/admin/reload",          axum::routing::post(routes::system::admin_reload_handler))
        .route("/api/admin/refresh-tokens",  axum::routing::post(routes::system::admin_refresh_tokens_handler))
        .route("/api/play/:type/:tmdbId",    get(routes::play::play_handler))
        .route("/api/drama/kisskh/search",   get(routes::drama::kisskh_search_handler))
        .route("/api/drama/kisskh/show",     get(routes::drama::kisskh_show_handler))
        .route("/api/drama/kisskh/browse",   get(routes::drama::kisskh_browse_handler))
        .route("/api/drama/kisskh/upcoming", get(routes::drama::kisskh_upcoming_handler))
        .route("/api/drama/kisskh/top-rating", get(routes::drama::kisskh_top_rating_handler))
        .route("/api/drama/kisskh/streams/:id/:episode",    get(routes::drama::kisskh_streams_handler))
        .route("/api/drama/dramacool/search",               get(routes::drama::dramacool_search_handler))
        .route("/api/drama/dramacool/streams/:id/:episode", get(routes::drama::dramacool_streams_handler))
        .route("/api/drama/moviebox/browse",                get(routes::drama::moviebox_browse_handler))
        .route("/api/drama/moviebox/search",                get(routes::drama::moviebox_search_handler))
        .route("/api/drama/moviebox/streams/:title/:episode", get(routes::drama::moviebox_streams_handler))
        .route("/api/drama/detail",          get(routes::drama::drama_detail_handler))
        .route("/api/moviebox/home",         get(routes::moviebox::home_handler))
        .route("/api/moviebox/discover",     get(routes::moviebox::discover_handler))
        .route("/api/moviebox/section",      get(routes::moviebox::section_handler))
        .route("/api/moviebox/trending",     get(routes::moviebox::trending_handler))
        .route("/api/moviebox/popular-movie",  get(routes::moviebox::popular_movie_handler))
        .route("/api/moviebox/popular-series", get(routes::moviebox::popular_series_handler))
        .route("/api/moviebox/hot-shorts",   get(routes::moviebox::hot_shorts_handler))
        .route("/api/moviebox/sa",           get(routes::moviebox::sa_handler))
        .route("/api/moviebox/thai",         get(routes::moviebox::thai_handler))
        .route("/api/moviebox/turkish",      get(routes::moviebox::turkish_handler))
        .route("/api/moviebox/western",      get(routes::moviebox::western_handler))
        .route("/api/moviebox/animation",    get(routes::moviebox::animation_handler))
        .route("/api/moviebox/black-shows",  get(routes::moviebox::black_shows_handler))
        .route("/api/moviebox/hero",         get(routes::moviebox::hero_handler))
        .route("/api/moviebox/bollywood",    get(routes::moviebox::bollywood_handler))
        .route("/api/moviebox/shorts",       get(routes::moviebox::shorts_handler))
        .route("/api/moviebox/filter",       get(routes::moviebox::filter_handler))
        .route("/api/moviebox/info",         get(routes::moviebox::info_handler))
        .route("/api/moviebox/search",       get(routes::moviebox::search_handler))
        .route("/api/anime/search",          get(routes::anime::search_handler))
        .route("/api/anime/latest",          get(routes::anime::latest_handler))
        .route("/api/anime/category",        get(routes::anime::category_handler))
        .route("/api/anime/home",            get(routes::anime::home_handler))
        .route("/api/anime/detail/:slug",    get(routes::anime::detail_handler))
        .route("/api/anime/episodes/:id",    get(routes::anime::episodes_handler))
        .route("/api/anime/servers",         get(routes::anime::servers_handler))
        .route("/api/anime/stream",          get(routes::anime::stream_handler))
        .route("/api/status",                get(routes::system::status_handler))
        .merge(SwaggerUi::new("/api/docs").url("/api/docs/openapi.json", openapi::ApiDoc::openapi()))
        .layer(middleware::from_fn_with_state(state.clone(), rate_limit_middleware))
        .layer(middleware::from_fn_with_state(state.clone(), auth_middleware))
        .layer(CorsLayer::permissive())
        .with_state(state)
}

pub async fn auth_middleware(
    State(state): State<AppState>,
    req: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    let path = req.uri().path();
    if path == "/ts-proxy" || path == "/m3u8-proxy" || path == "/sub-proxy" {
        return Ok(next.run(req).await);
    }
    let is_local = req.extensions()
        .get::<SocketAddr>()
        .map_or(false, |a| a.ip().is_loopback());
    if is_local { return Ok(next.run(req).await); }
    if let Ok(api_key) = env::var("API_KEY") {
        let auth = req.headers().get("x-api-key").and_then(|v| v.to_str().ok());
        if auth != Some(&api_key) { return Err(StatusCode::FORBIDDEN); }
    }
    Ok(next.run(req).await)
}

pub async fn rate_limit_middleware(
    State(state): State<AppState>,
    req: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    let path = req.uri().path();
    if path.starts_with("/m3u8-proxy") || path.starts_with("/ts-proxy")
        || path.starts_with("/sub-proxy") || path.starts_with("/storage/") {
        return Ok(next.run(req).await);
    }
    let ip = req.extensions()
        .get::<axum::extract::ConnectInfo<SocketAddr>>()
        .map(|c| c.0.ip().to_string())
        .or_else(|| req.headers().get("x-forwarded-for")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.split(',').next().unwrap_or(s).trim().to_string()))
        .unwrap_or_else(|| "unknown".to_string());
    if !state.rate_limiter.check(&ip) {
        return Ok((StatusCode::TOO_MANY_REQUESTS, "Rate limit exceeded").into_response());
    }
    Ok(next.run(req).await)
}
```

- [ ] **Step 2: Update all route files from `use crate::AppState` to `use crate::app::AppState`**

In each of these files, change the import:
```rust
// Before (in every route file)
use crate::AppState;

// After
use crate::app::AppState;
```

Files to update: `src/routes/streams.rs`, `src/routes/sse.rs`, `src/routes/drama.rs`, `src/routes/hitv.rs`, `src/routes/anime.rs`, `src/routes/moviebox.rs`, `src/routes/play.rs`, `src/routes/providers.rs`, `src/routes/system.rs`

- [ ] **Step 3: Slim `src/main.rs` to use `app::`**

Replace the entire body of `main.rs` with:

```rust
mod app;
mod infra;
mod clients;
mod providers;
mod proxy;
mod routes;
mod models;
mod security;
mod config;
mod filters;
mod openapi;
mod constants;
mod user_config;
mod microservice;

use dotenv::dotenv;
use tracing::info;
use std::net::SocketAddr;

#[tokio::main]
async fn main() {
    dotenv().ok();

    // CLI: --encrypt <url> [referer]
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 && args[1] == "--encrypt" {
        if args.len() < 3 {
            println!("Usage: huerust --encrypt <url> [referer]");
            return;
        }
        let url = &args[2];
        let referer = args.get(3).cloned();
        let cfg = config::AppConfig::from_env();
        let mut headers = std::collections::HashMap::new();
        if let Some(r) = referer { headers.insert("Referer".to_string(), r); }
        let token = security::encrypt_url(url, &cfg.secret, if headers.is_empty() { None } else { Some(headers) });
        println!("{}", token);
        return;
    }

    tracing_subscriber::fmt::init();

    let cfg = config::AppConfig::from_env();
    let state = app::build_state(&cfg).await;
    app::spawn_background_tasks(&state);
    let router = app::build_router(state.clone());

    let addr_str = format!("{}:{}", cfg.host, cfg.port);
    let addr: SocketAddr = addr_str.parse().expect("Invalid RUST_HOST or RUST_PORT");
    info!("Huerust listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();

    info!("Huerust Engine shutdown complete");
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c().await.expect("failed to install Ctrl+C handler");
    };
    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    tracing::info!("Shutdown signal received, starting graceful shutdown...");
}
```

- [ ] **Step 4: Verify build and test**

```bash
cargo build 2>&1 | grep -E "^error"
cargo test 2>&1 | tail -5
```

- [ ] **Step 5: Confirm `main.rs` line count**

```bash
wc -l src/main.rs
```
Expected: ≤ 70 lines.

- [ ] **Step 6: Commit**

```bash
git add src/app.rs src/main.rs src/routes/
git commit -m "refactor: extract AppState and router into app.rs, slim main.rs to boot sequence"
```

---

## Task 9: Introduce `FromRef` sub-states

Replace `State<AppState>` in route handlers with domain-specific sub-states. Middleware keeps `AppState` — it wraps the full router and legitimately needs broad access.

**Files:**
- Modify: `src/app.rs` (add sub-state structs + `FromRef` impls)
- Modify: `src/routes/streams.rs`, `src/routes/sse.rs`, `src/routes/drama.rs`, `src/routes/hitv.rs`, `src/routes/anime.rs`, `src/routes/moviebox.rs`, `src/routes/play.rs`, `src/routes/providers.rs`, `src/routes/system.rs`

- [ ] **Step 1: Add sub-state types to `src/app.rs`**

Add the following at the end of `src/app.rs` (after the middleware functions):

```rust
// ── Sub-state projections ───────────────────────────────────────────────────
use axum::extract::FromRef;

/// State for /api/streams and /api/streams/.../live (SSE)
#[derive(Clone)]
pub struct StreamState {
    pub registry:      Arc<ProviderRegistry>,
    pub cache:         Arc<StreamCache>,
    pub tmdb:          Arc<TmdbClient>,
    pub filter_config: Arc<FilterConfig>,
    pub coalescer:     Arc<FetchCoalescer>,
    pub metrics:       Arc<Metrics>,
    pub config:        Arc<AppConfig>,
    pub secret:        String,
    pub proxy_base_url: String,
}

impl FromRef<AppState> for StreamState {
    fn from_ref(s: &AppState) -> Self {
        Self {
            registry:      s.registry.clone(),
            cache:         s.cache.clone(),
            tmdb:          s.tmdb.clone(),
            filter_config: s.filter_config.clone(),
            coalescer:     s.coalescer.clone(),
            metrics:       s.metrics.clone(),
            config:        s.config.clone(),
            secret:        s.secret.clone(),
            proxy_base_url: s.proxy_base_url.clone(),
        }
    }
}

/// State for /api/drama/* routes
#[derive(Clone)]
pub struct DramaState {
    pub kisskh:        Arc<KissKhClient>,
    pub dramacool:     Arc<DramacoolClient>,
    pub moviebox:      Arc<MovieBoxClient>,
    pub config:        Arc<AppConfig>,
    pub secret:        String,
    pub proxy_base_url: String,
}

impl FromRef<AppState> for DramaState {
    fn from_ref(s: &AppState) -> Self {
        Self {
            kisskh:        s.kisskh.clone(),
            dramacool:     s.dramacool.clone(),
            moviebox:      s.moviebox.clone(),
            config:        s.config.clone(),
            secret:        s.secret.clone(),
            proxy_base_url: s.proxy_base_url.clone(),
        }
    }
}

/// State for /api/hitv/* routes
#[derive(Clone)]
pub struct HiTvState {
    pub hitv:   Arc<HiTvClient>,
    pub config: Arc<AppConfig>,
}

impl FromRef<AppState> for HiTvState {
    fn from_ref(s: &AppState) -> Self {
        Self { hitv: s.hitv.clone(), config: s.config.clone() }
    }
}

/// State for /api/anime/* routes
#[derive(Clone)]
pub struct AnimeState {
    pub hianime:       Arc<HianimeClient>,
    pub config:        Arc<AppConfig>,
    pub secret:        String,
    pub proxy_base_url: String,
}

impl FromRef<AppState> for AnimeState {
    fn from_ref(s: &AppState) -> Self {
        Self {
            hianime:       s.hianime.clone(),
            config:        s.config.clone(),
            secret:        s.secret.clone(),
            proxy_base_url: s.proxy_base_url.clone(),
        }
    }
}

/// State for /api/moviebox/* routes
#[derive(Clone)]
pub struct MovieBoxRouteState {
    pub moviebox: Arc<MovieBoxClient>,
    pub config:   Arc<AppConfig>,
}

impl FromRef<AppState> for MovieBoxRouteState {
    fn from_ref(s: &AppState) -> Self {
        Self { moviebox: s.moviebox.clone(), config: s.config.clone() }
    }
}

/// State for /api/providers
#[derive(Clone)]
pub struct ProvidersState {
    pub registry: Arc<ProviderRegistry>,
}

impl FromRef<AppState> for ProvidersState {
    fn from_ref(s: &AppState) -> Self {
        Self { registry: s.registry.clone() }
    }
}

/// State for system routes (health, metrics, status, admin)
#[derive(Clone)]
pub struct SystemState {
    pub metrics:    Arc<Metrics>,
    pub hitv:       Arc<HiTvClient>,
    pub registry:   Arc<ProviderRegistry>,
    pub start_time: std::time::Instant,
    pub client:     Client,
    pub config:     Arc<AppConfig>,
}

impl FromRef<AppState> for SystemState {
    fn from_ref(s: &AppState) -> Self {
        Self {
            metrics:    s.metrics.clone(),
            hitv:       s.hitv.clone(),
            registry:   s.registry.clone(),
            start_time: s.start_time,
            client:     s.client.clone(),
            config:     s.config.clone(),
        }
    }
}
```

- [ ] **Step 2: Update `src/routes/streams.rs` handler signature**

```rust
// Before
use crate::app::AppState;
// ...
pub async fn streams_handler(
    State(state): State<AppState>,
    ...
) -> Response { ... }

// After
use crate::app::StreamState;
// ...
pub async fn streams_handler(
    State(state): State<StreamState>,
    ...
) -> Response { ... }
```

Replace every `state.proxy_base_url` → `state.proxy_base_url` (field name unchanged), `state.config` → `state.config`, etc. All field names are identical — only the type name changes.

- [ ] **Step 3: Update `src/routes/sse.rs`**

Change `State<AppState>` → `State<StreamState>`, update import.

- [ ] **Step 4: Update `src/routes/drama.rs`**

```rust
// Before
use crate::app::AppState;
// After
use crate::app::DramaState;
```

Change all handler signatures from `State(state): State<AppState>` to `State(state): State<DramaState>`.
Field access is identical (`state.kisskh`, `state.dramacool`, `state.moviebox`, `state.secret`, `state.proxy_base_url`, `state.config`).

- [ ] **Step 5: Update `src/routes/hitv.rs`**

Change to `State<HiTvState>`. Fields: `state.hitv`, `state.config`.

- [ ] **Step 6: Update `src/routes/anime.rs`**

Change to `State<AnimeState>`. Fields: `state.hianime`, `state.config`, `state.secret`, `state.proxy_base_url`.

- [ ] **Step 7: Update `src/routes/moviebox.rs`**

Change to `State<MovieBoxRouteState>`. Fields: `state.moviebox`, `state.config`.

- [ ] **Step 8: Update `src/routes/providers.rs`**

Change to `State<ProvidersState>`. Field: `state.registry`.

- [ ] **Step 9: Update `src/routes/system.rs`**

Change to `State<SystemState>`. Fields: `state.metrics`, `state.hitv`, `state.registry`, `state.start_time`, `state.client`.

- [ ] **Step 10: Update `src/routes/play.rs`**

`play.rs` (`/api/play/:type/:tmdbId`) is a stream-lookup route that uses the same fields as `streams.rs`. Use `StreamState`:

```rust
// Before
use crate::app::AppState;
// After
use crate::app::StreamState;
```

Change the handler signature from `State(state): State<AppState>` to `State(state): State<StreamState>`.
If the compiler reports a missing field, add it to `StreamState` in `app.rs`.

- [ ] **Step 11: Verify build and test**

```bash
cargo build 2>&1 | grep -E "^error"
cargo test 2>&1 | tail -5
```

- [ ] **Step 12: Commit**

```bash
git add src/app.rs src/routes/
git commit -m "refactor: introduce FromRef sub-states per route domain"
```

---

## Task 10: Final verification

Confirm all success criteria from the spec are met.

**Files:** Read-only checks.

- [ ] **Step 1: Check line counts**

```bash
wc -l src/main.rs src/routes/streams.rs
```
Expected: `main.rs` ≤ 70 lines, `routes/streams.rs` ≤ 120 lines.

- [ ] **Step 2: Confirm no `AppState` in route handler signatures**

```bash
grep -r "State<AppState>" src/routes/
```
Expected: no output (middleware is in `src/app.rs`, not `src/routes/`).

- [ ] **Step 3: Full build and test**

```bash
cargo build 2>&1 | grep -E "^error|^warning\[unused"
cargo test -- --nocapture 2>&1 | tail -10
```
Expected: zero errors, all tests pass, no new unused-import warnings.

- [ ] **Step 4: Final commit**

```bash
git add -A
git commit -m "refactor: complete architecture restructure (infra/, clients/, sub-states)"
```
