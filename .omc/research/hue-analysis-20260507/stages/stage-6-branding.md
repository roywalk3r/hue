# Stage 6: Branding Branch Changes

**Tier:** LOW | **Status:** Complete

## Key Findings

### Branch Stats
- 6 commits ahead of main
- 268 files changed
- +7,071 insertions, -718 deletions = **+6,353 net lines**

### Two Meanings of "Branding"

**1. Cosmetic Branding (commit 849b8160)**
Standardized JSDoc header applied to all 268 source files:
```javascript
/**
 * Hue Stream API
 * Professional Streaming Aggregator
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */
```
Impact: cosmetic, establishes professional ownership/licensing identity.

**2. Architectural Branding (provider manifest system)**
Each provider gets an explicit identity, runtime assignment, behavior config, and test fixture via TOML. "Provider branding" = each source has a declared public name, capability set, and behavioral contract.

### Commit Breakdown
| Commit | Description |
|---|---|
| `849b8160` | feat: add professional branding headers to all source files |
| `b28641bf` | feat: add provider manifests for all 12 providers |
| `a6ac39dc` | feat: add manifest loader utility for Node providers |
| `3ebd0bfb` | docs: add provider manifest system implementation plan |
| `e518ce70` | docs: add provider manifest system design spec |
| `1a7488c3` | fix: align autoembed priority to scale and enable livesport stream assertion |

### Provider Manifest System

12 TOML manifests in `providers/manifests/`. Providers with `runtime = "both"`:
dramacool, kisskh, sflix, vixsrc, moviebox, mp4hydra, autoembed

Providers with `runtime = "node"`: livesport, aisports, livetv, ppv, vidrock

### Manifest Loader (utils/manifestLoader.js, 46 lines)
- Lazy singleton, process-lifetime cache
- `loadManifests()` — reads all TOMLs, indexes by `manifest.provider.name`
- `getManifestByLabel(label)` — case-insensitive
- `getBehaviorConfig(providerRealName)` — returns behavior or defaults
- `getNodeProviders()` — filters runtime === 'node' | 'both'

### Rust Engine Expansion
- Binary: 5.8MB → 24.5MB (4.2x growth)
- New: `security.rs`, expanded `hitv.rs`, full provider modules
- Deleted: `nineanime.rs` (consolidated)
- New providers: dramacool.rs, kisskh.rs, sflix.rs, vixsrc.rs, moviebox.rs
- Cargo.lock: +535 insertions (new dependencies)

### Provider Registry Integration
`providers/registry.js` updated to consume manifest behavior config:
- Injects `timeout_ms`, `max_retries`, `circuit_breaker`, `priority` at initialization
- No existing provider files modified

### New Route Handlers
- `routes/anime.js` — anime streaming endpoint
- `routes/drama.js` (65 lines) — drama streaming endpoint
- `routes/hitv.js` (+210 lines) — live TV aggregation
- `routes/streams.js` (+204 lines) — core aggregation engine
- `routes/play.js` (+62 lines) — stream playback orchestration

### Deployment Infrastructure
- `docker-compose.yml` (58 lines) — 4-service orchestration
- `ecosystem.config.js` (9 lines) — PM2 config

### Verification Note
Stage 6 originally claimed "token management moved to env vars" — this was INCORRECT.
Hardcoded JWT constants (`DEFAULT_COOKIE`, `DEFAULT_X_USER`) remain in Rust source files.
Both JWTs are now expired as of 2026-05-07.
