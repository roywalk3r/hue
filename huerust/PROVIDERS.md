# Provider Registry

## Name Map

Provider name = `StreamProvider::name()` (the human label shown in the
player UI), `real_name` = `StreamProvider::real_name()` (the registry key
used in metrics / circuit-breaker / provider-scoring caches).

"Fast?" reflects `is_fast()`. A provider is fast if it can land in < 5 s
on the happy path. The fast/slow split drives the dispatch phases
(see [ARCHITECTURE.md](ARCHITECTURE.md) → "Request flow: stream
aggregation" → "Fast vs slow phase").

| Display | Real key | Code | Fast? | Timeout | Status / Notes |
|---------|----------|------|-------|---------|----------------|
| **Cinema** | `moviebox` | `moviebox_provider.rs` | yes | 8s | MovieBox MP4 streams — CDN IP-blocks datacenters, returned raw with headers (no proxy) |
| **Nova** | `sflix` | `sflix_provider.rs` | yes | 8s | Playwright fallback via scraper server when direct fetch fails |
| **Zenith** | `kisskh` | `kisskh_provider.rs` | no | 8s | Drama-specialized. Gated by `ENABLE_KISSKH` (default: off). kkey generation required for streams/subtitles. Movie/TV callers get empty list. |
| **Orient** | `dramacool` | `dramacool_provider.rs` | no | 8s | Drama-specialized. Registered in `AppState` but commented out in registry — disabled, xyra faulty. |
| **Aura** | `vixsrc` | `vixsrc_provider.rs` | yes | 8s | Direct Vixsrc.to client. Gated by `ENABLE_VIXSRC`. Two-step extraction: API key → master playlist. |
| **Aurex** | `aurex` | `xyra_aurex_provider.rs` | yes | 8s | Xyra streamhub `aurex-alpha` drama aggregator. Always on. |
| **Hydra** | `mp4hydra` | `mp4hydra_provider.rs` | yes | 10s | Direct MP4 links. Disabled in registry. |
| **Luna** | `vidrock` | `vidrock_provider.rs` | yes | 8s | AES-256-CBC encrypted VidRock API. |
| **VidZee** | `vidzee` | `vidzee_provider.rs` | no | 8s | Two-layer AES decryption (GCM + CBC). |
| **Vortex** | `videasy` | `videasy_provider.rs` | no | 8s | Delegates WASM decryption to Node.js daemon. Disabled in registry. |
| **Pearl** | `kissasian` | `kissasian_provider.rs` | no | 8s | KissAsian (Oriental) drama. Skips movie requests. |
| **Rose** | `kissasian_top` | `kissasian_top_provider.rs` | yes | 6s | KissAsian.top — direct MP4. Slug-keyed; SSE dispatch not yet wired. |
| **NoTorrent** | `notorrent` | `notorrent_provider.rs` | yes | 8s | NoTorrent (hianime-hosted). Filters fake-iframe streams (see [docs/notorrent.md](docs/notorrent.md)). |
| **Vidlink** | `vidlink` | `vidlink_provider.rs` | yes | 8s | Vidlink. |
| **Lordflix** | `lordflix` | `lordflix_provider.rs` | yes | 15s | Disabled in registry — Cloudflare attestation. |
| **Nebula** | `vidsrc-embed` | `vidsrcembed_provider.rs` | no | 8s | Vidsrc.to embed scraper. |
| **Vault** | `vidvault` | `vidvault_provider.rs` | yes | 8s | Disabled — hakunaymatata CDN 403s. |
| **Aura (Xyra)** | `xyra-vixsrc` | `xyra_vixsrc_provider.rs` | yes | 8s | Vixsrc via xyra.stream streamhub. Gated by `ENABLE_XYRA_VIXSRC` (default: on). Falls back to direct Aura if off. |
| **KissKH (Xyra)** | `kisskh_xyra` | `kisskh_xyra_provider.rs` | no | 8s | Disabled in registry — xyra faulty. Drama-only. |

## Dispatch phases

`ProviderRegistry::fetch_all` runs providers in two phases
(`src/providers/dispatch.rs`):

- **Fast phase** — every provider where `is_fast() == true`, fired in
  parallel. Each gets the full fast deadline. **No abort-on-threshold**:
  a still-resolving Aura is allowed to keep running even after 3 Luna
  streams land, because Vixsrc was previously starved by early-abort.
- **Slow phase** — runs only if the fast phase returned fewer than
  `min_streams_threshold` (default 3) distinct streams. Fires **all**
  healthy providers in parallel and **does** abort on threshold.

Phases are capped by `FAST_PROVIDER_DEADLINE_SECS` (default 5) and
`SLOW_PROVIDER_DEADLINE_SECS` (default 25).

## Sticky preferences & provider scoring

Two infra modules shape the order in which providers are tried:

- **`src/infra/sticky_pref.rs`** — Per-user, per-title cache of "which
  provider worked for me last time". Resolved before the fast phase
  fires; if a sticky provider exists, it goes first.
- **`src/infra/provider_score.rs`** — Per-title popularity scores driven
  by global success rate. The dispatch sorts healthy providers by score
  within the phase.

See [docs/provider-scoring.md](docs/provider-scoring.md) for the full
algorithm and the relation to the circuit breaker.

## Circuit Breaker Config

All providers use the same circuit breaker settings
(`src/infra/circuit_breaker.rs`):
- Failure threshold: 5 errors
- Open duration: 30 seconds
- Half-open max calls: 3

A provider's circuit is opened automatically on consecutive failures;
it is re-tested in half-open state with up to 3 calls before either
closing again or re-opening.

## Proxy Routing Rules

See `src/proxy/wrap.rs` for the full routing matrix. Key rules:

- **Cinema (MovieBox) MP4 streams** — returned raw with headers. The
  CDN IP-blocks datacenters, so proxying always 403s.
- **Aura (Vixsrc) / Aurex (Xyra Vixsrc)** — forced `url` mode so signed
  URL query params are inherited by segments.
- **Pixeldrain / Google User Content / MovieBox CDN** — routed through
  `ts-proxy` with `noSynth` flag (no TS sanitization).
- **Default HLS** — `m3u8-proxy` with encrypted `h_token`.
- **Default MP4/MKV** — `ts-proxy` with encrypted `h_token`.

Full details in [docs/proxy.md](docs/proxy.md).

## Metrics Labels

Provider names (the `name()` value) are used as Prometheus label values:
- Request counts: `provider_requests_total{provider="Nova"}`
- Latency histograms: `provider_latency_seconds{provider="Cinema"}`
- Success/failure/timeout counts: `provider_results_total{provider="Zenith",result="success"}`

The full name list lives in `Metrics::new(&[...])` in `src/app.rs`.

## Adding a New Provider

1. Create the low-level client in `src/clients/` (if it talks to an
   external API/scraper) or `src/providers/<name>.rs` (if it belongs
   inside the provider pipeline).
2. Create `src/providers/<name>_provider.rs` implementing `StreamProvider`.
   Implement at minimum `name`, `real_name`, `is_fast`, `timeout_secs`,
   and `fetch`.
3. Add module declarations to `src/providers/mod.rs` (and `clients/mod.rs`
   if the client lives in `clients/`).
4. Add the client + provider to `AppState` in `src/app.rs` (`build_state`).
5. Register the provider in `ProviderRegistry` inside `build_state`. If
   it should be toggleable, add an `ENABLE_<NAME>` env-var helper.
6. Add the display name to the `Metrics::new(&[...])` array in
   `src/app.rs`.
7. Document the provider in this file with its name → real_name → code
   → fast/timeout/status mapping.
