# Provider Registry

## Name Map

| Display Name | Internal Key | Code File | Fast? | Timeout | Notes |
|--------------|--------------|-----------|-------|---------|-------|
| **Cinema** | `moviebox` | `moviebox_provider.rs` | yes | 30s | MovieBox CDN MP4 streams bypass proxy (datacenter IP-blocked) |
| **Nova** | `sflix` | `sflix_provider.rs` | yes | 30s | Playwright fallback available via scraper server |
| **Zenith** | `kisskh` | `kisskh_provider.rs` | no | 60s | Drama-specialized; kkey generation required for streams/subtitles |
| **Orient** | `dramacool` | `dramacool_provider.rs` | yes | 30s | Drama-specialized |
| **Aura** | `vixsrc` | `vixsrc_provider.rs` | yes | 15s | Formerly "autoembed" / "Blaze". API-first flow. |
| **Hydra** | `mp4hydra` | `mp4hydra_provider.rs` | yes | 30s | Direct MP4 links |
| **Luna** | `vidrock` | `vidrock_provider.rs` | yes | 20s | AES-256-CBC encrypted VidRock API |
| **VidZee** | `vidzee` | `vidzee_provider.rs` | yes | 15s | Two-layer AES decryption (GCM + CBC) |
| **Vortex** | `videasy` | `videasy_provider.rs` | no | 30s | Delegates WASM decryption to Node.js daemon |

## Circuit Breaker Config

All providers use the same circuit breaker settings:
- Failure threshold: 5 errors
- Open duration: 30 seconds
- Half-open max calls: 3

## Proxy Routing Rules

See `src/proxy/wrap.rs` for the full routing matrix.

Key rules:
- **Cinema (MovieBox) MP4 streams** — returned raw with headers. The CDN IP-blocks datacenters, so proxying through the server always 403s.
- **Vixsrc (Aura)** — forced `url` mode so signed URL query params are inherited by segments.
- **Pixeldrain / Google User Content / MovieBox CDN** — routed through `ts-proxy` with `noSynth` flag.
- **Default HLS** — `m3u8-proxy` with encrypted token.
- **Default MP4/MKV** — `ts-proxy` with encrypted token.

## Metrics Labels

Provider names are used as Prometheus label values:
- Request counts: `provider_requests_total{provider="Nova"}`
- Latency histograms: `provider_latency_seconds{provider="Cinema"}`
- Success/failure/timeout counts: `provider_results_total{provider="Zenith",result="success"}`

## Adding a New Provider

1. Create client module in `src/providers/` or `src/clients/`
2. Create `*_provider.rs` implementing the `StreamProvider` trait
3. Add module declarations to `src/providers/mod.rs`
4. Add client + provider to `AppState` in `src/app.rs`
5. Register in `ProviderRegistry` in `build_state()`
6. Add display name to `Metrics::new(&[...])` array
7. Document in this file
