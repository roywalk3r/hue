# Provider Manifest System — Design Spec

**Date:** 2026-05-06  
**Author:** roywalk3r  
**Status:** Approved

---

## Problem

Adding a new provider to Hue requires touching multiple files in both runtimes, with no clear rules about what goes where. Behavior (timeouts, retries, circuit breaker config) is hardcoded per-provider. There is no way to test a provider in isolation. The Node↔Rust bridge is informal. `main.rs` grows with every new provider.

---

## Goal

A provider manifest system that makes adding, testing, and maintaining providers mechanical — not artisanal. After this work, adding a provider means: write a manifest + write the impl. Nothing else changes.

---

## Architecture Decision

Both runtimes are permanent:

- **Rust** — HTTP-only providers, parallel aggregation, circuit breakers, caching
- **Node** — Browser-dependent providers (Playwright), auth, static serving
- **Bridge** — Formalized typed contract between the two

The split is intentional. The manifest system makes it explicit and enforced.

---

## Section 1: Provider Manifest Schema

Each provider gets a `manifest.toml` in `providers/manifests/`. Both runtimes read this directory at startup.

```toml
# providers/manifests/kisskh.toml

[provider]
name = "zenith"             # internal codename
label = "KissKH"            # display name
runtime = "rust"            # "rust" | "node" | "both"
media_types = ["drama"]     # "movie" | "series" | "drama" | "anime" | "sports"

[capabilities]
search = true
streams = true
episodes = true

[behavior]
timeout_ms = 12000
max_retries = 2
circuit_breaker = true
priority = 80               # 0–100, higher = preferred in aggregation

[test]
fixture_id = "9256"
fixture_type = "drama"      # "movie" | "series" | "drama" | "anime"
fixture_episode = 1
expect_streams = true
```

**Runtime values:**
- `rust` — Rust loads natively, Node ignores
- `node` — Node loads, Rust delegates to bridge
- `both` — exists on both sides; Rust preferred, Node as fallback. Test harness runs both the Rust and Node test for this provider.

**What the manifest encodes:**
- Which runtime owns this provider (answers the "where does this live?" question permanently)
- All behavior config (replaces every hardcoded timeout/retry in provider files)
- Capabilities (drives route registration and API docs)
- Test fixture (everything the harness needs — no separate test file required)

---

## Section 2: Directory Layout

```
providers/
  manifests/              ← new: one .toml per provider
    kisskh.toml
    dramacool.toml
    sflix.toml
    vixsrc.toml
    moviebox.toml
    hianime.toml
    mp4hydra.toml
    livesport.toml
    aisports.toml
    ppv.toml
    vidsrc.toml
    vidrock.toml
  kisskh.js               ← unchanged
  dramacool.js            ← unchanged
  sflix.js                ← unchanged
  ...

huerust/src/providers/    ← unchanged
  kisskh.rs
  dramacool.rs
  ...
```

`providers/manifests/` is the only new directory. No existing files move.

---

## Section 3: Registry Changes

### Node — `utils/providerRegistry.js` (new, ~60 lines)

- Reads all `providers/manifests/*.toml` at startup
- Filters to `runtime = "node" | "both"`
- Injects `behavior` config (timeout, retries) into each provider instance
- Existing provider JS files are unchanged — they receive config instead of hardcoding it

### Rust — `huerust/src/registry.rs` (updated)

- Reads manifests at startup via `ProviderRegistry::from_manifests("../providers/manifests")`
- Filters to `runtime = "rust" | "both"`
- Uses `behavior` fields to configure circuit breaker thresholds and timeouts per provider

The current manual registration block in `main.rs`:
```rust
registry.register(Arc::new(MovieBoxProvider::new(moviebox_client.clone())));
registry.register(Arc::new(KissKhProvider::new(kisskh_client.clone())));
// ... etc
```
is replaced by a single manifest load call. Adding a new provider no longer requires touching `main.rs`.

---

## Section 4: Bridge Formalization

### Contract

Node scraper service exposes a standardized endpoint:

```
POST /providers/:name/streams
Content-Type: application/json

Body:
{
  "type": "movie" | "series" | "drama" | "anime",
  "id": "string",
  "episode"?: number,
  "season"?: number
}

Response:
{
  "streams": [{ "url": "string", "quality": "string", "headers"?: {} }],
  "provider": "string",
  "cached": boolean
}
```

Schema saved at `providers/bridge-schema.json` — both sides validate against it.

### Rust `BridgeClient` (updates `microservice.rs`)

- Only calls Node for providers where `runtime = "node"`
- Provider name comes from manifest, not hardcoded strings
- Returns the same `StreamResult` type as native Rust providers — callers can't distinguish bridge vs. native
- If Node is unreachable, affected providers report `unavailable` — no silent failures

---

## Section 5: Test Harness

### Node providers

```bash
node scripts/test-provider.js kisskh
node scripts/test-provider.js --all
```

- Reads `providers/manifests/kisskh.toml` for fixture
- Calls provider directly (bypasses Express routing)
- Validates response: `streams[]` present, each stream has `url` + `quality`
- Hits real upstream once — integration smoke test, not unit test
- Reports pass/fail + timing per provider

### Rust providers

```bash
cargo test --test providers kisskh
cargo test --test providers
```

- Same fixture approach from manifest
- `tokio::test` with real HTTP client — no mocking
- Validates same stream shape contract

### CI

```bash
node scripts/test-provider.js --all && cargo test --test providers
```

No fake data. If an upstream goes down, the test fails and identifies exactly which provider broke.

---

## Section 6: `main.rs` Decomposition

Current state: 494 lines, 5 responsibilities in one file.

Target state:

```
huerust/src/
  main.rs          # ~80 lines: env init, AppState wiring, server bind only
  app.rs           # new: all Router .route() calls
  startup.rs       # new: background task spawning
  handlers/
    jikan.rs       # jikan_* handlers
    admin.rs       # admin_reload, admin_refresh_tokens
    metrics.rs     # metrics_handler, hitv_library_handler
  routes/          # unchanged
    mod.rs
    hitv.rs
    streams.rs
    ...
```

**Rules enforced going forward:**
- `main.rs`: env init, state construction, server bind — nothing else
- New route: one line in `app.rs` + a file in `routes/`
- New handler: goes in `routes/` or `handlers/` — never in `main.rs`
- New background task: goes in `startup.rs`

`main.rs` stops growing permanently.

---

## Implementation Order

1. Write all manifests (`providers/manifests/*.toml`) — no code changes, immediate value
2. Node `providerRegistry.js` — reads manifests, injects config into existing providers
3. Rust registry manifest loading — replace hardcoded `registry.register()` block
4. Bridge formalization — typed `BridgeClient` + `bridge-schema.json`
5. `main.rs` decomposition — `app.rs`, `startup.rs`, `handlers/`
6. Test harness — `scripts/test-provider.js` + `huerust/tests/providers.rs`

---

## Success Criteria

- Adding a new provider requires only: a manifest + the provider impl file. No other files change.
- `node scripts/test-provider.js --all` validates every Node provider
- `cargo test --test providers` validates every Rust provider
- `main.rs` is under 100 lines
- The Node↔Rust bridge has a versioned schema at `providers/bridge-schema.json`
