/**
 * Hue Proxy Layer
 * Handles secure stream proxying, HLS rewriting, and headers.
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @license MIT
 */

// Lightweight proxy/m3u8/segment/subtitle rewriting layer. thanks to https://github.com/cinepro-org/ for the original code.
// Adapted from external project (user supplied) to fit CommonJS style and existing config system.
// Provides conditional route registration via enableProxy flag.

const cors = require('cors');
const fetch = require('node-fetch');
const fs = require('fs');
const http = require('http');
const https = require('https');
const os = require('os');
const path = require('path');
const { spawn } = require('child_process');
const { encryptUrl, decryptUrl } = require('../utils/security');
let cryptoJs;

// Force persistent connections for proxy requests to avoid latency
const httpAgent = new http.Agent({ 
    keepAlive: true, 
    maxSockets: 1000, // Increased for 16GB VPS
    maxFreeSockets: 128,
    timeout: 30000, 
    family: 4 
});
const httpsAgent = new https.Agent({ 
    keepAlive: true, 
    maxSockets: 1000, // Increased for 16GB VPS
    maxFreeSockets: 128,
    timeout: 30000, 
    family: 4 
});

// Retry configuration for fetch operations
const MAX_RETRIES = 1; // Reduced for faster failure detection
const RETRY_DELAY_MS = 100;
const FETCH_TIMEOUT_MS = 30000; // 30s is enough for segments

// Helper function to fetch with retry logic
async function fetchWithRetry(url, options = {}, maxRetries = MAX_RETRIES) {
    // Prevent fetching localhost or internal IPs
    try {
        const u = new URL(url);
        const host = u.hostname.toLowerCase();
        if (host === 'localhost' || host === '127.0.0.1' || host === '0.0.0.0' || host.startsWith('192.168.') || host.startsWith('10.')) {
            throw new Error(`Blocked internal/local URL: ${url}`);
        }
    } catch (e) {
        if (e.message.includes('Blocked')) throw e;
    }

    const isSegment = /\.(ts|m4s|mp4|mkv)$/i.test(url.split('?')[0]);
    const fetchTimeout = options.timeout || FETCH_TIMEOUT_MS;
    // Allow 1 retry for segments if they fail (helps with transient network blips)
    const actualRetries = isSegment ? 2 : maxRetries;

    const fetchOptions = {
        ...options,
        agent: (parsedUrl) => (parsedUrl.protocol === 'http:' ? httpAgent : httpsAgent),
        timeout: fetchTimeout,
        compress: !isSegment, // Don't compress segments, they are already binary
        size: 0,
        follow: 5
    };
    
    let lastError;
    for (let attempt = 1; attempt <= actualRetries; attempt++) {
        try {
            return await fetch(url, fetchOptions);
        } catch (error) {
            lastError = error;
            if (attempt >= actualRetries) throw error;
            // Short delay for retries to keep things moving
            await new Promise(resolve => setTimeout(resolve, RETRY_DELAY_MS));
        }
    }
    throw lastError;
}

const CACHE_MAX_SIZE = 5000; // Increased for 16GB RAM
const CACHE_EXPIRY_MS = 4 * 60 * 60 * 1000; // 4 hours TTL
const segmentCache = new Map();
// Track first open-ended (bytes=0-) range per target to clamp only once per TTL window
const openRangeClampMap = new Map();
const OPEN_RANGE_CLAMP_TTL_MS = 5 * 60 * 1000; // 5 minutes
// Progressive open-ended growth tracking: keeps expanding bytes=0- served window.
const progressiveOpenMap = new Map(); // url -> { lastEnd }
// Tail prefetch cache: stores last N KB for quick tail range responses
const tailPrefetchMap = new Map(); // url -> { data: Buffer, start: number, end: number, size: number, ts: number }
const TAIL_PREFETCH_TTL_MS = 10 * 60 * 1000; // 10 minutes
const HLS_DOWNLOAD_TIMEOUT_MS = Number(process.env.HLS_DOWNLOAD_TIMEOUT_MS) || (15 * 60 * 1000);

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
        const resp = await fetchWithRetry(url, { headers: { 'User-Agent': DEFAULT_UA, ...headers } });
        if (!resp.ok) return;
        const data = new Uint8Array(await resp.arrayBuffer());
        const responseHeaders = {};
        resp.headers.forEach((v, k) => responseHeaders[k] = v);
        segmentCache.set(url, { data, headers: responseHeaders, timestamp: Date.now() });
    } catch (_e) { /* ignore */ }
}

const DEFAULT_UA = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36';

// ── Ad-Surgery (HLS Manifest) ──────────────────────────────────────────────────

const AD_KEYWORDS = ['googleads', 'googlesyndication', 'doubleclick', 'adsystem', 'adserver', 'google-analytics', 'googletagmanager', 'ads', 'pre-roll', 'mid-roll', 'post-roll', 'cdn.ads'];
const SUSPICIOUS_DURATION = 2.0;

function parseExtinfDuration(line) {
    const m = line.match(/^#EXTINF:([\d.]+)/);
    return m ? parseFloat(m[1]) : 0;
}

function isAdUrl(url) {
    if (!url) return false;
    const lower = url.toLowerCase();
    return AD_KEYWORDS.some(kw => lower.includes(kw));
}

function isAdSegment(extinfLine, urlLine) {
    const duration = parseExtinfDuration(extinfLine);
    if (duration > 0 && duration < SUSPICIOUS_DURATION) return true;
    return isAdUrl(urlLine);
}

function sanitizeM3u8Manifest(content) {
    const lines = content.split(/\r?\n/);
    const output = [];
    let removed = 0;
    let maxDuration = 0;
    let targetDurationIdx = -1;

    for (let i = 0; i < lines.length; i++) {
        if (lines[i].startsWith('#EXT-X-TARGETDURATION:')) {
            targetDurationIdx = i;
        }
    }

    let i = 0;
    while (i < lines.length) {
        const line = lines[i];

        if (line.trim() === '#EXT-X-DISCONTINUITY') {
            if (i + 2 < lines.length && isAdSegment(lines[i + 1], lines[i + 2])) {
                i += 3;
                removed++;
                continue;
            }
            output.push(line);
            i++;
            continue;
        }

        if (line.startsWith('#EXTINF:')) {
            const duration = parseExtinfDuration(line);
            const urlLine = i + 1 < lines.length ? lines[i + 1] : '';

            if (isAdSegment(line, urlLine)) {
                i += 2;
                removed++;
                continue;
            }

            if (duration > maxDuration) maxDuration = duration;
            output.push(line);
            if (urlLine && !urlLine.startsWith('#')) {
                output.push(urlLine);
                i += 2;
                continue;
            }
            i++;
            continue;
        }

        if (!line.startsWith('#') && line.trim()) {
            if (isAdUrl(line)) {
                i++;
                removed++;
                continue;
            }
        }

        output.push(line);
        i++;
    }

    if (targetDurationIdx >= 0 && maxDuration > 0 && targetDurationIdx < output.length) {
        output[targetDurationIdx] = `#EXT-X-TARGETDURATION:${Math.ceil(maxDuration)}`;
    }

    return { manifest: output.join('\n'), removed };
}

// ── TS Sanitization (Segment-Level) ────────────────────────────────────────────

const TS_PACKET_SIZE = 188;
const TS_SYNC_BYTE = 0x47;
const AD_PIDS = new Set([0x0100, 0x1FFF]);
const AD_KEYWORDS_BUF = ['googlesyndication', 'doubleclick', 'adsystem', 'adserver'];

function isValidTs(buffer) {
    return buffer.length >= TS_PACKET_SIZE && buffer[0] === TS_SYNC_BYTE && buffer.length % TS_PACKET_SIZE === 0;
}

function extractPid(packet) {
    return ((packet[1] & 0x1F) << 8) | packet[2];
}

function containsAdSignature(packet) {
    const hasAdaptation = (packet[3] & 0x20) !== 0;
    const payloadStart = hasAdaptation ? 5 + packet[4] : 4;
    if (payloadStart >= packet.length) return false;
    const payload = packet.subarray(payloadStart);
    if (payload.length > 3 && payload[0] === 0xFC) return true; // SCTE-35
    const checkLen = Math.min(payload.length, 64);
    const payloadStr = payload.subarray(0, checkLen).toString('ascii');
    return AD_KEYWORDS_BUF.some(kw => payloadStr.includes(kw));
}

function sanitizeTsSegment(buffer) {
    if (!isValidTs(buffer)) {
        return { cleaned: buffer, stripped: 0 };
    }
    const output = Buffer.allocUnsafe(buffer.length);
    let outOffset = 0;
    let stripped = 0;

    for (let offset = 0; offset + TS_PACKET_SIZE <= buffer.length; offset += TS_PACKET_SIZE) {
        const packet = buffer.subarray(offset, offset + TS_PACKET_SIZE);
        if (packet[0] !== TS_SYNC_BYTE) {
            packet.copy(output, outOffset);
            outOffset += TS_PACKET_SIZE;
            continue;
        }
        const pid = extractPid(packet);
        if (AD_PIDS.has(pid)) {
            stripped++;
            continue;
        }
        if (containsAdSignature(packet)) {
            stripped++;
            continue;
        }
        packet.copy(output, outOffset);
        outOffset += TS_PACKET_SIZE;
    }

    return { cleaned: output.subarray(0, outOffset), stripped };
}

function normalizeTargetUrl(url) {
    if (typeof url !== 'string') return url;
    if (url.startsWith('//')) return `https:${url}`;
    return url;
}

function buildInternalProxyUrl(req, routePath, omitKeys = []) {
    const port = Number(process.env.API_PORT) || Number(req.socket?.localPort) || 24608;
    const internalUrl = new URL(routePath, `http://127.0.0.1:${port}`);
    const omit = new Set(omitKeys);

    Object.entries(req.query || {}).forEach(([key, value]) => {
        if (omit.has(key) || value == null) return;
        if (Array.isArray(value)) {
            value.forEach(item => {
                if (item != null) internalUrl.searchParams.append(key, String(item));
            });
            return;
        }
        internalUrl.searchParams.set(key, String(value));
    });

    return internalUrl.toString();
}

function sanitizeDownloadFilename(rawName, fallback = 'video.mp4') {
    const value = typeof rawName === 'string' ? rawName.trim() : '';
    const cleaned = value
        .replace(/[\r\n]/g, ' ')
        .replace(/[<>:"/\\|?*\x00-\x1f]/g, '_')
        .replace(/\s+/g, ' ')
        .trim();
    const base = cleaned || fallback;
    return /\.[a-z0-9]{2,5}$/i.test(base) ? base : `${base}.mp4`;
}

function runFfmpegDownload(inputUrl, outputPath, mode) {
    const inputOptions = [
        '-allowed_extensions', 'ALL',
        '-allowed_segment_extensions', 'ALL',
        '-extension_picky', '0',
        '-protocol_whitelist', 'file,http,https,tcp,tls,crypto,data'
    ];
    const args = mode === 'transcode'
        ? [
            ...inputOptions,
            '-y',
            '-nostdin',
            '-hide_banner',
            '-loglevel', 'error',
            '-i', inputUrl,
            '-map', '0:v:0?',
            '-map', '0:a?',
            '-sn',
            '-c:v', 'libx264',
            '-preset', 'veryfast',
            '-crf', '23',
            '-c:a', 'aac',
            '-b:a', '128k',
            '-movflags', '+faststart',
            outputPath
        ]
        : [
            ...inputOptions,
            '-y',
            '-nostdin',
            '-hide_banner',
            '-loglevel', 'error',
            '-i', inputUrl,
            '-map', '0:v:0?',
            '-map', '0:a?',
            '-sn',
            '-c', 'copy',
            '-bsf:a', 'aac_adtstoasc',
            '-movflags', '+faststart',
            outputPath
        ];

    return new Promise((resolve, reject) => {
        const ffmpeg = spawn('ffmpeg', args, { stdio: ['ignore', 'ignore', 'pipe'] });
        let stderr = '';
        const startedAt = Date.now();
        const timeoutId = setTimeout(() => {
            if (!ffmpeg.killed) ffmpeg.kill('SIGKILL');
        }, HLS_DOWNLOAD_TIMEOUT_MS);

        ffmpeg.stderr.on('data', (chunk) => {
            if (stderr.length < 16384) stderr += chunk.toString();
        });

        ffmpeg.on('error', (error) => {
            clearTimeout(timeoutId);
            reject(error);
        });
        ffmpeg.on('close', (code) => {
            clearTimeout(timeoutId);
            if (code === 0) return resolve();
            const elapsedMs = Date.now() - startedAt;
            const details = stderr.trim() || `ffmpeg exited with code ${code} after ${elapsedMs}ms`;
            const error = new Error(details);
            error.code = code;
            reject(error);
        });
    });
}

async function streamHlsDownload(req, res) {
    const inputUrl = buildInternalProxyUrl(req, '/m3u8-proxy', ['download', 'filename']);
    const filename = sanitizeDownloadFilename(req.query.filename, 'video.mp4');
    const tempDir = await fs.promises.mkdtemp(path.join(os.tmpdir(), 'hue-hls-'));
    const outputPath = path.join(tempDir, filename);
    const startedAt = Date.now();
    const cleanup = async () => {
        await fs.promises.rm(tempDir, { recursive: true, force: true }).catch(() => undefined);
    };

    try {
        console.log('[download-hls] starting', {
            filename,
            timeoutMs: HLS_DOWNLOAD_TIMEOUT_MS,
            source: inputUrl
        });
        try {
            console.log('[download-hls] mode=copy');
            await runFfmpegDownload(inputUrl, outputPath, 'copy');
            console.log('[download-hls] copy mode succeeded');
        } catch (copyError) {
            console.error('[download-hls] copy mode failed:', copyError.message);
            console.log('[download-hls] mode=transcode');
            await runFfmpegDownload(inputUrl, outputPath, 'transcode').catch((transcodeError) => {
                console.error('[download-hls] transcode mode failed:', transcodeError.message);
                const error = new Error(transcodeError.message || copyError.message);
                error.cause = { copy: copyError.message, transcode: transcodeError.message };
                throw error;
            });
            console.log('[download-hls] transcode mode succeeded');
        }

        const stat = await fs.promises.stat(outputPath);
        if (!stat.size) {
            throw new Error('ffmpeg produced an empty output file');
        }
        console.log('[download-hls] completed', {
            filename,
            sizeBytes: stat.size,
            elapsedMs: Date.now() - startedAt
        });

        res.status(200);
        res.setHeader('Content-Type', 'video/mp4');
        res.setHeader('Content-Disposition', `attachment; filename="${filename}"`);
        res.setHeader('Content-Length', String(stat.size));
        res.setHeader('Cache-Control', 'no-store');
        res.setHeader('Access-Control-Allow-Origin', '*');

        const stream = fs.createReadStream(outputPath);
        stream.on('error', async (error) => {
            console.error('[download-hls] readback failed:', error.message);
            if (!res.headersSent) {
                res.status(500).json({ error: `download read failed: ${error.message}` });
            } else {
                res.destroy(error);
            }
            await cleanup();
        });
        stream.on('close', cleanup);
        stream.pipe(res);
    } catch (error) {
        await cleanup();
        const details = error?.cause ? error.cause : error.message;
        return res.status(502).json({ error: 'HLS download failed', details });
    }
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
        while (depth < 5 && (candidate.includes('/m3u8-proxy') || candidate.includes('/ts-proxy') || candidate.includes('url=') || candidate.includes('h_token='))) {
            let next = null;
            try {
                // Try parsing as a full URL first
                const urlObj = new URL(candidate.startsWith('http') ? candidate : 'http://localhost' + (candidate.startsWith('/') ? '' : '/') + candidate);
                if (urlObj.searchParams.has('h_token')) {
                    const decrypted = decryptUrl(urlObj.searchParams.get('h_token'));
                    next = decrypted ? decrypted.url : null;
                } else if (urlObj.searchParams.has('url')) {
                    next = urlObj.searchParams.get('url');
                } else if (urlObj.pathname.includes('/proxy/')) {
                    const m = urlObj.pathname.match(/\/proxy\/(.+)$/);
                    if (m) next = decodeURIComponent(m[1]);
                }
            } catch {
                // regex fallback for mangled URLs
                const m = candidate.match(/(url|h_token)=([^&]+)/);
                if (m) {
                    const val = decodeURIComponent(m[2]);
                    if (m[1] === 'h_token') {
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

function buildHeaderQuery(headers) {
    if (!headers || typeof headers !== 'object') return '';
    const skip = ['host', 'connection', 'content-length', 'accept-encoding'];
    const filtered = {};
    Object.keys(headers).forEach(k => {
        if (!skip.includes(k.toLowerCase())) filtered[k] = headers[k];
    });
    const keys = Object.keys(filtered);
    return keys.length ? `&headers=${encodeURIComponent(JSON.stringify(filtered))}` : '';
}

function isVixsrcTarget(targetUrl, headers) {
    if (typeof targetUrl === 'string' && targetUrl.includes('vixsrc.to')) return true;
    const referer = headers && (headers.Referer || headers.referer);
    return typeof referer === 'string' && referer.includes('vixsrc.to');
}

function rewriteM3u8(content, targetUrl, baseProxyUrl, headers, options = {}) {
    const lines = content.split('\n');
    const out = []; const segmentUrls = [];
    let nextLineIsStreamUrl = false;
    const vixsrcMode = isVixsrcTarget(targetUrl, headers);
    const urlMode = options.urlMode === true;
    const useUrlMode = vixsrcMode || urlMode;
    const headerPart = useUrlMode ? buildHeaderQuery(headers) : '';
    const modePart = useUrlMode ? '&mode=url' : '';
    
    for (const line of lines) {
        if (line.startsWith('#')) {
            if (line.startsWith('#EXT-X-KEY:')) {
                const uriMatch = line.match(/URI="([^"]+)"/);
                if (uriMatch) {
                    try {
                        const absKey = new URL(uriMatch[1], targetUrl).href;
                        const proxyUrl = useUrlMode
                            ? `${baseProxyUrl}/ts-proxy?url=${encodeURIComponent(absKey)}${headerPart}&clampOpen=0&progressiveOpen=0&noSynth=1&force200=1&tailPrefetch=0`
                            : `${baseProxyUrl}/ts-proxy?h_token=${encodeURIComponent(encryptUrl(absKey, null, headers))}`;
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
                        const proxyUrl = useUrlMode
                            ? `${baseProxyUrl}/m3u8-proxy?url=${encodeURIComponent(mediaUrl)}${headerPart}${modePart}`
                            : `${baseProxyUrl}/m3u8-proxy?h_token=${encodeURIComponent(encryptUrl(mediaUrl, null, headers))}`;
                        out.push(line.replace(uriMatch[1], proxyUrl));
                    } catch { out.push(line); }
                } else out.push(line);
            } else if (line.startsWith('#EXT-X-MAP:')) {
                const uriMatch = line.match(/URI="([^"]+)"/);
                if (uriMatch) {
                    try {
                        const mapUrl = new URL(uriMatch[1], targetUrl).href;
                        const proxyUrl = useUrlMode
                            ? `${baseProxyUrl}/ts-proxy?url=${encodeURIComponent(mapUrl)}${headerPart}&clampOpen=0&progressiveOpen=0&noSynth=1&force200=1&tailPrefetch=0`
                            : `${baseProxyUrl}/ts-proxy?h_token=${encodeURIComponent(encryptUrl(mapUrl, null, headers))}`;
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

                const isMp3WrappedTs = /\.mp3(\?|$)/i.test(abs);
                if (/\.(ts|mp4|png|jpg|jpeg|bmp)(\?|$)/i.test(abs) || isMp3WrappedTs) {
                    segmentUrls.push(abs);
                    const forceTypeParam = isMp3WrappedTs ? '&forceType=video%2Fmp2t' : '';
                    const segmentExtraParams = '&clampOpen=0&progressiveOpen=0&noSynth=1&tailPrefetch=0';
                    
                    out.push(
                        useUrlMode
                            ? `${baseProxyUrl}/ts-proxy?url=${encodeURIComponent(abs)}${headerPart}${segmentExtraParams}${forceTypeParam}`
                            : `${baseProxyUrl}/ts-proxy?h_token=${encodeURIComponent(encryptUrl(abs, null, headers))}${segmentExtraParams}${forceTypeParam}`
                    );
                } else {
                    // Default: proxy any HTTP/S child URL through m3u8-proxy so nested playlists (even without .m3u8 extension) get rewritten.
                    const proxyUrl = useUrlMode
                        ? `${baseProxyUrl}/m3u8-proxy?url=${encodeURIComponent(abs)}${headerPart}${modePart}`
                        : `${baseProxyUrl}/m3u8-proxy?h_token=${encodeURIComponent(encryptUrl(abs, null, headers))}`;
                    out.push(nextLineIsStreamUrl ? proxyUrl : proxyUrl);
                    nextLineIsStreamUrl = false;
                }
            } catch { out.push(line); }
        } else {
            out.push(line);
            nextLineIsStreamUrl = false;
        }
    }

    // --- RAM TURBO: PREFETCH NEXT 5 SEGMENTS ---
    if (segmentUrls.length > 0 && !isCacheDisabled()) {
        (async () => {
            // We prefetch the first 5 segments of any playlist to make the initial load instant
            const toPrefetch = segmentUrls.slice(0, 5);
            for (const url of toPrefetch) {
                if (!segmentCache.has(url)) {
                    prefetchSegment(url, headers).catch(() => {});
                }
            }
        })();
    }

    return out.join('\n');
}

function createProxyRoutes(app) {
    // m3u8 playlist proxy
    app.get('/m3u8-proxy', cors(), async (req, res) => {
        let targetUrl = req.query.url;
        const token = req.query.h_token;
        let decryptedHeaders = null;
        if (token) {
            const decrypted = decryptUrl(token);
            if (!decrypted) return res.status(403).json({ error: 'Invalid or expired token' });
            targetUrl = decrypted.url;
            decryptedHeaders = decrypted.headers;
        }
        if (!targetUrl) return res.status(400).json({ error: 'URL parameter required' });
        const debug = process.env.VIDZEE_DEBUG === '1';
        if (debug) console.log('[m3u8-proxy] Received URL:', targetUrl);
        
        let headers = {};
        try { headers = JSON.parse(req.query.headers || '{}'); } catch {
            // Ignore URL parsing errors
        }
        if (decryptedHeaders) headers = { ...decryptedHeaders, ...headers };
        const decoded = decodeVidzeeToken(targetUrl);
        if (debug) console.log('[m3u8-proxy] After decodeVidzeeToken:', decoded || 'null');
        if (decoded) targetUrl = decoded;
        if (targetUrl.includes('/m3u8-proxy') || targetUrl.includes('/ts-proxy') || targetUrl.includes('h_token=')) {
            const unwrapped = extractOriginalUrl(targetUrl);
            if (unwrapped && unwrapped !== targetUrl) targetUrl = unwrapped;
        }
        targetUrl = normalizeTargetUrl(targetUrl);

        console.log(`[m3u8-proxy] --> FETCHING: ${targetUrl.substring(0, 100)}...`);

        if (!/^https?:\/\//i.test(targetUrl)) {
            if (debug) console.log('[m3u8-proxy] Invalid URL check failed, targetUrl:', targetUrl);
            return res.status(400).json({ error: 'Invalid target URL', target: targetUrl });
        }
        if (req.query.download === '1') {
            return streamHlsDownload(req, res);
        }
        
        // Normalize headers to avoid duplicates and strip problematic ones
        const mergedHeaders = {};
        const skipHeaders = ['host', 'connection', 'content-length', 'content-type', 'accept-encoding', 'if-none-match', 'if-modified-since', 'range'];
        
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

        // Pass through Range header if present (for MP4s sent to m3u8-proxy)
        if (req.headers.range) {
            mergedHeaders['Range'] = req.headers.range;
        }

        try {
            const t0 = Date.now();
            const response = await fetchWithRetry(targetUrl, { headers: mergedHeaders, redirect: 'manual' });
            console.log(`[m3u8-proxy] <-- UPSTREAM: ${response.status} for ${targetUrl.substring(0, 80)}...`);
            const duration = Date.now() - t0;
            
            if (!response.ok && response.status !== 206) {
                console.error(`[m3u8-proxy] Upstream returned ${response.status} for ${targetUrl}`);
                if (response.status === 403) {
                    console.error('[m3u8-proxy] 403 Upstream Headers:', JSON.stringify(Object.fromEntries(response.headers.entries())));
                }
                return res.status(response.status).json({ error: `M3U8 fetch failed: ${response.status}`, target: targetUrl });
            }
            
            const contentType = response.headers.get('content-type') || '';
            let isHls = contentType.includes('mpegURL') || contentType.includes('x-mpegURL') || targetUrl.includes('.m3u8');
            
            // Buffer body to check signature if content-type is ambiguous
            let body = await response.text();
            if (!isHls && body.trim().startsWith('#EXTM3U')) {
                if (debug) console.log('[m3u8-proxy] Detected HLS content via signature');
                isHls = true;
            }

            if (isHls) {
                // Ad-surgery before rewriting
                const adsurgery = req.query.adsurgery === '1' || process.env.SANITIZE_STREAMS === 'true';
                if (adsurgery) {
                    const { manifest: cleaned, removed } = sanitizeM3u8Manifest(body);
                    if (removed > 0) {
                        console.log(`[m3u8-proxy] Ad-surgery removed ${removed} segments from ${targetUrl.substring(0, 80)}...`);
                    }
                    body = cleaned;
                }

                const protocol = req.headers['x-forwarded-proto'] || req.protocol;
                const baseProxyUrl = `${protocol}://${req.get('host')}`;

                // Prevent recursive rewriting base
                let rewriteBase = response.url;
                const host = req.get('host');
                if ((rewriteBase.includes('localhost') || rewriteBase.includes('127.0.0.1')) && !host.includes('localhost')) {
                    // Only override if we are NOT on localhost ourselves (i.e., we are on a VPS)
                    rewriteBase = targetUrl;
                }

                const urlMode = req.query.mode === 'url';
                const rewritten = rewriteM3u8(body, rewriteBase, baseProxyUrl, mergedHeaders, { urlMode });
                res.setHeader('Content-Type', 'application/vnd.apple.mpegurl');
                res.setHeader('Access-Control-Allow-Origin', '*');
                res.send(rewritten);
            } else {
                // Not HLS, send raw body
                if (debug) console.log('[m3u8-proxy] Sending raw body (not HLS)', { contentType });
                res.status(response.status);
                ['content-type', 'content-length', 'accept-ranges', 'content-range'].forEach(h => {
                    if (response.headers.has(h)) res.setHeader(h, response.headers.get(h));
                });
                res.setHeader('Access-Control-Allow-Origin', '*');
                res.send(body);
            }
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
        let targetUrl = req.query.url;
        const token = req.query.h_token;
        let decryptedHeaders = null;
        if (token) {
            const decrypted = decryptUrl(token);
            if (!decrypted) return res.status(403).json({ error: 'Invalid or expired token' });
            targetUrl = decrypted.url;
            decryptedHeaders = decrypted.headers;
        }
        if (!targetUrl) return res.status(400).json({ error: 'URL parameter required' });
        targetUrl = normalizeTargetUrl(targetUrl);
        if (!/^https?:\/\//i.test(targetUrl)) {
            return res.status(400).json({ error: 'Invalid target URL', target: targetUrl });
        }
        const debug = req.query.debug === '1';
        const noSynth = req.query.noSynth === '1';
        const force200 = req.query.force200 === '1';
        const isDownload = req.query.download === '1';
        const forceType = typeof req.query.forceType === 'string' && /^[a-z0-9.+-]+\/[a-z0-9.+-]+$/i.test(req.query.forceType)
            ? req.query.forceType
            : null;
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
        if (decryptedHeaders) headers = { ...decryptedHeaders, ...headers };
        if (targetUrl.includes('/m3u8-proxy') || targetUrl.includes('/ts-proxy') || targetUrl.includes('h_token=')) {
            const unwrapped = extractOriginalUrl(targetUrl);
            if (unwrapped && unwrapped !== targetUrl) targetUrl = unwrapped;
        }
        if (headers.Referer && !headers.Origin) {
            try { headers.Origin = new URL(headers.Referer).origin; } catch (_e) { /* ignore */ }
        }

        // Normalize headers to avoid duplicates and strip problematic ones
        const skipHeaders = ['host', 'connection', 'content-length', 'content-type', 'accept-encoding', 'if-none-match', 'if-modified-since', 'range'];
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
                const ct = forceType || inferContentType(cached.headers['content-type'], targetUrl);
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
                            const ct = forceType || inferContentType(null, targetUrl);
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

            const upstreamOptions = { headers: { 'User-Agent': DEFAULT_UA, ...headers }, redirect: 'manual' };
            // If client supplied a suffix range like bytes=0- we pass it; force200 strips Range
            if (force200 && headers.Range) delete headers.Range;
            let resp = await fetchWithRetry(targetUrl, upstreamOptions);
            
            // Handle redirects manually to prevent redirection to localhost
            if (resp.status >= 300 && resp.status < 400) {
                const location = resp.headers.get('location');
                if (location && (location.includes('localhost') || location.includes('127.0.0.1'))) {
                    console.error(`[ts-proxy] Blocked redirect to localhost: ${location} from ${targetUrl}`);
                    return res.status(403).json({ error: 'Blocked redirect to internal network', origin: targetUrl });
                }
            }

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
                            if (retry.status === 403) {
                                console.error(`[ts-proxy] 403 Forbidden from ${targetUrl}`);
                                console.error('[ts-proxy] 403 Headers:', JSON.stringify(Object.fromEntries(retry.headers.entries())));
                            }
                            return res.status(retry.status).json({ error: `TS fetch failed: ${retry.status}` });
                        }
                    } catch (retryErr) {
                        if (debug) console.log('[ts-proxy] retry failed', retryErr.message);
                        return res.status(resp.status).json({ error: `TS fetch failed: ${resp.status}` });
                    }
                } else {
                    if (resp.status === 403) {
                        console.error(`[ts-proxy] 403 Forbidden from ${targetUrl}`);
                        console.error('[ts-proxy] 403 Headers:', JSON.stringify(Object.fromEntries(resp.headers.entries())));
                    }
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
            const upstreamCT = forceType || inferContentType(resp.headers.get('content-type'), targetUrl);
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
            
            if (isDownload) {
                const filename = req.query.filename || 'video.mp4';
                res.setHeader('Content-Disposition', `attachment; filename="${filename}"`);
            }

            res.setHeader('Cache-Control', 'public, max-age=3600');
            res.setHeader('Access-Control-Allow-Origin', '*');
            if (debug) console.log('[ts-proxy] streaming body', { status: resp.status, ct: upstreamCT, cl, contentRange: force200 ? undefined : contentRange });
            
            // Add a timeout to the body stream to prevent hanging
            const streamTimeout = setTimeout(() => {
                if (!res.headersSent) res.status(504).json({ error: 'Upstream stream timeout' });
                resp.body.destroy(new Error('Stream timeout'));
            }, 30000);

            // Optional TS sanitization (buffer mode — adds latency, use sparingly)
            const sanitize = req.query.sanitize === '1' || process.env.SANITIZE_STREAMS === 'true';
            if (sanitize && !isDownload && !force200 && upstreamCT.includes('mp2t')) {
                const chunks = [];
                resp.body.on('data', chunk => chunks.push(chunk));
                resp.body.on('end', () => {
                    clearTimeout(streamTimeout);
                    const buffer = Buffer.concat(chunks);
                    const { cleaned, stripped } = sanitizeTsSegment(buffer);
                    if (stripped > 0) {
                        console.log(`[ts-proxy] Sanitized ${stripped} packets from ${targetUrl.substring(0, 80)}...`);
                    }
                    res.setHeader('Content-Length', cleaned.length);
                    res.end(cleaned);
                });
                resp.body.on('error', (err) => {
                    clearTimeout(streamTimeout);
                    console.error('[ts-proxy] stream error', err);
                    if (!res.headersSent) res.status(500).end();
                });
            } else {
                resp.body.on('end', () => clearTimeout(streamTimeout));
                resp.body.on('error', () => clearTimeout(streamTimeout));
                resp.body.pipe(res);
            }
        } catch (e) {
            if (debug) console.log('[ts-proxy] exception', e.message, e.stack);
            console.error('[ts-proxy] error', e);
            console.error('[ts-proxy] stack', e.stack);
            console.error('[ts-proxy] targetUrl', targetUrl);
            res.status(500).json({ error: e.message, details: e.stack });
        }
    });

    // subtitle proxy with SRT to WebVTT conversion
    app.get('/sub-proxy', cors(), async (req, res) => {
        let targetUrl = req.query.url;
        const token = req.query.h_token;
        let decryptedHeaders = null;
        if (token) {
            const decrypted = decryptUrl(token);
            if (!decrypted) return res.status(403).json({ error: 'Invalid or expired token' });
            targetUrl = decrypted.url;
            decryptedHeaders = decrypted.headers;
        }
        if (!targetUrl) return res.status(400).json({ error: 'url parameter required' });
        
        let headers = {}; 
        try { headers = JSON.parse(req.query.headers || '{}'); } catch {}
        if (decryptedHeaders) headers = { ...decryptedHeaders, ...headers };

        try {
            const resp = await fetchWithRetry(targetUrl, { headers: { 'User-Agent': DEFAULT_UA, ...headers } });
            if (!resp.ok) return res.status(resp.status).json({ error: `subtitle fetch failed: ${resp.status}` });
            
            const contentType = resp.headers.get('content-type') || '';
            let body = await resp.text();

            // Auto-convert SRT to WebVTT for maximum compatibility
            if (targetUrl.toLowerCase().endsWith('.srt') || body.trim().startsWith('1')) {
                console.log(`[sub-proxy] Converting SRT to WebVTT: ${targetUrl.substring(0, 50)}...`);
                body = 'WEBVTT\n\n' + body
                    .replace(/(\d+:\d+:\d+),(\d+)/g, '$1.$2') // Fix timestamp format
                    .replace(/^\d+\r?\n/gm, ''); // Remove sequence numbers
                res.setHeader('Content-Type', 'text/vtt');
            } else {
                res.setHeader('Content-Type', contentType || 'text/vtt');
            }

            res.setHeader('Access-Control-Allow-Origin', '*');
            res.setHeader('Cache-Control', 'public, max-age=3600');
            res.send(body);
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

        // Only skip if it's already proxied by this same server.
        // If it is proxied by a different host, unwrap and re-proxy locally.
        try {
            const u = new URL(s.url, serverUrl);
            const isProxyPath = u.pathname.includes('/m3u8-proxy') || u.pathname.includes('/ts-proxy');
            if (isProxyPath && u.host === serverHost) {
                return s;
            }
        } catch {}
        
        const original = extractOriginalUrl(s.url);
        
        let host = '';
        try {
            host = new URL(original).hostname.toLowerCase();
        } catch {}
        const isVixsrc = host.includes('vixsrc.to');
        const isMovieBoxCDN = host.includes('hakunaymatata.com') || host.includes('aisports.mobi') || host.includes('poocloud.in');
        const isHlsManifest = s.format === 'hls' || /\.m3u8(\?|$)/i.test(original);

        // Ensure we use the headers provided in the object, or fallback to those in the URL if it was already proxied
        let headers = s.headers || {};
        if (Object.keys(headers).length === 0 && s.url.includes('headers=')) {
            try {
                const u = new URL(s.url, 'http://localhost');
                const hJson = u.searchParams.get('headers');
                if (hJson) headers = JSON.parse(hJson);
            } catch {}
        }

        const token = encodeURIComponent(encryptUrl(original, null, headers));
        const modeParam = s.proxyMode === 'url' ? '&mode=url' : '';
        const hParam = Object.keys(headers).length ? `&headers=${encodeURIComponent(JSON.stringify(headers))}` : '';

        // Pre-proxy subtitles if they exist
        let proxiedSubtitles = s.subtitles;
        if (Array.isArray(s.subtitles)) {
            proxiedSubtitles = s.subtitles.map(sub => {
                if (!sub.url || typeof sub.url !== 'string') return sub;
                const subToken = encodeURIComponent(encryptUrl(sub.url, null, headers));
                return { ...sub, url: `${serverUrl}/sub-proxy?h_token=${subToken}` };
            });
        }

        let proxiedUrl;
        if (isHlsManifest) {
            if (s.proxyMode === 'url') {
                proxiedUrl = `${serverUrl}/m3u8-proxy?url=${encodeURIComponent(original)}${hParam}${modeParam}`;
            } else {
                proxiedUrl = `${serverUrl}/m3u8-proxy?h_token=${token}`;
            }
        } else if (host.includes('pixeldrain.') || host === 'video-downloads.googleusercontent.com' || isMovieBoxCDN) {
            const synthParams = isMovieBoxCDN ? '&noSynth=1' : '';
            proxiedUrl = `${serverUrl}/ts-proxy?h_token=${token}${synthParams}`;
        } else if (/\.(mp4|mkv)(\?|$)/i.test(original)) {
            proxiedUrl = `${serverUrl}/ts-proxy?h_token=${token}`;
        } else if (s.proxyMode === 'url') {
            const forceTsProxy = s.format === 'mp4' || s.format === 'video';
            if (forceTsProxy || /\.(mp4|mkv)(\?|$)/i.test(original)) {
                proxiedUrl = `${serverUrl}/ts-proxy?url=${encodeURIComponent(original)}${hParam}${modeParam}`;
            } else {
                proxiedUrl = `${serverUrl}/m3u8-proxy?url=${encodeURIComponent(original)}${hParam}${modeParam}`;
            }
        } else if (isVixsrc) {
            proxiedUrl = `${serverUrl}/m3u8-proxy?url=${encodeURIComponent(original)}${hParam}`;
        } else {
            proxiedUrl = `${serverUrl}/m3u8-proxy?h_token=${token}`;
        }

        return { ...s, url: proxiedUrl, subtitles: proxiedSubtitles };
    });
}

module.exports = { createProxyRoutes, processStreamsForProxy };
