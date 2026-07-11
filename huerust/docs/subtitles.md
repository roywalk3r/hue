# Subtitles

Subtitles travel through three different paths depending on
the source.

## Path 1: KissKH (Zenith) embedded subtitles

Zenith (KissKH) returns subtitle URLs alongside the video
stream. The URL is a CDN-served `.vtt` or `.srt` file
that requires a `Referer: https://kisskh.co/` header.

The Player receives the subtitle URL in the
`StreamResult.subtitles: [...]` field. When the Player
loads the URL directly, the browser does not send the
required Referer. So:

1. The Rust API embeds the subtitle URL into an
   `h_token`-encrypted `/sub-proxy?h_token=...` URL.
2. The proxy sees the explicit `Referer` header inside
   the token payload (`hdrs` field) and replays it.
   KissKH's CDN host is **not** in the implicit-injection
   table — the Referer comes from the upstream extractor.
3. The Player fetches the proxy URL and gets the
   subtitle bytes.

## Path 2: Vidrock subtitles (vdrk_subtitles)

`src/providers/vdrk_subtitles.rs` is a separate pipeline
that fetches subtitles from Vidrock's subtitle service
for the **main aggregator's** movie/TV streams. This
catches the case where a stream came from Luna (Vidrock
video) but the original source had no subtitles, or the
upstream subtitle URL had expired.

Flow:

```
xyra_streams_handler  /  streams_handler
        │
        ▼
wrap_with_proxy(...)  — primary pipeline
        │
        ▼
vdrk_subtitles.fetch(...)  — secondary pipeline, best-effort
        │
        ▼
merge subtitles[] into StreamResult
```

The vdrk pipeline runs in parallel with the main fetch
and merges results on completion. Failures are silent
(no subtitles is a valid result).

## Path 3: Xyra subtitle lookup

`src/providers/xyra_subtitle.rs` is a disabled pipeline
that would have used the xyra streamhub's subtitle
search endpoint. Disabled because the xyra upstream
stopped returning usable results; the in-line subtitle
URLs from KissKH and the vdrk fallback cover the
common cases.

## Subtitle routes (disabled)

```
/api/subtitle/search     — search by tmdb_id, season, episode
/api/subtitle/download   — download by xyra subtitle id
```

Both routes are registered as modules in
`src/routes/subtitle.rs` but commented out in
`build_router` (see `src/app.rs` for the
`// Xyra subtitle routes disabled — xyra faulty`
block). Re-enable by uncommenting the `.route(...)`
lines.

## Format support

The proxy responds with subtitles in the upstream's
native format (`.vtt`, `.srt`, `.ass`). The Player
auto-detects by file extension. The proxy does not
transcode between formats.

## CDN header injection

Subtitle CDNs get their `Referer` from the upstream
extractor (encoded in the `h_token` payload), **not** from
the proxy's implicit-injection table. The implicit table
in `src/proxy/mod.rs::inject_cdn_headers` only covers
video-segment hosts:

| Host pattern | Injected `Referer` |
|---|---|
| `vixsrc.to` | `https://vixsrc.to/` |
| `hakunaymatata.com` / `aisports.mobi` / `poocloud.in` | `https://moviebox.ph/` |
| `mewstream.buzz` / `animeplay.cfd` | `https://hianimes.se/` |

If a new upstream returns a subtitle URL with a
`Referer` requirement, the extractor (`src/clients/*.rs`)
must include the `Referer` header in the StreamResult's
`headers` map — the wrap pipeline will then bake it
into the `h_token` payload.

## Operations

- If a new upstream returns subtitle URLs with a
  `Referer` requirement, the upstream extractor
  must include the `Referer` in the StreamResult
  headers map (the `headers: {Referer: ...}` field
  on `StreamResult`). The wrap pipeline will then
  encode it into the `h_token` payload. No proxy
  or client changes needed.
- The vdrk fallback works best for English-language
  content. CJK subtitles need KissKH or a manual
  upload.
- Subtitle URLs are short-lived (typically 1-4
  hours). The Player re-fetches on each play
  rather than caching across sessions.
