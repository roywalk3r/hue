# Cross-Validation Results

**Status:** [VERIFIED with CONFLICTS]

All 7 stages are substantially consistent with source code. 5 specific conflicts identified and corrected.

## Conflicts Found and Resolved

### CONFLICT 1 — Route count
- **Stage 1 claimed:** 74 routes
- **Actual:** 76 routes
- **Resolution:** Accept 76. Two borderline `.use()` middleware mounts caused the discrepancy.

### CONFLICT 2 — Provider file counts
- **Stage 2 claimed:** 25 total files, 5 disabled
- **Actual:** 29 total files, 2 file-level disabled (`.disabled` suffix)
- **Resolution:** Accept 29/2. Stage 2 conflated registry-disabled with file-disabled.

### CONFLICT 3 — SFlix JWT expiry month
- **Stage 3 claimed:** expired March 2026
- **Actual:** expired April 10, 2026 (exp timestamp decoded)
- **Resolution:** April 2026. Both SFlix and MovieBox JWTs are now past expiry as of 2026-05-07.

### CONFLICT 4 — Token management env var migration
- **Stage 6 claimed:** token management moved to env vars in branding branch
- **Actual:** `DEFAULT_COOKIE` and `DEFAULT_X_USER` hardcoded JWT constants remain in `sflix.rs` and `moviebox.rs`; no `std::env::var` calls present
- **Resolution:** Claim is INCORRECT. Hardcoded JWTs remain. Stage 6 conflated manifest config with JWT handling.

### CONFLICT 5 — Ad-blocking "two implementations"
- **Stage 5 claimed:** two divergent ad-blocking frameworks
- **Actual:** one canonical `utils/interceptor.js` + `kisskh.js` implementing its own inline route-blocking without using the shared util
- **Resolution:** Directionally correct (inconsistency exists), but overstated. One shared util that one provider bypasses.

## Fully Verified Claims

- 3-process design with Redis ✅
- 11 route files ✅
- 12 TOML manifests ✅
- Manifest `priority` not wired into dispatch ✅
- Drama providers excluded from `/api/streams` ✅
- VidZee AES key `"qrincywincyspider"` ✅
- Static scrypt salt `'salt'` on BOTH Node and Rust sides ✅
- Circuit breaker: 3 failures, 10-min cooldown, in-memory ✅
- Browser semaphore 10-page limit ✅
- Token worker 15-minute interval ✅
- Warmup interval 1 hour ✅
- Fast deadline 5s, slow deadline 15s ✅
- Cache TTL 4 hours ✅
- Branding branch: 6 commits, 268 files, +6,353 net lines ✅
- Rust AES-256-CBC token encryption ✅
- PM2 port conflict (Rust 24608 vs Docker 24609 for Rust) ✅
- Vercel incompatibility ✅

## Gaps Identified (Not Covered by Any Stage)

1. `warmupWorker.js` has hardcoded `currentYear = 2025` — silently suppresses 2026 content (latent bug)
2. Static scrypt salt `'salt'` exists on BOTH sides (Node + Rust) — Stage 5 only attributed it to Rust
3. Drama route in `streams.js` has a third dispatch path (drama-specific with Rust-first fallback) architecturally distinct from main streams route — not clearly described in any stage
