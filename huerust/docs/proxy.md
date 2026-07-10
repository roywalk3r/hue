# Proxy

The Rust backend proxies HLS manifests, TS segments, and subtitle files
through encrypted URLs so the client never learns the upstream origin
and can never tamper with the request.

## Endpoints

```
GET /m3u8-proxy?h_token=...     (HLS manifests, .m3u8 + .m3u8 + variant)
GET /ts-proxy?h_token=...       (TS / MP4 / MKV / .aac segments)
GET /sub-proxy?h_token=...      (subtitle files: .vtt, .srt, .ass)
GET /api/proxy?url=...&headers= (generic, untrusted caller passthrough)
```

The first three are zero-knowledge from the client's perspective:
the upstream URL and any required headers are encoded inside the
`h_token` HMAC signature. The client just hands the proxy a token
and gets back the proxied response.

`/api/proxy` is the only proxy route that takes the URL as a plain
query parameter. It is used by the Node.js backend and admin tooling
to fetch arbitrary upstreams through the same auth/CORS/rate-limit
envelope.

## Token format

`h_token` is a single URL-encoded string of the form
`<base64url-payload>.<hex-hmac>`.

The payload is JSON:

```json
{
  "url":   "https://vixsrc.to/playlist/12345",
  "exp":   1735689600,
  "hdrs":  { "Referer": "https://vixsrc.to/" }
}
```

- `url` — the upstream origin.
- `exp` — unix-seconds expiry. `proxy::mod::verify_h_token` rejects
  any token whose `exp` is in the past.
- `hdrs` — optional per-host headers to inject on the upstream
  request. The proxy can also inject *implicit* headers (see
  "CDN header injection" below); `hdrs` is for the *explicit*
  ones the upstream extractor told us to send.

The HMAC is `HMAC-SHA256(secret, base64url-payload)`. The secret is
`APP_SECRET` (env var).

### `sign_proxy_url`

`security::sign_proxy_url(url, expires_at, headers, secret)` is the
single entry point. It serializes the payload, signs it, and returns
a percent-encoded query value. `routes/streams.rs::streams_handler`
and `routes/streams.rs::xyra_streams_handler` are the two main
callers.

### `dynamic_proxy_base`

`proxy::mod::dynamic_proxy_base(request_headers, fallback)` decides
what `Host` to embed in `h_token` rewrites:

- If the request came in on the public origin (matched against
  `RUST_PROXY_URL`), use that origin.
- If the request came in on loopback (`127.0.0.1:24609`), use the
  fallback (which is `RUST_PROXY_URL` in production, or the
  loopback origin in dev).

This is what stops the server from accidentally embedding
`127.0.0.1:24609` into h_tokens that the browser will then fail to
resolve from a public origin.

## CDN header injection

Upstreams that check `Referer` or `User-Agent` get those headers
injected *implicitly* on the proxy request, based on the upstream
host:

| Upstream host(s)            | Injected headers                                                |
|-----------------------------|------------------------------------------------------------------|
| `vixsrc.to`                 | `Referer: https://vixsrc.to/`, `User-Agent: <browser-default>`  |
| `vix-content.net`           | `Referer: https://vixsrc.to/`, `User-Agent: <browser-default>`  |
| `hakunaymatata.*`           | `Referer: https://moviebox.ph/`, `User-Agent: <browser-default>`|
| `aisports.*`                | `Referer: https://moviebox.ph/`, `User-Agent: <browser-default>`|
| `poocloud.*`                | `Referer: https://moviebox.ph/`, `User-Agent: <browser-default>`|
| `mewstream.*`               | `Referer: https://hianimes.se/`, `User-Agent: <browser-default>`|
| `animeplay.*`               | `Referer: https://hianimes.se/`, `User-Agent: <browser-default>`|

Anything not in the table is fetched with the browser's natural
`User-Agent` and no `Referer`. Explicit headers from the upstream
extractor (the `hdrs` field in the payload) are merged on top.

## Host whitelist (raw URL bypass)

Some CDNs IP-block datacenter egress — proxying through our server
guarantees a 403. For those, the proxy is bypassed entirely and
the upstream URL is returned as a plain HTTP redirect so the client
fetches it directly.

`proxy::mod::is_whitelisted_raw_url(url)` returns true for:

- `*.moviebox.*` (MovieBox CDN)
- `pixeldrain.com` (Pixeldrain)
- `*.googleusercontent.com` (Google User Content)
- `promoxy.xyra.stream` (xyra's own egress proxy)

When the upstream URL matches, `wrap_with_proxy` skips the
`h_token` wrapping entirely and returns the bare URL to the
client, plus a `Referer` hint header. The client must
fetch directly.

## Manifest rewriting

`/m3u8-proxy` rewrites the manifest before responding:

1. **Relative → absolute** — `foo.ts` becomes the full
   `https://upstream/foo.ts`. The browser needs absolute URLs to
   fetch segments.
2. **Segment URLs re-wrapped** — each segment URL becomes another
   `h_token`-encrypted `/ts-proxy?h_token=...` (or a raw URL if the
   host is whitelisted). The browser's segment fetcher doesn't
   need to know which mode the segment is in; the URL just works.
3. **Variant playlist recursion** — variant playlists reference
   other `.m3u8` files. Those are re-wrapped recursively until a
   media manifest is reached.
4. **Optional ad surgery** — `proxy::adsurgery` strips ad segments
   when the extractor is confident they're ads (currently disabled
   for the streams endpoint; enabled on the Player's own fetch path).
5. **AES-128 key handling** — HLS `EXT-X-KEY` URIs are re-wrapped
   through `/api/proxy` with the upstream's `Referer` injected.

## TS sanitization

`proxy::sanitize` (when enabled) drops garbage NALU packets and
non-video PES frames from `.ts` segments. This is the
`SANITIZE_TS=true` knob — left off in production because the
players handle errors fine and the overhead is non-zero.

## Segment prefetch

After a manifest is rewritten, the handler spawns a background
task to fetch the first 5 segments. The point is to warm the
proxy cache so the client's first `play()` click is instant.
Prefetch failures are silently dropped.

## Auth & rate limit

- All proxy routes are **exempt** from the API Bearer-token check
  (they self-authenticate via `h_token`).
- All proxy routes are **exempt** from the IP rate limiter.
- `/api/proxy` is rate-limited at 30 req/s per IP (lower than the
  API limit) to discourage abuse — it can be used to fetch any
  arbitrary URL through the server.

## Operational notes

- `APP_SECRET` is the single secret. Changing it invalidates
  every `h_token` in flight. Don't rotate it casually.
- `RUST_PROXY_URL` must be set to the public Rust origin in
  production, or `dynamic_proxy_base` will embed `127.0.0.1:24609`
  in every h_token and the browser will fail to resolve them.
- The 12 s proxy timeout per request comes from the underlying
  `reqwest::Client` `connect_timeout` and `timeout` settings,
  not from the `h_token` expiry. Tokens can outlive a single
  request (default 1 hour).
