# huerust Architecture Refactor — Design Spec
**Date:** 2026-05-17  
**Branch:** feat/branding  
**Scope:** Pure structural refactor — no logic changes, no behaviour changes

---

## Problem

The codebase has two root structural problems:

1. **`main.rs` is a God Object** (451 lines): CLI handling, `AppState` definition, `RateLimitState`, background task spawning, 50+ route wiring, middleware, and 6 handler functions all live here.
2. **Flat module layout mixes concerns**: infrastructure modules (`cache`, `metrics`, `circuit_breaker`) sit alongside domain clients (`hitv`, `hianime`, `moviebox`) at the same top level, with no grouping signal.

Secondary issues:
- `wrap_with_proxy` (100 lines of proxy-routing logic) lives in `routes/streams.rs`
- `fetch_from_providers` (125 lines of dispatch logic) lives in `routes/streams.rs` but is shared with `routes/sse.rs`
- `ProviderRegistry` lives at the root level instead of inside `providers/`
- `RateLimitState` is defined inline in `main.rs`

---

## Goals

- `main.rs` reduced to ~60 lines (boot sequence only)
- `routes/streams.rs` reduced to ~100 lines (request lifecycle only)
- Modules grouped by role: `infra/`, `clients/`, `providers/`
- Route handlers extract only the state they need via Axum `FromRef` sub-states
- Zero behaviour changes — every function moves, none changes

---

## Module Layout (target)

```
src/
  main.rs                   ← boot only: init tracing, call app::build_state/router, serve
  app.rs                    ← AppState, router construction, background tasks, middleware

  infra/
    mod.rs
    cache.rs                ← moved from root (no changes)
    circuit_breaker.rs      ← moved from root
    coalesce.rs             ← moved from root
    metrics.rs              ← moved from root
    rate_limit.rs           ← extracted from main.rs (RateLimitState)

  clients/
    mod.rs
    tmdb.rs                 ← moved from root
    hitv.rs                 ← moved from root
    hianime.rs              ← moved from root
    moviebox.rs             ← moved from root (the client, not the provider wrapper)
    vidrock.rs              ← moved from root

  providers/
    mod.rs                  ← StreamProvider trait (unchanged)
    registry.rs             ← moved from root registry.rs
    dispatch.rs             ← fetch_from_providers extracted from routes/streams.rs
    kisskh.rs               ← unchanged
    kisskh_provider.rs      ← unchanged
    sflix.rs
    sflix_provider.rs
    dramacool.rs
    dramacool_provider.rs
    vixsrc.rs
    vixsrc_provider.rs
    mp4hydra.rs
    mp4hydra_provider.rs
    moviebox_provider.rs

  proxy/
    mod.rs                  ← m3u8/ts/sub handlers (unchanged)
    wrap.rs                 ← wrap_with_proxy extracted from routes/streams.rs
    adsurgery.rs            ← unchanged
    sanitize.rs             ← unchanged

  routes/
    mod.rs
    streams.rs              ← slimmed: cache → TMDB → coalesce → dispatch → wrap → respond
    sse.rs
    drama.rs
    hitv.rs
    anime.rs
    moviebox.rs
    play.rs
    providers.rs
    system.rs               ← health, metrics, status, admin handlers (moved from main.rs)
    vidrock.rs

  models/                   ← unchanged
  security.rs               ← unchanged
  config.rs                 ← unchanged
  filters.rs                ← unchanged
  openapi.rs                ← unchanged
  constants.rs              ← unchanged
  user_config.rs            ← unchanged
  microservice.rs           ← unchanged
```

---

## AppState & Sub-states

`AppState` remains the single source-of-truth. Axum's `FromRef` trait lets route handlers extract only the fields they need.

```rust
// app.rs
#[derive(Clone)]
pub struct AppState {
    // Infrastructure
    pub client:        reqwest::Client,
    pub config:        Arc<AppConfig>,
    pub cache:         Arc<StreamCache>,
    pub metrics:       Arc<Metrics>,
    pub secret:        String,
    pub proxy_base:    String,
    pub start_time:    std::time::Instant,
    pub rate_limiter:  Arc<RateLimitState>,

    // Stream dispatch
    pub registry:      Arc<ProviderRegistry>,
    pub filter_config: Arc<FilterConfig>,
    pub coalescer:     Arc<FetchCoalescer>,

    // Domain clients
    pub tmdb:          Arc<TmdbClient>,
    pub hitv:          Arc<HiTvClient>,
    pub kisskh:        Arc<KissKhClient>,
    pub dramacool:     Arc<DramacoolClient>,
    pub moviebox:      Arc<MovieBoxClient>,
    pub hianime:       Arc<HianimeClient>,
}
```

Example sub-state for stream routes:

```rust
#[derive(Clone)]
pub struct StreamState { /* registry, cache, tmdb, filter_config, coalescer, metrics, config, secret, proxy_base */ }

impl FromRef<AppState> for StreamState { ... }
```

Drama, anime, hitv, and system routes each get an analogous sub-state containing only what they use.

---

## Key Moves (no logic changes)

| Function / Type | Current location | Target location |
|---|---|---|
| `RateLimitState` | `main.rs` | `infra/rate_limit.rs` |
| `AppState` + router + background tasks + middleware | `main.rs` | `app.rs` |
| `health_check`, `metrics_handler`, `status_handler`, `hitv_library_handler`, `admin_*` | `main.rs` | `routes/system.rs` |
| `wrap_with_proxy` | `routes/streams.rs` | `proxy/wrap.rs` |
| `fetch_from_providers` | `routes/streams.rs` | `providers/dispatch.rs` |
| `ProviderRegistry` | `registry.rs` (root) | `providers/registry.rs` |
| `TmdbClient` | `tmdb.rs` (root) | `clients/tmdb.rs` |
| `HiTvClient` | `hitv.rs` (root) | `clients/hitv.rs` |
| `HianimeClient` | `hianime.rs` (root) | `clients/hianime.rs` |
| `MovieBoxClient` | `moviebox.rs` (root) | `clients/moviebox.rs` |
| `VidRockClient` | `vidrock.rs` (root) | `clients/vidrock.rs` |
| `StreamCache`, `CircuitBreaker`, `Metrics`, `FetchCoalescer` | root | `infra/` |

---

## What Does NOT Change

- All provider implementations (`*Client`, `*Provider`) — same code, same split
- `StreamProvider` trait signature
- `StreamCache`, `CircuitBreaker`, `FetchCoalescer`, `AppConfig` internals
- Route handler logic
- All existing tests

---

## Success Criteria

- `cargo build` passes with zero warnings after refactor
- `cargo test` passes — all 22+ existing tests green
- `main.rs` ≤ 70 lines
- `routes/streams.rs` ≤ 120 lines
- No route handler has `AppState` in its signature — all use a sub-state
- Middleware (`auth_middleware`, `rate_limit_middleware`) may keep `AppState` — middleware wraps the full router and legitimately needs broad access
