# Hue Universal Cloudflare Worker Proxy

## What It Does

A single Cloudflare Worker that replaces multiple proxy responsibilities across your Hue stack:

| Feature | What It Solves |
|---|---|
| **Generic Provider Proxy** | Any provider URL can be proxied with forged `Referer`/`Origin`/`User-Agent` |
| **M3U8 Rewriting** | Relative `.ts` / key URLs inside HLS playlists are rewritten to route back through the worker |
| **Subtitle CORS** | `.vtt`, `.srt`, `.ass` files gain `Access-Control-Allow-Origin: *` automatically |
| **Image Optimization** | Posters/thumbnails routed through `images.weserv.nl` for WebP/AVIF + resizing |
| **Edge Caching** | Playlists (30s), segments (24h), subtitles (1h), images (7d) cached at Cloudflare edge |
| **Provider Fallback** | If `moviebox.id` returns 403/5xx, auto-retries `themoviebox.org`, then `netfilm.world` |
| **Geo Routing** | Block specific countries via `BLOCKED_COUNTRIES` env var |
| **Range Passthrough** | `Range: bytes=0-` headers forwarded correctly for video seeking |
| **Base64 Segment Decode** | Detects and decodes base64-wrapped `.ts` segments like your Node.js proxy |

## Deploy

### 1. Install Wrangler (once)
```bash
npm install -g wrangler
wrangler login
```

### 2. Create KV Namespaces (optional but recommended)
```bash
wrangler kv namespace create "PROVIDER_MIRRORS"
# Copy the id into wrangler.toml
wrangler kv namespace create "EDGE_CACHE_CONFIG"
# Copy the id into wrangler.toml
```

### 3. Seed Provider Mirrors (optional)
```bash
wrangler kv key put --namespace-id=YOUR_ID "moviebox" '["https://moviebox.id","https://themoviebox.org","https://netfilm.world"]'
wrangler kv key put --namespace-id=YOUR_ID "kisskh" '["https://kisskh.ovh"]'
```

### 4. Set Secrets
```bash
wrangler secret put ALLOWED_ORIGINS
# Enter: https://yourdomain.com,https://yourapp.com  (or * for open)
wrangler secret put BLOCKED_COUNTRIES
# Enter: CN,RU  (leave blank for none)
wrangler secret put IMAGE_OPTIMIZATION
# Enter: true
wrangler secret put DEFAULT_REFERER
# Enter: https://moviebox.id
```

### 5. Deploy
```bash
cd proxy
wrangler deploy
```

## Usage

### From Rust Backend
Replace `proxy_base_url` in your `.env`:
```
PROXY_BASE_URL=https://hue-universal-proxy.your-subdomain.workers.dev
```

The Rust `security::wrap_with_proxy` will now encrypt URLs pointing to the worker.

### From Frontend / Node.js Backend
```
https://hue-universal-proxy.your-subdomain.workers.dev/?target=<URL>&referer=<REF>&origin=<ORIGIN>
```

Query parameters:
| Param | Required | Description |
|---|---|---|
| `target` or `url` or `u` | Yes | The upstream URL to proxy |
| `referer` | No | `Referer` header sent upstream (defaults to target origin) |
| `origin` | No | `Origin` header sent upstream (defaults to referer) |
| `userAgent` or `ua` | No | `User-Agent` header sent upstream |
| `cache` | No | `cache=false` to bypass edge cache for this request |
| `w` or `width` | No | Image width hint (only used when `IMAGE_OPTIMIZATION=true`) |

### Examples

**Proxy a MovieBox stream:**
```
https://hue-universal-proxy.your-subdomain.workers.dev/?target=https://moviebox.id/api/play/12345&referer=https://moviebox.id
```

**Proxy a KissKH subtitle:**
```
https://hue-universal-proxy.your-subdomain.workers.dev/?target=https://kisskh.ovh/Sub/12345.vtt
```

**Optimized TMDB poster:**
```
https://hue-universal-proxy.your-subdomain.workers.dev/?target=https://image.tmdb.org/t/p/w500/abc.jpg&w=300
```

## Architecture Decision

| Concern | Node.js proxyServer.js | This Worker |
|---|---|---|
| CPU/RAM | Runs on your VPS (costs resources) | Runs on Cloudflare edge (free tier) |
| Segment Caching | In-memory Map (lost on restart) | Cloudflare Cache API (persistent, global) |
| Latency | Round-trip to your VPS | Serves from nearest edge POP |
| M3U8 Rewrite | Full rewrite with base64 | Full rewrite with base64 |
| Seek/Range | Supported | Supported |
| Image Optimization | None | Built-in via weserv.nl |
| Fallback Mirrors | None | KV-driven auto-retry |

**Recommended hybrid:** Use this Worker for all outbound provider proxying, image optimization, and subtitle CORS. Keep your Node.js `proxyServer.js` only if you need its HLS download cache for large DVR/offline scenarios.

## Free Tier Limits

Cloudflare Workers free tier: **100,000 requests/day**.
For a streaming app this is usually enough for metadata + playlist proxying. If you serve millions of `.ts` segments daily, consider:
1. Caching segments longer at the edge (already done — 24h TTL)
2. Upgrading to Workers Paid ($5/10M requests)
3. Keeping high-volume segment proxying on your VPS and using the Worker only for playlists / auth-heavy requests
