// Lightweight proxy/m3u8/segment/subtitle rewriting layer.
// Features: Signed URL tokens, automatic header persistence, aggressive failover, and CDN socket tuning.

const cors = require('cors');
const fetch = require('node-fetch');
const http = require('http');
const https = require('https');
const { encryptUrl, decryptUrl } = require('../utils/security');

// Shared agents with optimized TLS and keepAlive settings
const httpAgent = new http.Agent({
    keepAlive: true, 
    maxSockets: 200, 
    timeout: 60000,
    freeSocketTimeout: 30000 
});
const httpsAgent = new https.Agent({
    keepAlive: true, 
    maxSockets: 200, 
    timeout: 60000,
    freeSocketTimeout: 30000
});

// IPv4 Forced Agents for bypassing VPS routing issues
const httpAgentV4 = new http.Agent({ family: 4, keepAlive: true, maxSockets: 100 });
const httpsAgentV4 = new https.Agent({ family: 4, keepAlive: true, maxSockets: 100 });

const DEFAULT_UA = 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36';
const FALLBACK_UA = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36';

const FETCH_TIMEOUT_MS = 60000; 
const MAX_RETRIES = 2;
const RETRY_DELAY_MS = 500;

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

    // Force browser-like headers for common streaming domains
    if (isHostile || host.includes('redirector') || host.includes('cdnsync')) {
        if (!mergedHeaders['User-Agent']) mergedHeaders['User-Agent'] = DEFAULT_UA;
        mergedHeaders['Cache-Control'] = 'no-cache';
        // Auto-generate Origin if missing and Referer exists
        if (mergedHeaders['Referer'] && !mergedHeaders['Origin']) {
            try { mergedHeaders['Origin'] = new URL(mergedHeaders['Referer']).origin; } catch {}
        }
    }

    const actualRetries = isSegment ? 1 : maxRetries;
    const forceIPv4 = isHostile || host.includes('redirector') || host.includes('cdnsync') || host.includes('lightningbolt');

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
            if (attempt > 1) {
                fetchOptions.headers['User-Agent'] = attempt % 2 === 0 ? FALLBACK_UA : DEFAULT_UA;
                useIPv4Forced = true; 
            }
            if (useIPv4Forced) {
                fetchOptions.agent = isHostile ? false : ((parsedUrl) => (parsedUrl.protocol === 'http:' ? httpAgentV4 : httpsAgentV4));
            }
            
            const res = await fetch(url, { ...fetchOptions, timeout: finalTimeout });
            const duration = Date.now() - t0;
            if (isSegment && duration > 10000) console.log(`[fetch] Slow segment (${duration}ms): ${url}`);
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
            if (isNetworkError) useIPv4Forced = true;
            await new Promise(resolve => setTimeout(resolve, RETRY_DELAY_MS * attempt));
        }
    }
    throw lastError;
}

const CACHE_MAX_SIZE = 2000;
const CACHE_EXPIRY_MS = 2 * 60 * 60 * 1000;
const segmentCache = new Map();
const openRangeClampMap = new Map();
const OPEN_RANGE_CLAMP_TTL_MS = 5 * 60 * 1000;
const progressiveOpenMap = new Map();
const tailPrefetchMap = new Map();
const TAIL_PREFETCH_TTL_MS = 10 * 60 * 1000;

function isCacheDisabled() { return process.env.DISABLE_CACHE === 'true'; }

function getCachedSegment(url) {
    if (isCacheDisabled()) return null;
    const e = segmentCache.get(url);
    if (!e) return null;
    if (Date.now() - e.timestamp > CACHE_EXPIRY_MS) { segmentCache.delete(url); return null; }
    return e;
}

async function prefetchSegment(url, headers) {
    if (isCacheDisabled() || segmentCache.size >= CACHE_MAX_SIZE) return;
    try {
        const resp = await fetch(url, { headers: { 'User-Agent': DEFAULT_UA, ...headers } });
        if (!resp.ok) return;
        const data = new Uint8Array(await resp.arrayBuffer());
        const responseHeaders = {};
        resp.headers.forEach((v, k) => responseHeaders[k] = v);
        segmentCache.set(url, { data, headers: responseHeaders, timestamp: Date.now() });
    } catch (_e) {}
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
    try {
        if (typeof token !== 'string' || /^https?:\/\//i.test(token.trim()) || token.length < 20) return null;
        let normalized = token.replace(/-/g, '+').replace(/_/g, '/').replace(/\s/g, '+').replace(/[^A-Za-z0-9+\/=]/g, '');
        const pad = normalized.length % 4;
        const fixed = pad ? normalized + '='.repeat(4 - pad) : normalized;
        const buffer = Buffer.from(fixed, 'base64');
        let raw;
        try { raw = buffer.toString('utf8'); } catch (e) { raw = buffer.toString('binary'); }
        if (!raw) return null;
        raw = raw.replace(/[\x00-\x1F\x7F-\x9F]/g, '').trim();
        const urlRegex = /(https?:\/\/[^\s"'\\}{]|\[\]<>]+)/i;
        const match = raw.match(urlRegex);
        if (match) return match[1].split(/[\"'<>{}()[\]]/)[0];
        return null;
    } catch (e) { return null; }
}

function extractOriginalUrl(proxyUrl) {
    try {
        if (!proxyUrl || typeof proxyUrl !== 'string') return proxyUrl;
        let candidate = proxyUrl;
        let depth = 0;
        while (depth < 10 && (candidate.includes('/m3u8-proxy') || candidate.includes('/ts-proxy') || candidate.includes('url=') || candidate.includes('token='))) {
            let next = null;
            try {
                const urlObj = new URL(candidate.startsWith('http') ? candidate : 'http://localhost' + (candidate.startsWith('/') ? '' : '/') + candidate);
                if (urlObj.searchParams.has('token')) {
                    const decrypted = decryptUrl(urlObj.searchParams.get('token'));
                    next = decrypted ? decrypted.url : null;
                } else if (urlObj.searchParams.has('url')) {
                    next = urlObj.searchParams.get('url');
                } else if (urlObj.pathname.includes('/proxy/')) {
                    const m = urlObj.pathname.match(/\/proxy\/(.+)$/);
                    if (m) next = decodeURIComponent(m[1]);
                }
            } catch {
                const m = candidate.match(/(url|token)=([^&]+)/);
                if (m) {
                    const val = decodeURIComponent(m[2]);
                    if (m[1] === 'token') {
                        const decrypted = decryptUrl(val);
                        next = decrypted ? decrypted.url : null;
                    } else next = val;
                }
            }
            if (next && next !== candidate && next !== 'undefined') { candidate = next; depth++; } else break;
        }
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
            if (line.startsWith('#EXT-X-KEY:') || line.startsWith('#EXT-X-MAP:')) {
                const uriMatch = line.match(/URI="([^"]+)"/);
                if (uriMatch) {
                    try {
                        const abs = new URL(uriMatch[1], targetUrl).href;
                        const token = encryptUrl(abs, null, headers);
                        const proxyUrl = `${baseProxyUrl}/ts-proxy?token=${token}&noSynth=1&force200=1`;
                        out.push(line.replace(uriMatch[1], proxyUrl));
                    } catch { out.push(line); }
                } else out.push(line);
            } else if (line.startsWith('#EXT-X-MEDIA:') || line.startsWith('#EXT-X-I-FRAME-STREAM-INF:')) {
                const uriMatch = line.match(/URI="([^"]+)"/);
                if (uriMatch) {
                    try {
                        const abs = new URL(uriMatch[1], targetUrl).href;
                        const token = encryptUrl(abs, null, headers);
                        out.push(line.replace(uriMatch[1], `${baseProxyUrl}/m3u8-proxy?token=${token}`));
                    } catch { out.push(line); }
                } else out.push(line);
            } else if (line.startsWith('#EXT-X-STREAM-INF:')) {
                nextLineIsStreamUrl = true;
                out.push(line);
            } else out.push(line);
        } else if (line.trim()) {
            try {
                let candidate = line.trim();
                const decoded = decodeVidzeeToken(candidate);
                if (decoded) candidate = decoded;
                const abs = new URL(candidate, targetUrl).href;
                const token = encryptUrl(abs, null, headers);
                if (/\.(ts|m4s|mp4|mkv|m4v|m4a|fmp4)(\?|$)/i.test(abs) || abs.includes('/resource/') || abs.includes('/bt/')) {
                    out.push(`${baseProxyUrl}/ts-proxy?token=${token}`);
                } else {
                    out.push(`${baseProxyUrl}/m3u8-proxy?token=${token}`);
                }
                nextLineIsStreamUrl = false;
            } catch { out.push(line); }
        } else { out.push(line); nextLineIsStreamUrl = false; }
    }
    return out.join('\n');
}

function createProxyRoutes(app) {
    app.get('/m3u8-proxy', cors(), async (req, res) => {
        let targetUrl = req.query.url; 
        const token = req.query.token;
        const debug = process.env.VIDZEE_DEBUG === '1';
        let decryptedHeaders = null;

        if (token) {
            const decrypted = decryptUrl(token, req.ip);
            if (!decrypted) return res.status(403).json({ error: 'Invalid or expired token' });
            targetUrl = decrypted.url;
            decryptedHeaders = decrypted.headers;
        } else if (!targetUrl) return res.status(400).json({ error: 'Token or URL required' });

        if (!token && req.ip !== '127.0.0.1' && process.env.STRICT_SECURITY === 'true') return res.status(403).json({ error: 'Strict security enabled' });

        let headers = decryptedHeaders || {};
        try { const qh = JSON.parse(req.query.headers || '{}'); headers = { ...headers, ...qh }; } catch {}
        
        const mergedHeaders = {};
        const skip = ['host', 'connection', 'content-length', 'content-type', 'accept-encoding', 'cookie', 'range', 'sec-fetch-mode', 'sec-fetch-site', 'sec-fetch-dest', 'sec-fetch-user'];
        Object.keys(headers).forEach(k => { if (!skip.includes(k.toLowerCase())) mergedHeaders[k] = headers[k]; });
        if (!mergedHeaders['User-Agent']) mergedHeaders['User-Agent'] = DEFAULT_UA;

        try {
            const t0 = Date.now();
            let currentUrl = targetUrl;
            let response = null;
            let redirectCount = 0;
            while (redirectCount <= 5) {
                response = await fetchWithRetry(currentUrl, { headers: mergedHeaders, redirect: 'manual', timeout: 30000 });
                if (response.status >= 300 && response.status < 400) {
                    const loc = response.headers.get('location');
                    if (!loc) break;
                    currentUrl = new URL(loc, currentUrl).href;
                    redirectCount++;
                    continue;
                }
                break;
            }

            if (!response.ok && response.status !== 206) return res.status(response.status).json({ error: `Fetch failed ${response.status}`, target: currentUrl });
            
            const contentType = response.headers.get('content-type') || '';
            const isVideo = contentType.includes('video/') || /\.(mp4|mkv|mov|avi|ts|m4s)$/i.test(currentUrl.split('?')[0]);
            
            if (isVideo || response.status === 206) {
                res.status(response.status);
                res.setHeader('Content-Type', contentType || 'video/mp4');
                ['content-range', 'accept-ranges', 'content-length'].forEach(h => { if(response.headers.get(h)) res.setHeader(h, response.headers.get(h)); });
                res.setHeader('Access-Control-Allow-Origin', '*');
                return response.body.pipe(res);
            }

            const text = await response.text();
            if (!text.trim().startsWith('#EXTM3U') && !targetUrl.includes('.m3u8')) {
                res.setHeader('Content-Type', contentType || 'text/plain');
                return res.send(text);
            }

            const protocol = req.headers['x-forwarded-proto'] || req.protocol || 'http';
            const baseProxyUrl = `${protocol}://${req.get('host')}`;
            const rewritten = rewriteM3u8(text, targetUrl, baseProxyUrl, headers);
            res.setHeader('Content-Type', 'application/vnd.apple.mpegurl');
            res.setHeader('Cache-Control', 'no-cache, no-store');
            res.setHeader('Access-Control-Allow-Origin', '*');
            res.send(rewritten);
        } catch (e) { res.status(500).json({ error: e.message }); }
    });

    app.get('/ts-proxy', cors(), async (req, res) => {
        let targetUrl = req.query.url;
        const token = req.query.token;
        let decryptedHeaders = null;

        if (token) {
            const decrypted = decryptUrl(token, req.ip);
            if (!decrypted) return res.status(403).json({ error: 'Invalid or expired token' });
            targetUrl = decrypted.url;
            decryptedHeaders = decrypted.headers;
        } else if (!targetUrl) {
            return res.status(400).json({ error: 'Token or URL parameter is required' });
        }

        const headers = { ...(decryptedHeaders || {}) };
        const clientRange = req.headers.range;

        // Clean headers and prepare for upstream request
        const upstreamHeaders = {};
        const skipHeaders = ['host', 'connection', 'content-length', 'content-type', 'accept-encoding', 'cookie'];
        Object.keys(headers).forEach(k => {
            if (!skipHeaders.includes(k.toLowerCase())) {
                upstreamHeaders[k] = headers[k];
            }
        });
        if (!upstreamHeaders['User-Agent']) {
            upstreamHeaders['User-Agent'] = DEFAULT_UA;
        }
        if (clientRange) {
            upstreamHeaders.range = clientRange;
        }

        try {
            const upstreamRes = await fetchWithRetry(targetUrl, { headers: upstreamHeaders });

            // Send upstream status code to client
            res.status(upstreamRes.status);

            // Copy all relevant headers from the upstream response to our client response
            ['content-type', 'content-length', 'content-range', 'accept-ranges', 'last-modified', 'etag', 'cache-control'].forEach(headerName => {
                const headerValue = upstreamRes.headers.get(headerName);
                if (headerValue) {
                    res.setHeader(headerName, headerValue);
                }
            });
            res.setHeader('Access-Control-Allow-Origin', '*');

            // Pipe the body directly. This handles all cases (200 OK, 206 Partial Content, etc.)
            upstreamRes.body.pipe(res);

        } catch (error) {
            console.error(`[ts-proxy] Failed to fetch target: ${targetUrl}`, error);
            if (!res.headersSent) {
                res.status(502).json({ error: 'Bad Gateway', message: error.message });
            }
        }
    });

    app.get('/sub-proxy', cors(), async (req, res) => {
        let targetUrl = req.query.url;
        const token = req.query.token;
        let decryptedHeaders = null;
        if (token) {
            const decrypted = decryptUrl(token, req.ip);
            if (!decrypted) return res.status(403).json({ error: 'Invalid or expired token' });
            targetUrl = decrypted.url;
            decryptedHeaders = decrypted.headers;
        } else if (!targetUrl) return res.status(400).json({ error: 'Token or URL required' });

        let headers = decryptedHeaders || {};
        try { const qh = JSON.parse(req.query.headers || '{}'); headers = { ...headers, ...qh }; } catch {}
        const merged = {};
        const skip = ['host', 'connection', 'content-length', 'content-type', 'accept-encoding', 'cookie', 'range'];
        Object.keys(headers).forEach(k => { if (!skip.includes(k.toLowerCase())) merged[k] = headers[k]; });
        if (!merged['User-Agent']) merged['User-Agent'] = DEFAULT_UA;

        try {
            const resp = await fetchWithRetry(targetUrl, { headers: merged, redirect: 'follow' });
            if (!resp.ok) return res.status(resp.status).json({ error: `Subtitle failed ${resp.status}` });
            let ct = resp.headers.get('content-type');
            if (!ct || ct.includes('octet-stream')) {
                if (targetUrl.includes('.vtt')) ct = 'text/vtt';
                else if (targetUrl.includes('.srt')) ct = 'text/plain';
            }
            res.setHeader('Access-Control-Allow-Origin', '*');
            res.setHeader('Content-Type', ct || 'text/vtt');
            resp.body.pipe(res);
        } catch (e) { res.status(500).json({ error: e.message }); }
    });
}

function processStreamsForProxy(streams, serverUrl) {
    if (!Array.isArray(streams)) return streams;
    let serverHost = ''; try { serverHost = new URL(serverUrl).host; } catch {}
    return streams.map(s => {
        if (!s || !s.url || typeof s.url !== 'string' || s.skipProxy === true || s.url.includes('/m3u8-proxy') || s.url.includes('/ts-proxy')) return s;
        const original = extractOriginalUrl(s.url);
        let headers = s.headers || {};
        if (Object.keys(headers).length === 0 && s.url.includes('headers=')) {
            try {
                const u = new URL(s.url, 'http://localhost');
                const hJson = u.searchParams.get('headers');
                if (hJson) headers = JSON.parse(hJson);
            } catch {}
        }
        if (Array.isArray(s.subtitles)) {
            s.subtitles = s.subtitles.map(sub => {
                if (!sub || !sub.url || sub.url.includes('/sub-proxy')) return sub;
                const subOriginal = extractOriginalUrl(sub.url);
                const subToken = encryptUrl(subOriginal, null, headers);
                return { ...sub, url: `${serverUrl}/sub-proxy?token=${subToken}` };
            });
        }
        let host = ''; try { host = new URL(original).hostname.toLowerCase(); } catch {}
        const token = encryptUrl(original, null, headers);
        if (host.includes('pixeldrain.') || host === 'video-downloads.googleusercontent.com' || /\.(mp4|mkv)(\?|$)/i.test(original)) {
            return { ...s, url: `${serverUrl}/ts-proxy?token=${token}` };
        }
        return { ...s, url: `${serverUrl}/m3u8-proxy?token=${token}` };
    });
}

module.exports = { createProxyRoutes, processStreamsForProxy };
