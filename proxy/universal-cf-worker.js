/**
 * Hue Universal Streaming Proxy — Cloudflare Worker
 *
 * Handles: generic provider proxying, M3U8 playlist rewriting, subtitle CORS,
 * image optimization, edge caching, provider fallback routing, geo-based rules,
 * and range-request passthrough for video segments.
 *
 * Deploy: https://dash.cloudflare.com → Workers & Pages → Create Service
 * Bindings required (optional):
 *   - KV namespace "PROVIDER_MIRRORS"   (provider fallback map)
 *   - KV namespace "EDGE_CACHE_CONFIG"  (TTL overrides per provider)
 *
 * Environment variables (Secrets):
 *   - ALLOWED_ORIGINS   (comma-separated, e.g. "https://hue.app,https://yourdomain.com")
 *   - BLOCKED_COUNTRIES (comma-separated ISO codes, e.g. "CN,RU")
 *   - IMAGE_OPTIMIZATION ("true" to route images through weserv.nl)
 *   - DEFAULT_REFERER   (fallback referer for provider requests)
 *
 * Usage from Hue frontend / Rust backend:
 *   https://your-worker.workers.dev/?target=<url>&referer=<ref>&origin=<origin>
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @license MIT
 */

// ─────────────────────────────────────────────────────────────────────────────
// Constants
// ─────────────────────────────────────────────────────────────────────────────
const DEFAULT_UA = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36';
const BLOCKED_HOSTS = ['localhost', '127.0.0.1', '0.0.0.0', '::1'];
const SUBTITLE_EXTS = ['.vtt', '.srt', '.ass', '.ssa'];
const IMAGE_EXTS = ['.jpg', '.jpeg', '.png', '.gif', '.webp', '.bmp', '.ico'];
const VIDEO_EXTS = ['.ts', '.m4s', '.mp4', '.mkv', '.webm'];
const HLS_EXTS = ['.m3u8', '.m3u'];

// Cache TTLs (seconds)
const CACHE_TTL = {
  m3u8: 30,           // Playlists change frequently
  segment: 86400,     // Segments are immutable
  subtitle: 3600,     // Subtitles rarely change
  image: 604800,      // Posters/images are static
  default: 60,
};

// Provider fallback mirror map (fallback if KV not bound)
const MIRROR_FALLBACK = {
  'moviebox': ['https://moviebox.id', 'https://themoviebox.org', 'https://netfilm.world'],
  'kisskh': ['https://kisskh.ovh'],
  'dramacool': ['https://dramacool.com.pa'],
};

// ─────────────────────────────────────────────────────────────────────────────
// Main Fetch Handler
// ─────────────────────────────────────────────────────────────────────────────
export default {
  async fetch(request, env, ctx) {
    const url = new URL(request.url);

    // ── 1. CORS Preflight ───────────────────────────────────────────────────
    if (request.method === 'OPTIONS') {
      return handleCorsPreflight(request, env);
    }

    // ── 2. Geo Checks ───────────────────────────────────────────────────────
    const country = request.cf?.country || 'XX';
    if (isBlockedCountry(country, env)) {
      return jsonResponse({ error: 'Service not available in your region' }, 403);
    }

    // ── 3. Parse Target ─────────────────────────────────────────────────────
    const targetRaw = url.searchParams.get('target')
      || url.searchParams.get('url')
      || url.searchParams.get('u');

    if (!targetRaw) {
      return jsonResponse({
        error: 'Missing target parameter',
        usage: 'GET ?target=<url>&referer=<ref>&origin=<origin>&userAgent=<ua>&cache=<true|false>'
      }, 400);
    }

    let targetUrl;
    try {
      targetUrl = new URL(targetRaw);
    } catch {
      return jsonResponse({ error: 'Invalid target URL' }, 400);
    }

    // ── 4. Security: Block Internal IPs ─────────────────────────────────────
    if (isBlockedHost(targetUrl.hostname)) {
      return jsonResponse({ error: 'Blocked internal target' }, 403);
    }

    // ── 5. Image Optimization Shortcut ──────────────────────────────────────
    if (isImageRequest(targetUrl) && env.IMAGE_OPTIMIZATION === 'true') {
      return proxyImageOptimized(targetUrl, request, env);
    }

    // ── 6. Provider Fallback Routing (KV or static map) ───────────────────────
    const providerKey = detectProvider(targetUrl.hostname);
    if (providerKey) {
      const mirrorResult = await tryProviderMirrors(
        providerKey, targetUrl, request, env, ctx
      );
      if (mirrorResult) return mirrorResult;
    }

    // ── 7. Execute Proxy ────────────────────────────────────────────────────
    return executeProxy(targetUrl, request, env, ctx);
  },
};

// ─────────────────────────────────────────────────────────────────────────────
// Core Proxy Logic
// ─────────────────────────────────────────────────────────────────────────────
async function executeProxy(targetUrl, request, env, ctx, attempt = 0) {
  const url = new URL(request.url);

  // Build headers
  const headers = new Headers(request.headers);

  // Strip hop-by-hop / problematic headers
  ['host', 'connection', 'content-length', 'accept-encoding'].forEach(h => {
    headers.delete(h);
  });

  // Inject configurable headers from query params, but preserve incoming headers if present
  const incomingReferer = headers.get('referer') || headers.get('Referer');
  const incomingOrigin = headers.get('origin') || headers.get('Origin');
  const incomingUa = headers.get('user-agent') || headers.get('User-Agent');

  const referer = incomingReferer
    || url.searchParams.get('referer')
    || url.searchParams.get('ref')
    || env.DEFAULT_REFERER
    || targetUrl.origin;
  const origin = incomingOrigin || url.searchParams.get('origin') || referer;
  const userAgent = incomingUa || url.searchParams.get('userAgent') || url.searchParams.get('ua') || DEFAULT_UA;

  headers.set('Referer', referer);
  headers.set('Origin', origin);
  headers.set('User-Agent', userAgent);

  // Preserve range headers for video segments
  const range = request.headers.get('Range');
  if (range) {
    headers.set('Range', range);
  }

  // ── Try Cache First ───────────────────────────────────────────────────────
  const cacheKey = buildCacheKey(targetUrl, range);
  const cache = caches.default;
  const isGet = request.method === 'GET';
  const shouldCache = isGet && url.searchParams.get('cache') !== 'false';

  if (shouldCache) {
    const cached = await cache.match(cacheKey);
    if (cached) {
      const resp = new Response(cached.body, {
        status: cached.status,
        statusText: cached.statusText,
        headers: injectCors(cached.headers, env),
      });
      return resp;
    }
  }

  // ── Forward Request ───────────────────────────────────────────────────────
  const upstream = await fetch(targetUrl.toString(), {
    method: request.method,
    headers,
    body: request.method !== 'GET' && request.method !== 'HEAD' ? request.body : null,
    redirect: 'follow',
  });

  // ── Retry on specific errors (5xx / 403) if provider mirrors exist ────────
  if ((upstream.status >= 500 || upstream.status === 403) && attempt < 2) {
    const providerKey = detectProvider(targetUrl.hostname);
    if (providerKey) {
      const nextMirror = await getNextMirror(providerKey, targetUrl, env, attempt);
      if (nextMirror) {
        return executeProxy(nextMirror, request, env, ctx, attempt + 1);
      }
    }
  }

  // ── Build Response ──────────────────────────────────────────────────────
  const responseHeaders = new Headers(upstream.headers);

  // Detect content type
  const contentType = (responseHeaders.get('content-type') || '').toLowerCase();
  const pathLower = targetUrl.pathname.toLowerCase();
  const isM3U8 = HLS_EXTS.some(ext => pathLower.endsWith(ext)) || contentType.includes('mpegurl');
  const isSubtitle = SUBTITLE_EXTS.some(ext => pathLower.endsWith(ext));
  const isSegment = VIDEO_EXTS.some(ext => pathLower.endsWith(ext));
  const isImage = IMAGE_EXTS.some(ext => pathLower.endsWith(ext));

  let body = upstream.body;
  let status = upstream.status;

  // ── M3U8 Rewriting ────────────────────────────────────────────────────────
  if (isM3U8 && upstream.ok) {
    const text = await upstream.text();
    const rewritten = rewriteM3U8(text, targetUrl, url);
    body = rewritten;
    responseHeaders.set('Content-Type', 'application/vnd.apple.mpegurl');
  }

  // ── Base64-Encoded Segment Detection ──────────────────────────────────────
  if (isSegment && upstream.ok && !isM3U8) {
    const buffer = await upstream.arrayBuffer();
    const view = new Uint8Array(buffer);
    // MPEG-TS starts with 0x47; JPEG with 0xFF 0xD8
    const isTs = view[0] === 0x47;
    const isJpeg = view[0] === 0xFF && view[1] === 0xD8;
    if (!isTs && !isJpeg && view.length > 100) {
      // Might be base64-encoded binary
      const text = new TextDecoder().decode(view.slice(0, 200)).trim();
      if (/^[A-Za-z0-9+/=\s]+$/.test(text) && text.length > 100) {
        const fullText = new TextDecoder().decode(view).trim().replace(/[\r\n\s]/g, '');
        if (/^[A-Za-z0-9+/=]+$/.test(fullText) && fullText.length > 100) {
          try {
            const decoded = base64ToArrayBuffer(fullText);
            body = decoded;
            const decodedView = new Uint8Array(decoded);
            if (decodedView[0] === 0x47) {
              responseHeaders.set('Content-Type', 'video/mp2t');
            }
          } catch {
            body = buffer;
          }
        } else {
          body = buffer;
        }
      } else {
        body = buffer;
      }
    } else {
      body = buffer;
      if (isTs) responseHeaders.set('Content-Type', 'video/mp2t');
    }
  }

  // ── Set proper content-type for subtitles ─────────────────────────────────
  if (isSubtitle) {
    if (pathLower.endsWith('.vtt')) responseHeaders.set('Content-Type', 'text/vtt');
    if (pathLower.endsWith('.srt')) responseHeaders.set('Content-Type', 'text/plain');
    if (pathLower.endsWith('.ass') || pathLower.endsWith('.ssa')) responseHeaders.set('Content-Type', 'text/plain');
  }

  // ── Inject CORS ───────────────────────────────────────────────────────────
  injectCors(responseHeaders, env);

  const response = new Response(body, {
    status,
    statusText: upstream.statusText,
    headers: responseHeaders,
  });

  // ── Store in Edge Cache ─────────────────────────────────────────────────
  if (shouldCache && upstream.ok) {
    const ttl = resolveCacheTtl(isM3U8, isSegment, isSubtitle, isImage);
    ctx.waitUntil(cache.put(cacheKey, response.clone()));
    responseHeaders.set('Cache-Control', `public, max-age=${ttl}`);
    responseHeaders.set('CF-Cache-Status', 'MISS');
  }

  return response;
}

// ─────────────────────────────────────────────────────────────────────────────
// M3U8 Rewriting
// ─────────────────────────────────────────────────────────────────────────────
function rewriteM3U8(text, targetUrl, workerUrl) {
  const workerBase = `${workerUrl.origin}${workerUrl.pathname}`;
  const lines = text.split('\n');

  return lines.map(line => {
    line = line.trim();
    if (!line || line.startsWith('#EXT-X-ENDLIST')) return line;

    // Rewrite KEY and MAP URIs
    if (line.startsWith('#')) {
      if (line.startsWith('#EXT-X-KEY') || line.startsWith('#EXT-X-MAP')) {
        return line.replace(/URI=["']([^"']+)["']/g, (match, uri) => {
          try {
            const absolute = new URL(uri, targetUrl.href).href;
            const proxied = buildProxyUrl(workerBase, absolute, targetUrl);
            return `URI="${proxied}"`;
          } catch {
            return match;
          }
        });
      }
      return line;
    }

    // Rewrite segment / relative URLs
    try {
      const absolute = new URL(line, targetUrl.href).href;
      return buildProxyUrl(workerBase, absolute, targetUrl);
    } catch {
      return line;
    }
  }).join('\n');
}

function buildProxyUrl(workerBase, target, originalTarget) {
  const params = new URLSearchParams();
  params.set('target', target);
  // Preserve referer/origin from original request context
  params.set('referer', originalTarget.origin);
  params.set('origin', originalTarget.origin);
  return `${workerBase}?${params.toString()}`;
}

// ─────────────────────────────────────────────────────────────────────────────
// Image Optimization via weserv.nl
// ─────────────────────────────────────────────────────────────────────────────
function proxyImageOptimized(targetUrl, request, env) {
  const url = new URL(request.url);
  const weserv = new URL('https://images.weserv.nl');
  weserv.searchParams.set('url', targetUrl.href);

  // Client hints for format
  const accept = request.headers.get('Accept') || '';
  if (accept.includes('image/avif')) {
    weserv.searchParams.set('output', 'avif');
  } else if (accept.includes('image/webp')) {
    weserv.searchParams.set('output', 'webp');
  }

  // Width hint
  const width = url.searchParams.get('w') || url.searchParams.get('width');
  if (width) {
    weserv.searchParams.set('w', width);
  }

  return fetch(weserv.toString(), {
    headers: { 'User-Agent': request.headers.get('User-Agent') || DEFAULT_UA },
  }).then(resp => {
    const headers = new Headers(resp.headers);
    injectCors(headers, env);
    headers.set('Cache-Control', 'public, max-age=604800');
    return new Response(resp.body, { status: resp.status, headers });
  });
}

// ─────────────────────────────────────────────────────────────────────────────
// Provider Fallback / Mirror Routing
// ─────────────────────────────────────────────────────────────────────────────
async function tryProviderMirrors(providerKey, targetUrl, request, env, ctx) {
  const mirrors = await getMirrors(providerKey, env);
  if (!mirrors || mirrors.length <= 1) return null;

  for (let i = 0; i < mirrors.length; i++) {
    const mirrorBase = mirrors[i];
    const mirrorUrl = new URL(targetUrl.pathname + targetUrl.search, mirrorBase);
    try {
      const resp = await executeProxy(mirrorUrl, request, env, ctx, i);
      if (resp.ok) return resp;
    } catch {
      continue;
    }
  }
  return null;
}

async function getMirrors(providerKey, env) {
  if (env.PROVIDER_MIRRORS) {
    const kv = await env.PROVIDER_MIRRORS.get(providerKey);
    if (kv) {
      try { return JSON.parse(kv); } catch { /* fallthrough */ }
    }
  }
  return MIRROR_FALLBACK[providerKey] || null;
}

async function getNextMirror(providerKey, targetUrl, env, attempt) {
  const mirrors = await getMirrors(providerKey, env);
  if (!mirrors || attempt >= mirrors.length) return null;
  return new URL(targetUrl.pathname + targetUrl.search, mirrors[attempt]);
}

function detectProvider(hostname) {
  const h = hostname.toLowerCase();
  if (h.includes('moviebox') || h.includes('movibox') || h.includes('netfilm.world')) return 'moviebox';
  if (h.includes('kisskh')) return 'kisskh';
  if (h.includes('dramacool')) return 'dramacool';
  return null;
}

// ─────────────────────────────────────────────────────────────────────────────
// Cache Helpers
// ─────────────────────────────────────────────────────────────────────────────
function buildCacheKey(targetUrl, range) {
  const key = range ? `${targetUrl.href}|${range}` : targetUrl.href;
  return new Request(key);
}

function resolveCacheTtl(isM3U8, isSegment, isSubtitle, isImage) {
  if (isM3U8) return CACHE_TTL.m3u8;
  if (isSegment) return CACHE_TTL.segment;
  if (isSubtitle) return CACHE_TTL.subtitle;
  if (isImage) return CACHE_TTL.image;
  return CACHE_TTL.default;
}

// ─────────────────────────────────────────────────────────────────────────────
// CORS
// ─────────────────────────────────────────────────────────────────────────────
function handleCorsPreflight(request, env) {
  const origin = request.headers.get('Origin') || '*';
  const allowed = resolveAllowedOrigin(origin, env);
  return new Response(null, {
    status: 204,
    headers: {
      'Access-Control-Allow-Origin': allowed,
      'Access-Control-Allow-Methods': 'GET, POST, OPTIONS, HEAD',
      'Access-Control-Allow-Headers': 'Content-Type, Authorization, X-Requested-With, Range',
      'Access-Control-Expose-Headers': 'Content-Length, Content-Range, Accept-Ranges',
      'Access-Control-Max-Age': '86400',
    },
  });
}

function injectCors(headers, env) {
  const origin = headers.get('Origin') || '*';
  headers.set('Access-Control-Allow-Origin', resolveAllowedOrigin(origin, env));
  headers.set('Access-Control-Allow-Methods', 'GET, POST, OPTIONS, HEAD');
  headers.set('Access-Control-Allow-Headers', 'Content-Type, Authorization, X-Requested-With, Range');
  headers.set('Access-Control-Expose-Headers', 'Content-Length, Content-Range, Accept-Ranges');
  return headers;
}

function resolveAllowedOrigin(requestOrigin, env) {
  if (!env.ALLOWED_ORIGINS) return '*';
  const allowed = env.ALLOWED_ORIGINS.split(',').map(s => s.trim());
  if (allowed.includes('*')) return '*';
  if (allowed.includes(requestOrigin)) return requestOrigin;
  return allowed[0] || '*';
}

function isBlockedCountry(country, env) {
  if (!env.BLOCKED_COUNTRIES) return false;
  const blocked = env.BLOCKED_COUNTRIES.split(',').map(s => s.trim().toUpperCase());
  return blocked.includes(country.toUpperCase());
}

function isBlockedHost(hostname) {
  const h = hostname.toLowerCase();
  return BLOCKED_HOSTS.includes(h) || h.startsWith('192.168.') || h.startsWith('10.');
}

function isImageRequest(targetUrl) {
  return IMAGE_EXTS.some(ext => targetUrl.pathname.toLowerCase().endsWith(ext));
}

// ─────────────────────────────────────────────────────────────────────────────
// Utilities
// ─────────────────────────────────────────────────────────────────────────────
function jsonResponse(obj, status = 200) {
  return new Response(JSON.stringify(obj, null, 2), {
    status,
    headers: { 'Content-Type': 'application/json' },
  });
}

function base64ToArrayBuffer(base64) {
  const binaryString = atob(base64);
  const bytes = new Uint8Array(binaryString.length);
  for (let i = 0; i < binaryString.length; i++) {
    bytes[i] = binaryString.charCodeAt(i);
  }
  return bytes.buffer;
}
