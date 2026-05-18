# Stage 2: Provider System & Manifests

**Tier:** MEDIUM | **Status:** Complete

## Key Findings

### Provider Count (verified)
- 29 total `.js` files in `providers/`
- 2 file-level disabled (`.disabled` suffix): `vidsrccc.js.disabled`, `vidsrcwtf.js.disabled`
- Registry-disabled (hardcoded): `vidsrc-xyz`, `vidsrc-icu`, `vidsrccc-playwright`
- ~20 actively usable providers

### Public Name Aliasing
`PROVIDER_RENAME_MAP` has 23 entries. All real provider names hidden from API consumers.
`sanitizeText()` additionally scrubs provider names from stream titles.

### Full Provider Table
| Public Name | Real Name | Vertical | Runtime | Priority |
|---|---|---|---|---|
| Aura | vixsrc | Movie/Series | Both | 90 |
| Cinema | moviebox | Movie/Series | Both | 85 |
| Zenith | kisskh | Drama | Both | 80 |
| Stadium | livesport | Sports | Node | 80 |
| Orient | dramacool | Drama | Both | 75 |
| Field | aisports | Sports | Node | 75 |
| Blaze | autoembed | Movie/Series | Both | 70 |
| Arena | livetv | Sports | Node | 70 |
| Nova | sflix | Movie/Series | Both | 70 |
| Hydra | mp4hydra | Movie/Series | Both | 65 |
| Event | ppv | Sports | Node | 65 |
| Luna | vidrock | Movie/Series | Node | 60 |
| Cine | moviesmod | Movie/Series | Node | 50 |
| Ultra | uhdmovies | Movie/Series | Node | 50 |
| Prime | 4khdhub | Movie/Series | Node | 50 |
| Nebula | vidsrc-embed | Movie/Series | Node | 50 |
| Echo | showbox | Movie/Series | Node | 50 |
| Echo Pro | showbox-playwright | Movie/Series | Node | 50 |
| Saber | 9anime | Anime | Node | 50 |
| — | anikai | Anime | Node | 50 |
| Relay | streamed | Sports | Node | 50 |

### Manifest System (12 TOML files)
Schema sections: `[provider]`, `[capabilities]`, `[behavior]`, `[test]`

```toml
[behavior]
timeout_ms = 15000
max_retries = 2
circuit_breaker = true
priority = 90
```

**Critical gap:** `priority` and `circuit_breaker` fields are loaded by `manifestLoader.js` but NOT consumed by `routes/streams.js` or `providers/registry.js`. 8 providers have no manifest (default to priority 50).

### Circuit Breaker
- Threshold: 3 consecutive failures (empty results count)
- Cooldown: 10 minutes
- State: in-memory, resets on restart
- Sports providers: `circuit_breaker = false` in manifests (declared but not yet enforced by CB module itself)

### Manifest Loader (utils/manifestLoader.js)
- Lazy singleton: loads all TOMLs once, caches in `_cache`
- `getBehaviorConfig(name)` → defaults `{timeout_ms:15000, max_retries:2, circuit_breaker:true, priority:50}`
- `getNodeProviders()` → filters `runtime === 'node' | 'both'`
- `getManifestByLabel(label)` → case-insensitive lookup

### Huerust Deduplication Gate
When `enableHuerustProvider=true`, these 5 Node providers are forcibly disabled:
`['moviebox', 'sflix', 'vixsrc', 'kisskh', 'dramacool']`
All 5 have `runtime = "both"` in their manifests — by design.

### Drama Exclusion
`dramacool`/`orient` explicitly filtered from `/api/streams` route (lines 190, 294 of streams.js).
No drama aggregate endpoint exists — only provider-specific drama routes.
