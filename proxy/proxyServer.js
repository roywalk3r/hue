// Lightweight proxy/m3u8/segment/subtitle rewriting layer. thanks to https://github.com/cinepro-org/ for the original code.
// Adapted from external project (user supplied) to fit CommonJS style and existing config system.
// Provides conditional route registration via enableProxy flag.

const cors = require('cors');
const fetch = require('node-fetch');
const http = require('http');
const https = require('https');
const { encryptUrl } = require('../utils/security');
let cryptoJs;

// Shared agents with optimized TLS and keepAlive settings
const httpAgent = new http.Agent({
    keepAlive: true, 
    maxSockets: 150, 
    timeout: 60000,
    freeSocketTimeout: 30000 
});
const httpsAgent = new https.Agent({
    keepAlive: true, 
    maxSockets: 150, 
    timeout: 60000,
    freeSocketTimeout: 30000
});

// IPv4 Forced Agents for bypassing VPS routing issues
const httpAgentV4 = new http.Agent({ family: 4, keepAlive: true, maxSockets: 100 });
const httpsAgentV4 = new https.Agent({ family: 4, keepAlive: true, maxSockets: 100 });

const DEFAULT_UA = 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36';
const FALLBACK_UA = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36';

// Retry configuration for fetch operations
const MAX_RETRIES = 2;
const RETRY_DELAY_MS = 500;
const FETCH_TIMEOUT_MS = 60000; 

// Helper function to fetch with retry logic
async function fetchWithRetry(url, options = {}, maxRetries = MAX_RETRIES) {
    let host = '';
    const isSegment = /\.(ts|m4s|mp4|mkv|m4v|m4a|fmp4)(\?|$)/i.test(url) || url.includes('/resource/') || url.includes('/bt/');
    
    try {
        const u = new URL(url);
        host = u.hostname.toLowerCase();
        if (host === 'localhost' || host === '127.0.0.1' || host === '0.0.0.0' || host.startsWith('192.168.') || host.startsWith('10.')) {
            throw new Error(`Blocked internal/local URL: ${url}`);
        }
    } catch(e) {
        if (e.message.includes('Blocked')) throw e;
    }

    const hostileHosts = ['hakunaymatata', 'quibble', 'vidhide', 'cdnvideo', 'stream', 'vixsrc', 'wyzie', 'rabbitstream', 'vidbox', 'lightningbolt'];
    const isHostile = hostileHosts.some(h => host.includes(h));
    
    let baseTimeout = FETCH_TIMEOUT_MS;
    if (isSegment) baseTimeout = 120000;
    else if (isHostile) baseTimeout = 60000;

    const finalTimeout = Math.max(baseTimeout, options.timeout || 0);
    
    const mergedHeaders = { ...(options.headers || {}) };
    if (isHostile) {
        mergedHeaders['Connection'] = 'close';
    }

    const actualRetries = isSegment ? 1 : maxRetries;
    const forceIPv4 = isHostile || host.includes('redirector') || host.includes('cdnsync');

    const fetchOptions = {
        ...options,
        headers: mergedHeaders,
        agent: options.agent || (isHostile ? false : (forceIPv4 
            ? ((parsedUrl) => (parsedUrl.protocol === 'http:' ? httpAgentV4 : httpsAgentV4))
            : ((parsedUrl) => (parsedUrl.protocol === 'http:' ? httpAgent : httpsAgent)))),
        compress: true 
    };
    
    let lastError;
    let useIPv4Forced = forceIPv4;

    for (let attempt = 1; attempt <= actualRetries; attempt++) {
        try {
            const t0 = Date.now();
            
            // On retry attempts, rotate User-Agent and potentially force IPv4
            if (attempt > 1) {
                if (!fetchOptions.headers) fetchOptions.headers = {};
                fetchOptions.headers['User-Agent'] = attempt % 2 === 0 ? FALLBACK_UA : DEFAULT_UA;
                useIPv4Forced = true; // Force IPv4 on ANY retry
            }

            if (useIPv4Forced) {
                fetchOptions.agent = isHostile ? false : ((parsedUrl) => (parsedUrl.protocol === 'http:' ? httpAgentV4 : httpsAgentV4));
            }
            
            const res = await fetch(url, { ...fetchOptions, timeout: finalTimeout });
            const duration = Date.now() - t0;
            
            if (isSegment && duration > 10000) {
                console.log(`[fetch] Slow segment (${duration}ms): ${url}`);
            }
            
            return res;
        } catch (error) {
            lastError = error;
            const isNetworkError = error.code === 'ECONNRESET' || error.code === 'ECONNREFUSED' || error.code === 'ENOTFOUND' || error.code === 'ETIMEDOUT';
            
            if (error.name === 'AbortError' || error.code === 'ETIMEDOUT' || error.message.includes('timeout')) {
                console.warn(`[fetch] Timeout on ${url} (Attempt ${attempt}/${actualRetries}, timeout: ${finalTimeout}ms)`);
            } else {
                console.warn(`[fetch] Network error ${error.code || error.name} on ${url} (Attempt ${attempt}/${actualRetries})`);
            }

            if (attempt >= actualRetries) throw error;
            
            // If it was a network reset/refusal, force IPv4 for the next attempt immediately
            if (isNetworkError) useIPv4Forced = true;

            await new Promise(resolve => setTimeout(resolve, RETRY_DELAY_MS * attempt));
        }
    }
    throw lastError;
}

const CACHE_MAX_SIZE = 2000;
const CACHE_EXPIRY_MS = 2 * 60 * 60 * 1000; // 2 hours
const segmentCache = new Map();
// Track first open-ended (bytes=0-) range per target to clamp only once per TTL window
const openRangeClampMap = new Map();
const OPEN_RANGE_CLAMP_TTL_MS = 5 * 60 * 1000; // 5 minutes
// Progressive open-ended growth tracking: keeps expanding bytes=0- served window.
const progressiveOpenMap = new Map(); // url -> { lastEnd }
// Tail prefetch cache: stores last N KB for quick tail range responses
const tailPrefetchMap = new Map(); // url -> { data: Buffer, start: number, end: number, size: number, ts: number }
const TAIL_PREFETCH_TTL_MS = 10 * 60 * 1000; // 10 minutes

function isCacheDisabled() {
    return process.env.DISABLE_CACHE === 'true';
}

function cleanupCache() {
    const now = Date.now();
    for (const [url, entry] of segmentCache.entries()) {
        if (now - entry.timestamp > CACHE_EXPIRY_MS) segmentCache.delete(url);
    }
    if (segmentCache.size > CACHE_MAX_SIZE) {
        const entries = Array.from(segmentCache.entries()).sort((a, b) => a[1].timestamp - b[1].timestamp);
        const toRemove = entries.slice(0, segmentCache.size - CACHE_MAX_SIZE);
        toRemove.forEach(([u]) => segmentCache.delete(u));
    }
    return segmentCache.size;
}
setInterval(cleanupCache, 30 * 60 * 1000).unref();

function cleanupClampMap() {
    const now = Date.now();
    for (const [url, ts] of openRangeClampMap.entries()) {
        if (now - ts > OPEN_RANGE_CLAMP_TTL_MS) openRangeClampMap.delete(url);
    }
}
setInterval(cleanupClampMap, 10 * 60 * 1000).unref();

function cleanupTailPrefetch() {
    const now = Date.now();
    for (const [url, entry] of tailPrefetchMap.entries()) {
        if (now - entry.ts > TAIL_PREFETCH_TTL_MS) tailPrefetchMap.delete(url);
    }
}
setInterval(cleanupTailPrefetch, 15 * 60 * 1000).unref();

function getCachedSegment(url) {
    if (isCacheDisabled()) return null;
    const e = segmentCache.get(url);
    if (!e) return null;
    if (Date.now() - e.timestamp > CACHE_EXPIRY_MS) { segmentCache.delete(url); return null; }
    return e;
}

async function prefetchSegment(url, headers) {
    if (isCacheDisabled() || segmentCache.size >= CACHE_MAX_SIZE) return;
    const existing = segmentCache.get(url);
    if (existing && Date.now() - existing.timestamp <= CACHE_EXPIRY_MS) return;
    try {
        const resp = await fetch(url, { headers: { 'User-Agent': DEFAULT_UA, ...headers } });
        if (!resp.ok) return;
        const data = new Uint8Array(await resp.arrayBuffer());
        const responseHeaders = {};
        resp.headers.forEach((v, k) => responseHeaders[k] = v);
        segmentCache.set(url, { data, headers: responseHeaders, timestamp: Date.now() });
    } catch (_e) { /* ignore */ }
}

function inferContentType(upstreamCT, targetUrl) {
    const bad = !upstreamCT || /application\/octet-stream/i.test(upstreamCT) || /application\/(x-)?zip/i.test(upstreamCT) || /image\//i.test(upstreamCT);
    if (!bad) return upstreamCT;
    if (/\.mkv(\?|$)/i.test(targetUrl)) return 'video/x-matroska';
    if (/\.mp4(\?|$)/i.test(targetUrl)) return 'video/mp4';
    if (/\.m3u8(\?|$)/i.test(targetUrl)) return 'application/vnd.apple.mpegurl';
    if (/\.(ts|png|jpg|jpeg|bmp)(\?|$)/i.test(targetUrl)) return 'video/mp2t';
    return 'application/octet-stream';
}

function decodeVidzeeToken(token) {
    const debug = process.env.VIDZEE_DEBUG === '1';
    try {
        if (typeof token !== 'string') {
            if (debug) console.log('[VidZee] Token is not a string');
            return null;
        }
        if (/^https?:\/\//i.test(token.trim())) return null;
        // Skip obviously invalid tokens
        if (token.length < 20) {
            if (debug) console.log('[VidZee] Token too short');
            return null;
        }
        
        if (debug) {
            console.log('[VidZee] Token starts with:', token.substring(0, 50));
            console.log('[VidZee] Token contains spaces:', token.includes(' '));
        }
        
        // Normalize base64 - handle URL-safe and spaces
        let normalized = token
            .replace(/-/g, '+')
            .replace(/_/g, '/')
            .replace(/\s/g, '+') // Replace spaces with +
            .replace(/[^A-Za-z0-9+/=]/g, '');
        const pad = normalized.length % 4;
        const fixed = pad ? normalized + '='.repeat(4 - pad) : normalized;
        
        if (debug) {
            console.log('[VidZee] Fixed token starts with:', fixed.substring(0, 50));
            console.log('[VidZee] Fixed token length:', fixed.length);
        }
        
        let raw;
        let buffer;
        
        try {
            // First get the buffer from base64
            buffer = Buffer.from(fixed, 'base64');
            
            if (debug) {
                console.log('[VidZee] Buffer created, length:', buffer.length);
                console.log('[VidZee] Buffer first 20 bytes:', Array.from(buffer.slice(0, 20)).map(b => b.toString(16).padStart(2, '0')).join(''));
            }
            
            // Try multiple encoding strategies in order of preference
            try {
                // Try UTF-8 first (standard approach)
                raw = buffer.toString('utf8');
                if (debug) console.log('[VidZee] UTF-8 decode success, raw length:', raw.length);
            } catch (e) {
                try {
                    // Fall back to binary encoding if UTF-8 fails
                    raw = buffer.toString('binary');
                    if (debug) console.log('[VidZee] Binary decode used as fallback');
                } catch (e2) {
                    throw new Error('All string decoding attempts failed');
                }
            }
        } catch (e) {
            if (debug) console.log('[VidZee] Base64 decode failed:', e.message);
            return null;
        }

        if (!raw) return null;

        // Clean raw string from unprintable chars that might break URL parsing
        raw = raw.replace(/[\x00-\x1F\x7F-\x9F]/g, '').trim();

        if (debug) {
            console.log('[VidZee] Decoded raw string (first 100):', raw.substring(0, 100));
        }

        // Strategy 1: Look for URL inside JSON-like or quoted structure
        const urlRegex = /(https?:\/\/[^\s"'\\\}{\}\]\[<>]+)/i;
        const match = raw.match(urlRegex);
        
        if (match) {
            let finalUrl = match[1];
            // Remove any trailing garbage like \ or " or )
            finalUrl = finalUrl.split(/[\\"'<>{}()\[\]]/)[0];
            if (debug) console.log('[VidZee] Found URL in token:', finalUrl);
            return finalUrl;
        }

        return null;
    } catch (e) {
        if (debug) console.error('[VidZee] Token decode exception:', e.message);
        return null;
    }
}

function extractOriginalUrl(proxyUrl) {
    try {
        if (!proxyUrl || typeof proxyUrl !== 'string') return proxyUrl;
        
        let candidate = proxyUrl;
        // Recursively unwrap proxy URLs
        let depth = 0;
        const { decryptUrl } = require('../utils/security');

        while (depth < 10 && (candidate.includes('/m3u8-proxy') || candidate.includes('/ts-proxy') || candidate.includes('url=') || candidate.includes('token='))) {
            let next = null;
            try {
                const urlObj = new URL(candidate.startsWith('http') ? candidate : 'http://localhost' + (candidate.startsWith('/') ? '' : '/') + candidate);
                
                // Handle tokenized URLs (New)
                if (urlObj.searchParams.has('token')) {
                    const decrypted = decryptUrl(urlObj.searchParams.get('token'));
                    next = decrypted ? decrypted.url : null;
                } 
                // Handle raw URLs
                else if (urlObj.searchParams.has('url')) {
                    next = urlObj.searchParams.get('url');
                } 
                else if (urlObj.pathname.includes('/proxy/')) {
                    const m = urlObj.pathname.match(/\/proxy\/(.+)$/);
                    if (m) next = decodeURIComponent(m[1]);
                }
            } catch {
                // regex fallback for mangled URLs
                const m = candidate.match(/(url|token)=([^&]+)/);
                if (m) {
                    const val = decodeURIComponent(m[2]);
                    if (m[1] === 'token') {
                        const decrypted = decryptUrl(val);
                        next = decrypted ? decrypted.url : null;
                    } else {
                        next = val;
                    }
                }
            }
            
            if (next && next !== candidate && next !== 'undefined') {
                candidate = next;
                depth++;
            } else {
                break;
            }
        }
        
        // Final decoding if needed (handle multiple levels of encoding)
        let last = '';
        while (candidate.includes('%') && candidate !== last && depth < 10) {
            last = candidate;
            try { candidate = decodeURIComponent(candidate); } catch { break; }
            depth++;
        }
        
        return candidate;
    } catch { return proxyUrl; }
}

function rewriteM3u8(content, targetUrl, baseProxyUrl, headers) {
    const lines = content.split('\n');
    const out = []; const segmentUrls = [];
    let nextLineIsStreamUrl = false;
    
    for (const line of lines) {
        if (line.startsWith('#')) {
            if (line.startsWith('#EXT-X-KEY:')) {
                const uriMatch = line.match(/URI=\"([^\"]+)\"/);
                if (uriMatch) {
                    try {
                        const absKey = new URL(uriMatch[1], targetUrl).href;
                        const token = encryptUrl(absKey, null, headers);
                        const proxyUrl = `${baseProxyUrl}/ts-proxy?token=${token}&clampOpen=0&progressiveOpen=0&noSynth=1&force200=1&tailPrefetch=0`;
                        out.push(line.replace(uriMatch[1], proxyUrl));
                        if (!isCacheDisabled()) prefetchSegment(absKey, headers);
                    } catch (_e) {
                        out.push(line);
                    }
                } else out.push(line);
            } else if (line.startsWith('#EXT-X-MEDIA:') || line.startsWith('#EXT-X-I-FRAME-STREAM-INF:')) {
                const uriMatch = line.match(/URI=\"([^\"]+)\"/);
                if (uriMatch) {
                    try {
                        const mediaUrl = new URL(uriMatch[1], targetUrl).href;
                        const token = encryptUrl(mediaUrl, null, headers);
                        const proxyUrl = `${baseProxyUrl}/m3u8-proxy?token=${token}`;
                        out.push(line.replace(uriMatch[1], proxyUrl));
                    } catch { out.push(line); }
                } else out.push(line);
            } else if (line.startsWith('#EXT-X-MAP:')) {
                const uriMatch = line.match(/URI=\"([^\"]+)\"/);
                if (uriMatch) {
                    try {
                        const mapUrl = new URL(uriMatch[1], targetUrl).href;
                        const token = encryptUrl(mapUrl, null, headers);
                        const proxyUrl = `${baseProxyUrl}/ts-proxy?token=${token}&clampOpen=0&progressiveOpen=0&noSynth=1&force200=1&tailPrefetch=0`;
                        out.push(line.replace(uriMatch[1], proxyUrl));
                        if (!isCacheDisabled()) prefetchSegment(mapUrl, headers);
                    } catch (_e) { out.push(line); }
                } else out.push(line);
            } else if (line.startsWith('#EXT-X-STREAM-INF:')) {
                // The URL for EXT-X-STREAM-INF is on the next line
                nextLineIsStreamUrl = true;
                out.push(line);
            } else out.push(line);
        } else if (line.trim()) {
            try {
                let candidate = line.trim();
                const decoded = decodeVidzeeToken(candidate);
                if (decoded) candidate = decoded;
                
                // Correctly resolve relative and absolute-path-style URLs
                let abs;
                try {
                    // Standard resolution
                    const resolved = new URL(candidate, targetUrl);
                    
                    // Inherit query parameters from the parent playlist URL if the segment doesn't have them
                    // This is crucial for signed HLS (VixSrc, etc.) where the token is on the playlist URL
                    const parentUrl = new URL(targetUrl);
                    if (parentUrl.search) {
                        parentUrl.searchParams.forEach((value, key) => {
                            if (!resolved.searchParams.has(key)) {
                                resolved.searchParams.set(key, value);
                            }
                        });
                    }
                    abs = resolved.href;
                } catch (urlErr) {
                    // Fallback for absolute-path-style if URL constructor fails with relative base
                    if (candidate.startsWith('/')) {
                        const origin = new URL(targetUrl).origin;
                        abs = origin + candidate;
                    } else {
                        throw urlErr;
                    }
                }

                if (/\.(ts|m4s|mp4|png|jpg|jpeg|bmp)(\?|$)/i.test(abs)) {
                    segmentUrls.push(abs);
                    const token = encryptUrl(abs, null, headers);
                    // Removed force200=1 for segments to allow Range requests if needed
                    out.push(`${baseProxyUrl}/ts-proxy?token=${token}&clampOpen=0&progressiveOpen=0&noSynth=1&tailPrefetch=0`);
                } else {
                    // Default: proxy any HTTP/S child URL through m3u8-proxy so nested playlists (even without .m3u8 extension) get rewritten.
                    const token = encryptUrl(abs, null, headers);
                    const proxyUrl = `${baseProxyUrl}/m3u8-proxy?token=${token}`;
                    out.push(nextLineIsStreamUrl ? proxyUrl : proxyUrl);
                    nextLineIsStreamUrl = false;
                }
            } catch { out.push(line); }
        } else {
            out.push(line);
            nextLineIsStreamUrl = false;
        }
    }
    // Disable aggressive prefetching to prevent rate-limiting/stuttering
    // if (segmentUrls.length && !isCacheDisabled()) {
    //     Promise.all(segmentUrls.length > 5 ? segmentUrls.slice(0, 5).map(u => prefetchSegment(u, headers)) : segmentUrls.map(u => prefetchSegment(u, headers))).catch(() => undefined);
    // }
    return out.join('\n');
}

function createProxyRoutes(app) {
    // m3u8 playlist proxy
    app.get('/m3u8-proxy', cors(), async (req, res) => {
        let targetUrl = req.query.url; 
        const token = req.query.token;
        const debug = process.env.VIDZEE_DEBUG === '1';
        let decryptedHeaders = null;

        // Security: Decrypt token if provided
        if (token) {
            const { decryptUrl } = require('../utils/security');
            const decrypted = decryptUrl(token, req.ip);
            if (!decrypted) return res.status(403).json({ error: 'Invalid or expired token' });
            targetUrl = decrypted.url;
            decryptedHeaders = decrypted.headers;
        } else if (!targetUrl) {
            return res.status(400).json({ error: 'Token or URL parameter required' });
        }

        // Strict Mode: Block raw URL usage from external IPs to prevent open relay abuse
        const isLocal = req.ip === '127.0.0.1' || req.ip === '::1';
        if (!token && !isLocal && process.env.STRICT_SECURITY === 'true') {
             return res.status(403).json({ error: 'Access denied: Token required' });
        }

        if (debug) console.log('[m3u8-proxy] Received URL:', targetUrl); 
        
        let headers = decryptedHeaders || {};
        // Merge query headers if present (fallback)
        try { 
            const queryHeaders = JSON.parse(req.query.headers || '{}'); 
            headers = { ...headers, ...queryHeaders };
        } catch {
            // Ignore URL parsing errors
        }
        const decoded = decodeVidzeeToken(targetUrl);
        if (debug) console.log('[m3u8-proxy] After decodeVidzeeToken:', decoded || 'null');
        if (decoded) targetUrl = decoded;
        if (!/^https?:\/\//i.test(targetUrl)) {
            if (debug) console.log('[m3u8-proxy] Invalid URL check failed, targetUrl:', targetUrl);
            return res.status(400).json({ error: 'Invalid target URL', target: targetUrl });
        }
        if (headers.Referer && !headers.Origin) {
            try { headers.Origin = new URL(headers.Referer).origin; } catch (_e) { /* ignore */ }
        }
        
        // Headers to clean
        const mergedHeaders = {};
        const skipHeaders = ['host', 'connection', 'content-length', 'content-type', 'accept-encoding', 'cookie', 'if-none-match', 'if-modified-since', 'range', 'sec-fetch-mode', 'sec-fetch-site', 'sec-fetch-dest', 'sec-fetch-user'];
        
        Object.keys(headers).forEach(k => {
            if (!skipHeaders.includes(k.toLowerCase())) {
                mergedHeaders[k] = headers[k];
            }
        });

        const hasUA = Object.keys(mergedHeaders).some(k => k.toLowerCase() === 'user-agent');
        if (!hasUA) {
            mergedHeaders['User-Agent'] = DEFAULT_UA;
        }
        
        const hasAccept = Object.keys(mergedHeaders).some(k => k.toLowerCase() === 'accept');
        if (!hasAccept) {
            mergedHeaders['Accept'] = '*/*';
        }

        // Pass through Range header only if it doesn't look like a standard HLS request
        // Standard HLS players don't use Range for the master playlist
        if (req.headers.range && !targetUrl.includes('.m3u8')) {
            mergedHeaders['Range'] = req.headers.range;
        }

        try {
            const t0 = Date.now();
            let currentUrl = targetUrl;
            let response = null;
            let redirectCount = 0;
            const maxRedirects = 5;

            // Manual redirect following with security checks
            while (redirectCount <= maxRedirects) {
                response = await fetchWithRetry(currentUrl, { 
                    headers: mergedHeaders, 
                    redirect: 'manual',
                    timeout: 30000 // Increased from 15s to 30s
                });

                if (response.status >= 300 && response.status < 400) {
                    const location = response.headers.get('location');
                    if (!location) break;
                    
                    const nextUrl = new URL(location, currentUrl).href;
                    // Security check for the new location
                    const u = new URL(nextUrl);
                    const host = u.hostname.toLowerCase();
                    if (host === 'localhost' || host === '127.0.0.1' || host === '0.0.0.0' || host.startsWith('192.168.') || host.startsWith('10.')) {
                        console.error(`[m3u8-proxy] Blocked redirect to internal network: ${nextUrl}`);
                        return res.status(403).json({ error: 'Blocked redirect to internal network', origin: targetUrl });
                    }
                    
                    currentUrl = nextUrl;
                    redirectCount++;
                    if (debug) console.log(`[m3u8-proxy] Following redirect ${redirectCount} to: ${currentUrl}`);
                    continue;
                }
                break;
            }

            const duration = Date.now() - t0;
            if (duration > 8000 && debug) {
                console.warn(`[m3u8-proxy] Slow upstream chain (${duration}ms): ${targetUrl}`);
            }

            if (!response.ok && response.status !== 206) return res.status(response.status).json({ error: `M3U8 fetch failed: ${response.status}`, target: currentUrl, final: currentUrl });
            
            const contentType = response.headers.get('content-type') || '';
            const isVideo = contentType.includes('video/') || /\.(mp4|mkv|mov|avi|ts)$/i.test(currentUrl.split('?')[0]);
            
            if (isVideo || response.status === 206) {
                if (debug) console.log('[m3u8-proxy] Detected video or 206 response, piping raw', { contentType, status: response.status, targetUrl });
                res.status(response.status);
                res.setHeader('Content-Type', contentType || 'video/mp4');
                if (response.headers.get('content-range')) res.setHeader('Content-Range', response.headers.get('content-range'));
                if (response.headers.get('accept-ranges')) res.setHeader('Accept-Ranges', response.headers.get('accept-ranges'));
                if (response.headers.get('content-length')) res.setHeader('Content-Length', response.headers.get('content-length'));
                res.setHeader('Access-Control-Allow-Origin', '*');
                return response.body.pipe(res);
            }

            const text = await response.text();
            
            // Basic validation: if it doesn't look like HLS and isn't a known extension, don't rewrite
            if (!text.trim().startsWith('#EXTM3U') && !targetUrl.includes('.m3u8')) {
                if (debug) console.log('[m3u8-proxy] Content does not start with #EXTM3U and no .m3u8 ext, sending raw');
                res.setHeader('Content-Type', contentType || 'text/plain');
                res.setHeader('Access-Control-Allow-Origin', '*');
                return res.send(text);
            }

            const protocol = req.headers['x-forwarded-proto'] || req.protocol || 'http';
            const baseProxyUrl = `${protocol}://${req.get('host')}`;
            const rewritten = rewriteM3u8(text, targetUrl, baseProxyUrl, headers);
            res.setHeader('Content-Type', 'application/vnd.apple.mpegurl');
            res.setHeader('Cache-Control', 'no-cache, no-store, must-revalidate');
            res.setHeader('Access-Control-Allow-Origin', '*');
            res.send(rewritten);
        } catch (e) {
            console.error('[m3u8-proxy] error', e);
            console.error('[m3u8-proxy] stack', e.stack);
            console.error('[m3u8-proxy] targetUrl', targetUrl);
            res.status(500).json({ error: e.message, details: e.stack });
        }
    });

    // ts / key segment proxy
    app.get('/ts-proxy', cors(), async (req, res) => {
        let targetUrl = req.query.url; 
        const token = req.query.token;
        let decryptedHeaders = null;

        // Security: Decrypt token if provided
        if (token) {
            const { decryptUrl } = require('../utils/security');
            const decrypted = decryptUrl(token, req.ip);
            if (!decrypted) return res.status(403).json({ error: 'Invalid or expired token' });
            targetUrl = decrypted.url;
            decryptedHeaders = decrypted.headers;
        } else if (!targetUrl) {
            return res.status(400).json({ error: 'Token or URL parameter required' });
        }

        // Strict Mode: Block raw URL usage from external IPs
        const isLocal = req.ip === '127.0.0.1' || req.ip === '::1';
        if (!token && !isLocal && process.env.STRICT_SECURITY === 'true') {
             return res.status(403).json({ error: 'Access denied: Token required' });
        }

        if (!/^https?:\/\//i.test(targetUrl)) {
            return res.status(400).json({ error: 'Invalid target URL', target: targetUrl });
        }

        const debug = req.query.debug === '1';
        const force200 = req.query.force200 === '1';
        const progressiveOpen = req.query.progressiveOpen === '1';
        const clampOpen = req.query.clampOpen === '1';
        const openChunkKB = parseInt(req.query.openChunkKB || '1024');
        const noSynth = req.query.noSynth === '1';
        const initChunkKB = parseInt(req.query.initChunkKB || '512');
        const tailPrefetchEnabled = req.query.tailPrefetch === '1';
        const tailPrefetchKB = parseInt(req.query.tailPrefetchKB || '256');

        let headers = decryptedHeaders || {};
        try { 
            const queryHeaders = JSON.parse(req.query.headers || '{}'); 
            headers = { ...headers, ...queryHeaders };
        } catch {
            // Ignore URL parsing errors
        }
        if (headers.Referer && !headers.Origin) {
            try { headers.Origin = new URL(headers.Referer).origin; } catch (_e) { /* ignore */ }
        }

        // Normalize headers to avoid duplicates and strip problematic ones
        const skipHeaders = ['host', 'connection', 'content-length', 'content-type', 'accept-encoding', 'cookie', 'if-none-match', 'if-modified-since', 'range'];
        Object.keys(headers).forEach(k => {
            if (skipHeaders.includes(k.toLowerCase())) {
                delete headers[k];
            }
        });

        const hasUA = Object.keys(headers).some(k => k.toLowerCase() === 'user-agent');
        if (!hasUA) {
            headers['User-Agent'] = DEFAULT_UA;
        }

        const hasAccept = Object.keys(headers).some(k => k.toLowerCase() === 'accept');
        if (!hasAccept) {
            headers['Accept'] = '*/*';
        }
        // Pass through Range header for progressive playback / seeking
        const range = req.headers['range'];
        let appliedClamp = false;
        let effectiveRange = range;
        if (range) headers.Range = range;
        if (force200) { // treat as full fetch
            effectiveRange = null;
            delete headers.Range;
        }
        // Clamp first open-ended bytes=0- request to a bounded span (once per TTL) to avoid gigantic initial 206
        if (range && /^bytes=0-\s*$/i.test(range) && !force200) {
            const now = Date.now();
            // Progressive growth supersedes single-shot clamp if enabled
            if (progressiveOpen) {
                const prog = progressiveOpenMap.get(targetUrl) || { lastEnd: -1 };
                const prevEnd = prog.lastEnd;
                let nextEnd;
                if (prog.lastEnd < 0) {
                    nextEnd = openChunkKB * 1024 - 1;
                } else {
                    const increment = openChunkKB * 1024;
                    nextEnd = prog.lastEnd + increment;
                    const maxCap = 256 * 1024 * 1024 - 1; // 256MB
                    if (nextEnd > maxCap) nextEnd = maxCap;
                }
                const newRange = `bytes=0-${nextEnd}`;
                headers.Range = newRange;
                effectiveRange = newRange;
                prog.lastEnd = nextEnd;
                progressiveOpenMap.set(targetUrl, prog);
                appliedClamp = true; // reuse flag for logging
                if (debug) console.log('[ts-proxy] progressiveOpen expand', { prevEnd, nextEnd, increment: openChunkKB * 1024 });
            } else if (clampOpen) {
                const lastTs = openRangeClampMap.get(targetUrl);
                if (!lastTs || (now - lastTs) > OPEN_RANGE_CLAMP_TTL_MS) {
                    const clampBytes = openChunkKB * 1024;
                    const newRange = `bytes=0-${clampBytes - 1}`;
                    headers.Range = newRange;
                    effectiveRange = newRange;
                    openRangeClampMap.set(targetUrl, now);
                    appliedClamp = true;
                }
            }
        }
        if (debug) console.log('[ts-proxy] incoming', { url: targetUrl, range, effectiveRange, appliedClamp, clampOpen, progressiveOpen, openChunkKB, noSynth, initChunkKB, force200, tailPrefetchEnabled, tailPrefetchKB });
        if (!isCacheDisabled()) {
            const cached = getCachedSegment(targetUrl);
            if (cached) {
                const ct = inferContentType(cached.headers['content-type'], targetUrl);
                res.setHeader('Content-Type', ct);
                res.setHeader('Cache-Control', 'public, max-age=3600');
                res.setHeader('Access-Control-Allow-Origin', '*');
                res.setHeader('Accept-Ranges', 'bytes');
                if (debug) console.log('[ts-proxy] cache hit');
                return res.send(Buffer.from(cached.data));
            }
        }
        try {
            // If we have a tail prefetch entry and the client requests a tail range fully inside it, serve from memory early.
            if (effectiveRange && /^bytes=\d+-$/i.test(effectiveRange)) {
                const tailEntry = tailPrefetchMap.get(targetUrl);
                if (tailEntry) {
                    const m = effectiveRange.match(/bytes=(\d+)-/i);
                    if (m) {
                        const startReq = parseInt(m[1], 10);
                        if (!isNaN(startReq) && startReq >= tailEntry.start && startReq <= tailEntry.end) {
                            const offset = startReq - tailEntry.start;
                            const slice = tailEntry.data.subarray(offset);
                            const endByte = tailEntry.end;
                            res.status(206);
                            const ct = inferContentType(null, targetUrl);
                            res.setHeader('Content-Type', ct);
                            res.setHeader('Accept-Ranges', 'bytes');
                            res.setHeader('Content-Length', slice.length.toString());
                            res.setHeader('Content-Range', `bytes ${startReq}-${endByte}/${tailEntry.size}`);
                            res.setHeader('Cache-Control', 'public, max-age=3600');
                            res.setHeader('Access-Control-Allow-Origin', '*');
                            if (debug) console.log('[ts-proxy] tail serve hit', { startReq, cachedStart: tailEntry.start, cachedEnd: tailEntry.end, size: tailEntry.size });
                            return res.end(slice);
                        }
                    }
                }
            }

            // HEAD preflight (only when no explicit Range requested and not disabled) to get size for synthesized partial response
            let contentLength = null;
            let upstreamAcceptRanges = null;
            if (!effectiveRange && !noSynth && !force200) {
                try {
                    const headResp = await fetchWithRetry(targetUrl, { method: 'HEAD', headers: { 'User-Agent': DEFAULT_UA, ...headers } });
                    if (headResp.ok) {
                        contentLength = headResp.headers.get('content-length');
                        upstreamAcceptRanges = headResp.headers.get('accept-ranges');
                        if (debug) console.log('[ts-proxy] HEAD ok', { contentLength, upstreamAcceptRanges });
                    } else if (debug) {
                        console.log('[ts-proxy] HEAD status', headResp.status);
                    }
                } catch (_e) { if (debug) console.log('[ts-proxy] HEAD failed'); }
            }

            // Fallback probe: if still no size, attempt a 0-0 range request to derive total size from Content-Range
            if (!effectiveRange && !contentLength && !noSynth && !force200) {
                try {
                    const probeRange = 'bytes=0-0';
                    if (debug) console.log('[ts-proxy] probe range');
                    const probeResp = await fetchWithRetry(targetUrl, { headers: { 'User-Agent': DEFAULT_UA, ...headers, 'Range': probeRange } });
                    if (probeResp.status === 206) {
                        const cr = probeResp.headers.get('content-range');
                        if (cr) {
                            const m = cr.match(/\/(\d+)$/); if (m) contentLength = m[1];
                        }
                        try { await probeResp.arrayBuffer(); } catch (readErr) {
                            if (debug) console.log('[ts-proxy] probe body read failed', readErr.message);
                        }
                        if (debug) console.log('[ts-proxy] probe success', { contentLength });
                    } else if (debug) {
                        console.log('[ts-proxy] probe status', probeResp.status);
                    }
                } catch (_e) { if (debug) console.log('[ts-proxy] probe failed'); }
            }

            const upstreamOptions = { headers: { 'User-Agent': DEFAULT_UA, ...headers }, redirect: 'manual' };
            // If client supplied a suffix range like bytes=0- we pass it; force200 strips Range
            if (force200 && headers.Range) delete headers.Range;
            
            // Forced Header Tuning for hostile CDNs in segments
            const hostileHosts = ['hakunaymatata', 'quibble', 'vidhide', 'cdnvideo', 'stream'];
            if (hostileHosts.some(h => targetUrl.includes(h))) {
                upstreamOptions.headers['Connection'] = 'close';
            } else {
                upstreamOptions.headers['Connection'] = 'keep-alive';
            }

            let resp = null;
            let currentUrl = targetUrl;
            let redirectCount = 0;
            const maxRedirects = 3; // Segments shouldn't redirect much
            const isSegment = /\.(ts|m4s|mp4|mkv|m4v|m4a|fmp4)(\?|$)/i.test(targetUrl) || targetUrl.includes('/resource/') || targetUrl.includes('/bt/');

            while (redirectCount <= maxRedirects) {
                resp = await fetchWithRetry(currentUrl, { 
                    ...upstreamOptions, 
                    timeout: isSegment ? 120000 : 45000 
                });

                if (resp.status >= 300 && resp.status < 400) {
                    const location = resp.headers.get('location');
                    if (!location) break;
                    
                    const nextUrl = new URL(location, currentUrl).href;
                    const u = new URL(nextUrl);
                    const host = u.hostname.toLowerCase();
                    if (host === 'localhost' || host === '127.0.0.1' || host === '0.0.0.0' || host.startsWith('192.168.') || host.startsWith('10.')) {
                        console.error(`[ts-proxy] Blocked redirect to localhost: ${nextUrl} from ${targetUrl}`);
                        return res.status(403).json({ error: 'Blocked redirect to internal network', origin: targetUrl });
                    }
                    
                    currentUrl = nextUrl;
                    redirectCount++;
                    if (debug) console.log(`[ts-proxy] Following redirect ${redirectCount} to: ${currentUrl}`);
                    continue;
                }
                break;
            }

            if (!resp.ok && resp.status !== 206) {
                if (debug) console.log('[ts-proxy] upstream error', resp.status);
                return res.status(resp.status).json({ error: `upstream returned ${resp.status}`, target: currentUrl });
            }

            // Copy crucial headers
            const upstreamCT = resp.headers.get('content-type');
            const ct = inferContentType(upstreamCT, currentUrl);
            res.setHeader('Content-Type', ct);
            upstreamAcceptRanges = resp.headers.get('accept-ranges');
            res.setHeader('Accept-Ranges', upstreamAcceptRanges || 'bytes');
            const contentRange = resp.headers.get('content-range');
            if (contentRange) res.setHeader('Content-Range', contentRange);
            const cl = resp.headers.get('content-length'); if (cl) res.setHeader('Content-Length', cl);
            res.setHeader('Cache-Control', 'public, max-age=3600');
            res.setHeader('Access-Control-Allow-Origin', '*');
            // Pipe first chunk then leave connection open? Simpler: end after chunk; player will request next range.
            return resp.body.pipe(res);
        } catch (e) {
            console.error('[ts-proxy] error', e);
            console.error('[ts-proxy] stack', e.stack);
            console.error('[ts-proxy] targetUrl', targetUrl);
            res.status(500).json({ error: e.message, details: e.stack });
        }
    });

    // subtitle proxy
    app.get('/sub-proxy', cors(), async (req, res) => {
        let targetUrl = req.query.url; 
        const token = req.query.token;
        let decryptedHeaders = null;

        // Security: Decrypt token if provided
        if (token) {
            const { decryptUrl } = require('../utils/security');
            const decrypted = decryptUrl(token, req.ip);
            if (!decrypted) return res.status(403).json({ error: 'Invalid or expired token' });
            targetUrl = decrypted.url;
            decryptedHeaders = decrypted.headers;
        } else if (!targetUrl) {
            return res.status(400).json({ error: 'Token or URL parameter required' });
        }

        // Strict Mode: Block raw URL usage from external IPs
        const isLocal = req.ip === '127.0.0.1' || req.ip === '::1';
        if (!token && !isLocal && process.env.STRICT_SECURITY === 'true') {
             return res.status(403).json({ error: 'Access denied: Token required' });
        }

        let headers = decryptedHeaders || {};
        try { 
            const queryHeaders = JSON.parse(req.query.headers || '{}'); 
            headers = { ...headers, ...queryHeaders };
        } catch {
            // Ignore URL parsing errors
        }
        
        const mergedHeaders = {};
        const skipHeaders = ['host', 'connection', 'content-length', 'content-type', 'accept-encoding', 'cookie', 'range'];
        Object.keys(headers).forEach(k => {
            if (!skipHeaders.includes(k.toLowerCase())) mergedHeaders[k] = headers[k];
        });
        if (!mergedHeaders['User-Agent']) mergedHeaders['User-Agent'] = DEFAULT_UA;

        try {
            const resp = await fetchWithRetry(targetUrl, { headers: mergedHeaders, redirect: 'follow' });
            if (!resp.ok) return res.status(resp.status).json({ error: `subtitle fetch failed: ${resp.status}` });
            
            let ct = resp.headers.get('content-type');
            if (!ct || ct.includes('octet-stream') || ct.includes('text/plain')) {
                if (targetUrl.includes('.vtt')) ct = 'text/vtt';
                else if (targetUrl.includes('.srt')) ct = 'text/plain'; // browser treats srt as text
            }

            res.setHeader('Access-Control-Allow-Origin', '*');
            res.setHeader('Cache-Control', 'public, max-age=3600');
            res.setHeader('Content-Type', ct || 'text/vtt');
            resp.body.pipe(res);
        } catch (e) { res.status(500).json({ error: e.message }); }
    });
}

function processStreamsForProxy(streams, serverUrl) {
    if (!Array.isArray(streams)) return streams;
    
    let serverHost = '';
    try {
        serverHost = new URL(serverUrl).host;
    } catch {}

    return streams.map(s => {
        if (!s || !s.url || typeof s.url !== 'string') return s;
        
        // Skip proxy wrapping if explicitly requested (e.g., direct CDN links)
        if (s.skipProxy === true) {
            return s;
        }
        
        // Check if already proxied by US to avoid double wrapping
        // We check for the presence of the proxy path segments anywhere in the URL
        if (s.url.includes('/m3u8-proxy') || s.url.includes('/ts-proxy')) {
            return s;
        }
        
        try {
            const u = new URL(s.url);
            if (u.host === serverHost && (u.pathname === '/m3u8-proxy' || u.pathname === '/ts-proxy')) {
                return s;
            }
        } catch {}
        
        const original = extractOriginalUrl(s.url);
        
        // Ensure we use the headers provided in the object, or fallback to those in the URL if it was already proxied
        let headers = s.headers || {};
        if (Object.keys(headers).length === 0 && s.url.includes('headers=')) {
            try {
                const u = new URL(s.url, 'http://localhost');
                const hJson = u.searchParams.get('headers');
                if (hJson) headers = JSON.parse(hJson);
            } catch {}
        }

        // Proxy subtitles if present
        if (Array.isArray(s.subtitles)) {
            s.subtitles = s.subtitles.map(sub => {
                if (!sub || !sub.url || sub.url.includes('/sub-proxy')) return sub;
                const subOriginal = extractOriginalUrl(sub.url);
                const subToken = encryptUrl(subOriginal, null, headers);
                return {
                    ...sub,
                    url: `${serverUrl}/sub-proxy?token=${subToken}`
                };
            });
        }

        let host = '';
        try {
            host = new URL(original).hostname.toLowerCase();
        } catch {
            // Ignore errors
        }
        
        const token = encryptUrl(original, null, headers);
        // Force specific hosts through ts-proxy (direct file style) even without extension
        if (host.includes('pixeldrain.') || host === 'video-downloads.googleusercontent.com') {
            return { ...s, url: `${serverUrl}/ts-proxy?token=${token}` };
        }
        if (/\.(mp4|mkv)(\?|$)/i.test(original)) {
            return { ...s, url: `${serverUrl}/ts-proxy?token=${token}` };
        }
        return { ...s, url: `${serverUrl}/m3u8-proxy?token=${token}` };
    });
}

module.exports = { createProxyRoutes, processStreamsForProxy };