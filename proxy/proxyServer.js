// Lightweight proxy/m3u8/segment/subtitle rewriting layer. thanks to https://github.com/cinepro-org/ for the original code.
// Adapted from external project (user supplied) to fit CommonJS style and existing config system.
// Provides conditional route registration via enableProxy flag.

const cors = require('cors');
const fetch = require('node-fetch');
const http = require('http');
const https = require('https');
let cryptoJs;

// Force IPv4 for proxy requests to avoid VPS connectivity issues
const httpAgent = new http.Agent({ family: 4 });
const httpsAgent = new https.Agent({ family: 4 });

// Retry configuration for fetch operations
const MAX_RETRIES = 3;
const RETRY_DELAY_MS = 1000;
const FETCH_TIMEOUT_MS = 30000; // Increased timeout for slower VPS connections

// Helper function to fetch with retry logic
async function fetchWithRetry(url, options = {}, maxRetries = MAX_RETRIES) {
    const fetchOptions = {
        ...options,
        agent: (parsedUrl) => (parsedUrl.protocol === 'http:' ? httpAgent : httpsAgent),
        timeout: options.timeout || FETCH_TIMEOUT_MS
    };
    
    let lastError;
    for (let attempt = 1; attempt <= maxRetries; attempt++) {
        try {
            return await fetch(url, fetchOptions);
        } catch (error) {
            lastError = error;
            if (attempt >= maxRetries) throw error;
            console.log(`[retry] Attempt ${attempt}/${maxRetries} failed for ${url}: ${error.message}`);
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

const DEFAULT_UA = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36';

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
                    console.log('[VidZee] Fallback to binary encoding for token');
                } catch (e2) {
                    try {
                        // Last resort: latin1 (handles all byte values)
                        raw = buffer.toString('latin1');
                        console.log('[VidZee] Fallback to latin1 encoding for token');
                    } catch (e3) {
                        // Only log if debug enabled
                        if (process.env.VIDZEE_DEBUG === '1') {
                            console.error('[VidZee] Decode error: Failed with all encodings');
                        }
                        return null;
                    }
                }
            }
        } catch (e) {
            // Only log if debug enabled
            if (process.env.VIDZEE_DEBUG === '1') {
                console.error('[VidZee] Decode error: Failed to decode base64:', e.message);
            }
            return null;
        }
        
        // Extra validation and sanitization
        if (!raw) {
            if (debug) console.log('[VidZee] No raw data after decode');
            return null;
        }
        
        if (debug) {
            console.log('[VidZee] Raw before sanitization:', raw.substring(0, 100));
            console.log('[VidZee] Raw contains colon:', raw.includes(':'));
        }
        
        // Aggressive sanitization of non-printable and control characters
        raw = raw.replace(/[\u0000-\u001F\u007F-\u009F]/g, '');
        
        // Make sure we still have a valid structure after sanitization
        if (!raw.includes(':')) {
            if (debug) console.log('[VidZee] No colon after sanitization');
            return null;
        }
        const [ivB64, cipherB64] = raw.split(':');
        if (!ivB64 || !cipherB64) {
            if (debug) console.log('[VidZee] Missing IV or cipher after split');
            return null;
        }
        
        if (debug) {
            console.log('[VidZee] IV length:', ivB64.length);
            console.log('[VidZee] Cipher length:', cipherB64.length);
        }
        
        // Normalize inner base64 strings
        const normalizeInner = (s) => {
            let n = s.replace(/-/g, '+').replace(/_/g, '/').replace(/[^A-Za-z0-9+/=]/g, '');
            const p = n.length % 4;
            return p ? n + '='.repeat(4 - p) : n;
        };
        
        const fixedIv = normalizeInner(ivB64);
        const fixedCipher = normalizeInner(cipherB64);
        
        if (!cryptoJs) cryptoJs = require('crypto-js');
        const iv = cryptoJs.enc.Base64.parse(fixedIv.trim());
        const keyStr = 'qrincywincyspider';
        const keyUtf8 = cryptoJs.enc.Utf8.parse(keyStr.padEnd(32, '\0'));
        
        // Decrypt with proper error handling for crypto-js malformed UTF-8 errors
        let decrypted;
        try {
            const decryptedWords = cryptoJs.AES.decrypt(fixedCipher.trim(), keyUtf8, {
                iv,
                mode: cryptoJs.mode.CBC,
                padding: cryptoJs.pad.Pkcs7
            });
            
            if (debug) {
                console.log('[VidZee] CryptoJS decrypt completed');
                console.log('[VidZee] Decrypted words length:', decryptedWords.sigBytes);
            }
            
            // First try standard UTF-8 decoding
            try {
                decrypted = decryptedWords.toString(cryptoJs.enc.Utf8);
                if (debug) console.log('[VidZee] UTF-8 toString success');
            } catch (utf8Error) {
                // Fall back to Latin1 if UTF-8 fails - silent fallback to reduce logs
                decrypted = decryptedWords.toString(cryptoJs.enc.Latin1);
                if (debug) console.log('[VidZee] Latin1 toString fallback');
            }
        } catch (cryptoError) {
            if (debug) console.log('[VidZee] CryptoJS decrypt error:', cryptoError.message);
            // Silent failure to reduce logs
            return null;
        }
        
        if (!decrypted) {
            if (debug) console.log('[VidZee] No decrypted result');
            return null;
        }
        
        if (!/^https?:\/\//i.test(decrypted)) {
            if (debug) console.log('[VidZee] Decrypted result is not a URL');
            // Silent failure
            return null;
        }
        
        if (debug) console.log('[VidZee] Successfully decoded URL');
        return decrypted.trim();
    } catch (_e) {
        // Silently handle decode errors to reduce log spam
        // Only log detailed errors if debug is enabled
        if (process.env.VIDZEE_DEBUG === '1') {
            console.error('[VidZee] Decode error:', _e.message);
        }
        return null;
    }
}

function extractOriginalUrl(proxyUrl) {
    try {
        if (!proxyUrl || typeof proxyUrl !== 'string') return proxyUrl;
        
        let candidate = proxyUrl;
        // Recursively unwrap proxy URLs
        let depth = 0;
        while (depth < 5 && (candidate.includes('/m3u8-proxy') || candidate.includes('/ts-proxy') || candidate.includes('url='))) {
            let next = null;
            try {
                const urlObj = new URL(candidate, 'http://localhost');
                if (urlObj.searchParams.has('url')) {
                    next = urlObj.searchParams.get('url');
                }
            } catch {
                // regex fallback
                const m = candidate.match(/url=([^&]+)/);
                if (m) next = decodeURIComponent(m[1]);
            }
            
            if (next && next !== candidate) {
                candidate = next;
                depth++;
            } else {
                break;
            }
        }
        
        // Final decoding if needed
        if (candidate.includes('%')) {
            try { candidate = decodeURIComponent(candidate); } catch {}
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
                const uriMatch = line.match(/URI="([^"]+)"/);
                if (uriMatch) {
                    try {
                        const absKey = new URL(uriMatch[1], targetUrl).href;
                        const proxyUrl = `${baseProxyUrl}/ts-proxy?url=${encodeURIComponent(absKey)}&headers=${encodeURIComponent(JSON.stringify(headers))}&clampOpen=0&progressiveOpen=0&noSynth=1&force200=1&tailPrefetch=0`;
                        out.push(line.replace(uriMatch[1], proxyUrl));
                        if (!isCacheDisabled()) prefetchSegment(absKey, headers);
                    } catch (_e) {
                        out.push(line);
                    }
                } else out.push(line);
            } else if (line.startsWith('#EXT-X-MEDIA:') || line.startsWith('#EXT-X-I-FRAME-STREAM-INF:')) {
                const uriMatch = line.match(/URI="([^"]+)"/);
                if (uriMatch) {
                    try {
                        const mediaUrl = new URL(uriMatch[1], targetUrl).href;
                        const proxyUrl = `${baseProxyUrl}/m3u8-proxy?url=${encodeURIComponent(mediaUrl)}&headers=${encodeURIComponent(JSON.stringify(headers))}`;
                        out.push(line.replace(uriMatch[1], proxyUrl));
                    } catch { out.push(line); }
                } else out.push(line);
            } else if (line.startsWith('#EXT-X-MAP:')) {
                const uriMatch = line.match(/URI="([^"]+)"/);
                if (uriMatch) {
                    try {
                        const mapUrl = new URL(uriMatch[1], targetUrl).href;
                        const proxyUrl = `${baseProxyUrl}/ts-proxy?url=${encodeURIComponent(mapUrl)}&headers=${encodeURIComponent(JSON.stringify(headers))}&clampOpen=0&progressiveOpen=0&noSynth=1&force200=1&tailPrefetch=0`;
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

                if (/\.(ts|mp4|png|jpg|jpeg|bmp)(\?|$)/i.test(abs)) {
                    segmentUrls.push(abs);
                    // Removed force200=1 for segments to allow Range requests if needed
                    out.push(`${baseProxyUrl}/ts-proxy?url=${encodeURIComponent(abs)}&headers=${encodeURIComponent(JSON.stringify(headers))}&clampOpen=0&progressiveOpen=0&noSynth=1&tailPrefetch=0`);
                } else {
                    // Default: proxy any HTTP/S child URL through m3u8-proxy so nested playlists (even without .m3u8 extension) get rewritten.
                    const proxyUrl = `${baseProxyUrl}/m3u8-proxy?url=${encodeURIComponent(abs)}&headers=${encodeURIComponent(JSON.stringify(headers))}`;
                    out.push(nextLineIsStreamUrl ? proxyUrl : `${baseProxyUrl}/m3u8-proxy?url=${encodeURIComponent(abs)}&headers=${encodeURIComponent(JSON.stringify(headers))}`);
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
        let targetUrl = req.query.url; if (!targetUrl) return res.status(400).json({ error: 'URL parameter required' });
        const debug = process.env.VIDZEE_DEBUG === '1';
        if (debug) console.log('[m3u8-proxy] Received URL:', targetUrl);
        
        let headers = {};
        try { headers = JSON.parse(req.query.headers || '{}'); } catch {
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
        
        // Normalize headers to avoid duplicates (especially User-Agent)
        const mergedHeaders = { ...headers };
        const hasUA = Object.keys(mergedHeaders).some(k => k.toLowerCase() === 'user-agent');
        if (!hasUA) {
            mergedHeaders['User-Agent'] = DEFAULT_UA;
        }

        try {
            const response = await fetchWithRetry(targetUrl, { headers: mergedHeaders });
            if (!response.ok) return res.status(response.status).json({ error: `M3U8 fetch failed: ${response.status}`, target: targetUrl });
            
            const contentType = response.headers.get('content-type') || '';
            const isVideo = contentType.includes('video/') || /\.(mp4|mkv|mov|avi|ts)$/i.test(targetUrl.split('?')[0]);
            
            if (isVideo) {
                if (debug) console.log('[m3u8-proxy] Detected non-M3U8 content type, piping raw', { contentType, targetUrl });
                res.setHeader('Content-Type', contentType || 'video/mp4');
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
            console.error('[m3u8-proxy] headers', headers);
            res.status(500).json({ error: e.message, details: e.stack });
        }
    });

    // ts / key segment proxy
    app.get('/ts-proxy', cors(), async (req, res) => {
        let targetUrl = req.query.url; if (!targetUrl) return res.status(400).json({ error: 'URL parameter required' });
        if (!/^https?:\/\//i.test(targetUrl)) {
            return res.status(400).json({ error: 'Invalid target URL', target: targetUrl });
        }
        const debug = req.query.debug === '1';
        const noSynth = req.query.noSynth === '1';
        const force200 = req.query.force200 === '1';
        const clampOpen = req.query.clampOpen === '1'; // default off to avoid upstream rejections
        const progressiveOpen = req.query.progressiveOpen === '1';
        const tailPrefetchEnabled = req.query.tailPrefetch !== '0';
        let tailPrefetchKB = parseInt(req.query.tailPrefetchKB || '256', 10);
        if (isNaN(tailPrefetchKB) || tailPrefetchKB < 64) tailPrefetchKB = 256;
        if (tailPrefetchKB > 2048) tailPrefetchKB = 2048; // max 2MB tail window
        let openChunkKB = parseInt(req.query.openChunkKB || '4096', 10);
        if (isNaN(openChunkKB) || openChunkKB < 64) openChunkKB = 4096;
        if (openChunkKB > 16384) openChunkKB = 16384; // cap at 16MB
        let initChunkKB = parseInt(req.query.initChunkKB || '512', 10);
        if (isNaN(initChunkKB) || initChunkKB < 64) initChunkKB = 512;
        if (initChunkKB > 2048) initChunkKB = 2048; // hard cap 2MB
        let headers = {}; try { headers = JSON.parse(req.query.headers || '{}'); } catch {
            // Ignore URL parsing errors
        }
        if (headers.Referer && !headers.Origin) {
            try { headers.Origin = new URL(headers.Referer).origin; } catch (_e) { /* ignore */ }
        }

        // Normalize headers to avoid duplicates
        const hasUA = Object.keys(headers).some(k => k.toLowerCase() === 'user-agent');
        if (!hasUA) {
            headers['User-Agent'] = DEFAULT_UA;
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
            if (debug && contentLength) console.log('[ts-proxy] size determined', contentLength);

            // Initiate tail prefetch asynchronously (once) when size known and feature enabled
            if (tailPrefetchEnabled && contentLength && !tailPrefetchMap.has(targetUrl)) {
                const total = parseInt(contentLength, 10);
                if (!isNaN(total) && total > 0) {
                    const tailBytes = Math.min(tailPrefetchKB * 1024, total);
                    const start = Math.max(0, total - tailBytes);
                    const tailRange = `bytes=${start}-`;
                    (async () => {
                        try {
                            if (debug) console.log('[ts-proxy] tail prefetch start', { tailRange });
                            const tr = await fetchWithRetry(targetUrl, { headers: { 'User-Agent': DEFAULT_UA, ...headers, Range: tailRange } });
                            if (tr.status === 206) {
                                const buf = Buffer.from(await tr.arrayBuffer());
                                const cr = tr.headers.get('content-range');
                                if (cr) {
                                    const mm = cr.match(/bytes\s+(\d+)-(\d+)\/(\d+)/i);
                                    if (mm) {
                                        const s = parseInt(mm[1], 10); const e = parseInt(mm[2], 10); const sz = parseInt(mm[3], 10);
                                        if (!isNaN(s) && !isNaN(e) && !isNaN(sz)) {
                                            tailPrefetchMap.set(targetUrl, { data: buf, start: s, end: e, size: sz, ts: Date.now() });
                                            if (debug) console.log('[ts-proxy] tail prefetch success', { start: s, end: e, bytes: buf.length });
                                        }
                                    }
                                }
                            } else if (debug) {
                                console.log('[ts-proxy] tail prefetch status', tr.status);
                            }
                        } catch (e) { if (debug) console.log('[ts-proxy] tail prefetch error', e.message); }
                    })();
                }
            }

            // If no range requested but we now know size (HEAD or probe), synthesize an initial small range fetch to accelerate playback.
            if (!force200 && !effectiveRange && contentLength && !noSynth && req.query.progressiveOpen === '0') {
                const total = parseInt(contentLength, 10);
                const desired = initChunkKB * 1024;
                const chunkSize = Math.min(desired, Math.max(0, total - 1));
                const syntheticRange = `bytes=0-${chunkSize}`;
                const resp = await fetchWithRetry(targetUrl, { headers: { 'User-Agent': DEFAULT_UA, ...headers, 'Range': syntheticRange } });
                if (resp.status === 206) {
                    if (debug) console.log('[ts-proxy] synthetic 206', syntheticRange);
                    const upstreamCT = inferContentType(resp.headers.get('content-type'), targetUrl);
                    res.status(206);
                    res.setHeader('Content-Type', upstreamCT);
                    res.setHeader('Accept-Ranges', upstreamAcceptRanges || 'bytes');
                    const contentRange = resp.headers.get('content-range');
                    if (contentRange) res.setHeader('Content-Range', contentRange);
                    const cl = resp.headers.get('content-length'); if (cl) res.setHeader('Content-Length', cl);
                    res.setHeader('Cache-Control', 'public, max-age=3600');
                    res.setHeader('Access-Control-Allow-Origin', '*');
                    // Pipe first chunk then leave connection open? Simpler: end after chunk; player will request next range.
                    return resp.body.pipe(res);
                }
                // If server ignored range (e.g., returned 200), fall through to normal logic below.
            }

            const upstreamOptions = { headers: { 'User-Agent': DEFAULT_UA, ...headers } };
            // If client supplied a suffix range like bytes=0- we pass it; force200 strips Range
            if (force200 && headers.Range) delete headers.Range;
            let resp = await fetchWithRetry(targetUrl, upstreamOptions);
            if (!resp.ok && resp.status !== 206) {
                if (debug) console.log('[ts-proxy] upstream error', resp.status);
                // Retry once without Range if upstream rejects range / partial
                if (headers.Range) {
                    const { Range, ...rest } = headers;
                    const retryOpts = { headers: { 'User-Agent': DEFAULT_UA, ...rest } };
                    try {
                        const retry = await fetchWithRetry(targetUrl, retryOpts);
                        if (retry.ok || retry.status === 206) {
                            resp = retry;
                        } else {
                            return res.status(retry.status).json({ error: `TS fetch failed: ${retry.status}` });
                        }
                    } catch (retryErr) {
                        if (debug) console.log('[ts-proxy] retry failed', retryErr.message);
                        return res.status(resp.status).json({ error: `TS fetch failed: ${resp.status}` });
                    }
                } else {
                    return res.status(resp.status).json({ error: `TS fetch failed: ${resp.status}` });
                }
            }
            // Forward partial content status when range used
            if (effectiveRange && resp.status === 206 && !force200) { res.status(206); if (debug) console.log('[ts-proxy] forwarding 206', { appliedClamp }); }
            if (force200 && resp.status === 206) {
                // Some origins still return 206 even without Range; normalize to 200 for players expecting full stream
                if (debug) console.log('[ts-proxy] normalizing 206 -> 200 due to force200');
            }
            // Copy key streaming headers
            const upstreamCT = inferContentType(resp.headers.get('content-type'), targetUrl);
            res.setHeader('Content-Type', upstreamCT);
            let cl = resp.headers.get('content-length');
            const crHeader = resp.headers.get('content-range');
            if (!cl && crHeader) {
                // Try to derive length from range span
                const m = crHeader.match(/bytes\s+(\d+)-(\d+)\/(\d+|\*)/i);
                if (m) {
                    const start = parseInt(m[1], 10); const end = parseInt(m[2], 10);
                    if (!isNaN(start) && !isNaN(end)) cl = (end - start + 1).toString();
                }
            }
            if (cl) res.setHeader('Content-Length', cl);
            const acceptRanges = resp.headers.get('accept-ranges');
            if (acceptRanges) res.setHeader('Accept-Ranges', acceptRanges); else res.setHeader('Accept-Ranges', 'bytes');
            const contentRange = resp.headers.get('content-range'); if (contentRange && !force200) res.setHeader('Content-Range', contentRange);
            res.setHeader('Cache-Control', 'public, max-age=3600');
            res.setHeader('Access-Control-Allow-Origin', '*');
            if (debug) console.log('[ts-proxy] streaming body', { status: resp.status, ct: upstreamCT, cl, contentRange: force200 ? undefined : contentRange });
            resp.body.pipe(res);
        } catch (e) { 
            if (debug) console.log('[ts-proxy] exception', e.message, e.stack); 
            console.error('[ts-proxy] error', e);
            console.error('[ts-proxy] stack', e.stack);
            console.error('[ts-proxy] targetUrl', targetUrl);
            res.status(500).json({ error: e.message, details: e.stack }); 
        }
    });

    // subtitle proxy
    app.get('/sub-proxy', cors(), async (req, res) => {
        const targetUrl = req.query.url; if (!targetUrl) return res.status(400).json({ error: 'url parameter required' });
        let headers = {}; try { headers = JSON.parse(req.query.headers || '{}'); } catch {
            // Ignore URL parsing errors
        }
        try {
            const resp = await fetchWithRetry(targetUrl, { headers: { 'User-Agent': DEFAULT_UA, ...headers } });
            if (!resp.ok) return res.status(resp.status).json({ error: `subtitle fetch failed: ${resp.status}` });
            res.setHeader('Access-Control-Allow-Origin', '*');
            res.setHeader('Cache-Control', 'public, max-age=3600');
            res.setHeader('Content-Type', resp.headers.get('content-type') || 'text/vtt');
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
        const headers = s.headers || {};
        const hParam = Object.keys(headers).length ? `&headers=${encodeURIComponent(JSON.stringify(headers))}` : '';
        let host = '';
        try {
            host = new URL(original).hostname.toLowerCase();
        } catch {
            // Ignore errors
        }
        // Force specific hosts through ts-proxy (direct file style) even without extension
        if (host.includes('pixeldrain.') || host === 'video-downloads.googleusercontent.com') {
            return { ...s, url: `${serverUrl}/ts-proxy?url=${encodeURIComponent(original)}${hParam}` };
        }
        if (/\.(mp4|mkv)(\?|$)/i.test(original)) {
            return { ...s, url: `${serverUrl}/ts-proxy?url=${encodeURIComponent(original)}${hParam}` };
        }
        return { ...s, url: `${serverUrl}/m3u8-proxy?url=${encodeURIComponent(original)}${hParam}` };
    });
}

module.exports = { createProxyRoutes, processStreamsForProxy };
