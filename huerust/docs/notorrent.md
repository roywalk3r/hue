# NoTorrent provider and the fake-iframe filter

`src/providers/notorrent_provider.rs` (display name: **NoTorrent**,
real_name: `notorrent`) is an anime source that lives on the
hianime hosting stack. It is one of the few anime providers
that returns actual HLS streams rather than iframes.

It is also one of the few providers that ships with a
**fake-iframe filter** baked into the client.

## What the fake-iframe filter does

Several anime scraping sites return URLs that look like
video streams but are actually HTML pages containing
embedded ads, or HLS manifests whose segments are
ad HTML pages. NoTorrent's client filters them at the
provider level so they never reach the Player.

The actual filter (from `src/clients/notorrent.rs`):

```rust
if url.contains("vid1.php?url=")
    || url.contains("vid2.php?url=")
    || url.contains("/vid/series/")
    || url.contains("/vid/movie/")
    // 321moviesfree "streams" point at HTML detail pages, not media
    || url.contains("321moviesfree.com/detail")
    // p.111477.xyz serves tiny range probes but 429s every real
    // transfer — passes reachability checks, then never plays
    || url.contains("p.111477.xyz/bulk")
{
    continue;
}
```

What each pattern blocks:

1. `vid1.php?url=` / `vid2.php?url=` — PHP "stream extractor"
   pages on `hostingersite` / `whpaste` style scrapers. Return
   a JSON with a `url` field, but the URL is itself an HLS
   manifest whose segments are ad HTML pages (e.g.
   `page-N.html?token=?token=` on `scalableimpactgroup.site`).
2. `/vid/series/` / `/vid/movie/` — Same family of scrapers,
   different URL pattern.
3. `321moviesfree.com/detail` — Returns HTML detail pages, not
   media segments.
4. `p.111477.xyz/bulk` — Passes the Player's reachability check
   (returns a tiny 200 response) but 429s on real segment
   transfers, so the stream never plays.

The filter is conservative: it only matches the specific
URL substrings we have observed. Adding a new fake-iframe
pattern means editing the source and rebuilding.

## Provider registration

NoTorrent is registered in the main provider
registry (`src/app.rs`) and participates in the
fast phase. It is fast (8 s timeout) because the
upstream response is usually a single JSON
payload of CDN URLs.

## Why a provider-level filter and not a global one?

The fake-iframe patterns are NoTorrent-specific.
The other providers (VidZee, Luna, Vixsrc) do not
emit URLs in this shape, so a global filter would
add maintenance burden with no coverage benefit.

A global "deny list" of bad CDN hosts exists in
`src/filters.rs` for known-bad upstreams (ad
networks, scam sites, etc.) — that's a different
mechanism for a different problem.

## Failure modes

- If the upstream rotates to a new fake-iframe
  pattern, the filter will miss it. The Player
  will then try to play the fake playlist and
  fail with a `manifestParsingError`. Add the
  new pattern to `is_fake_iframe`.
- If a legitimate CDN path starts containing
  `vid1.php` or `hostingersite.com` (very
  unlikely), the filter would drop a real stream.
  In that case, narrow the filter to be more
  specific.

## Operations

- The filter currently matches: `vid1.php?url=`, `vid2.php?url=`,
  `/vid/series/`, `/vid/movie/`, `321moviesfree.com/detail`,
  `p.111477.xyz/bulk`. Add new patterns by editing the source.
- NoTorrent is currently always-on. There is no
  `ENABLE_NOTORRENT` flag. If the upstream goes
  down, the circuit breaker will open after 5
  consecutive failures; if it stays down, the
  provider will be skipped automatically.
