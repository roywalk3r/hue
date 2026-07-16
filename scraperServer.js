/**
 * Hue Stream API
 * Professional Streaming Aggregator
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */

// Polyfill: undici 7.x requires global File (Node.js v20+). We run v18.
if (typeof global.File === 'undefined') {
  global.File = class File {
    constructor(bits, name, options = {}) {
      this.name = name;
      this.size = 0;
      this.type = options.type || '';
      this.lastModified = options.lastModified || Date.now();
    }
    text() { return Promise.resolve(''); }
    arrayBuffer() { return Promise.resolve(new ArrayBuffer(0)); }
    stream() { return new ReadableStream(); }
    slice() { return new Blob([]); }
  };
}

const express = require('express');
const fs = require('fs');
const path = require('path');
const { URL } = require('url');
const sflixProvider = require('./providers/sflix');
const kissasianProvider = require('./providers/kissasian');
const { decrypt: videasyDecrypt } = require('./providers/videasy/decrypt');
const { getNewContext } = require('./utils/browserManager');
const vidsrcEmbedProvider = require('./providers/vidsrc-embed');
const iframeExtractor = require('./providers/iframeExtractor');
const { config } = require('./utils/config');

// Load cf_proxies.txt (host:port:user:pass lines) into config.proxyList as
// http://user:pass@host:port URLs. Only seeds if PROXY_LIST env is unset so
// explicit overrides still win. Runs once at boot — no per-request I/O.
function loadCfProxiesFile() {
    if (config.proxyList && config.proxyList.length > 0) return;
    const filePath = path.join(__dirname, 'cf_proxies.txt');
    if (!fs.existsSync(filePath)) return;
    const lines = fs.readFileSync(filePath, 'utf8')
        .split('\n').map(l => l.trim()).filter(l => l && !l.startsWith('#'));
    const urls = [];
    for (const line of lines) {
        const parts = line.split(':');
        if (parts.length !== 4) continue;
        const [host, port, user, pass] = parts;
        urls.push(`http://${user}:${pass}@${host}:${port}`);
    }
    if (urls.length > 0) {
        config.proxyList = urls;
        console.log(`[Scraper] Loaded ${urls.length} proxies from cf_proxies.txt`);
    }
}
loadCfProxiesFile();

const app = express();
const port = process.env.SCRAPER_PORT || 24610;

app.use(express.json());

app.get('/health', (req, res) => {
    res.json({ status: 'ok', service: 'Playwright Scraper Microservice' });
});

// SFlix Fallback Scraper Route
app.post('/scrape/sflix', async (req, res) => {
    const { url, isMovie, season, episode } = req.body;
    if (!url) return res.status(400).json({ error: 'Missing url parameter' });
    
    try {
        console.log(`[Scraper] Scraping SFlix: ${url}`);
        const streams = await sflixProvider.extractFromUrlWithPlaywright(url, isMovie, season, episode);
        res.json({ success: true, streams: streams || [] });
    } catch (e) {
        console.error(`[Scraper] SFlix error: ${e.message}`);
        res.status(500).json({ success: false, error: e.message });
    }
});

// Videasy Decryption Route
app.post('/scrape/videasy', async (req, res) => {
    const { hexResponse, tmdbId } = req.body;
    if (!hexResponse || !tmdbId) {
        return res.status(400).json({ error: 'Missing hexResponse or tmdbId parameter' });
    }

    try {
        console.log(`[Scraper] Decrypting Videasy for TMDB ${tmdbId}`);
        const decrypted = await videasyDecrypt(hexResponse.trim(), String(tmdbId));

        if (!decrypted || !decrypted.sources || !Array.isArray(decrypted.sources)) {
            return res.json({ success: false, streams: [], subtitles: [] });
        }

        const streams = decrypted.sources
            .filter(s => s.url)
            .map(s => ({
                url: s.url,
                quality: s.quality || 'Auto'
            }));

        const subtitles = (decrypted.subtitles || [])
            .filter(s => s.url)
            .map(s => ({
                url: s.url,
                lang: s.lang || 'en',
                label: s.language || s.lang || 'English'
            }));

        console.log(`[Scraper] Videasy decrypted ${streams.length} streams, ${subtitles.length} subtitles`);
        res.json({ success: true, streams, subtitles });
    } catch (e) {
        console.error(`[Scraper] Videasy error: ${e.message}`);
        res.status(500).json({ success: false, error: e.message, streams: [], subtitles: [] });
    }
});

// KissAsian Fallback Scraper Route
app.post('/scrape/kissasian', async (req, res) => {
    const { url, title, season, episode } = req.body;

    try {
        if (url) {
            console.log(`[Scraper] Scraping KissAsian URL: ${url}`);
            const streams = await kissasianProvider.getStreamsByUrl(url);
            res.json({ success: true, streams: streams || [] });
        } else if (title) {
            console.log(`[Scraper] Scraping KissAsian: ${title} S${season || 1}E${episode || 1}`);
            const streams = await kissasianProvider.getStreams(null, 'tv', season, episode);
            res.json({ success: true, streams: streams || [] });
        } else {
            res.status(400).json({ error: 'Missing url or title parameter' });
        }
    } catch (e) {
        console.error(`[Scraper] KissAsian error: ${e.message}`);
        res.status(500).json({ success: false, error: e.message });
    }
});

// Vidsrc-Embed Browser Scraper Route
app.post('/scrape/vidsrc-embed', async (req, res) => {
    const { tmdbId, mediaType, season, episode } = req.body;
    if (!tmdbId || !mediaType) {
        return res.status(400).json({ error: 'Missing tmdbId or mediaType parameter' });
    }

    try {
        console.log(`[Scraper] Scraping Vidsrc-Embed: ${mediaType} ${tmdbId}`);
        const streams = await vidsrcEmbedProvider.getStreams(tmdbId, mediaType, season, episode);
        if (streams && streams.length > 0) {
            const mapped = streams.map(s => ({
                url: s.url,
                quality: s.quality || 'Auto',
                subtitles: s.subtitles || [],
                headers: s.headers || {}
            }));
            res.json({ success: true, streams: mapped });
        } else {
            res.json({ success: false, streams: [] });
        }
    } catch (e) {
        console.error(`[Scraper] Vidsrc-Embed error: ${e.message}`);
        res.status(500).json({ success: false, error: e.message, streams: [] });
    }
});

// Iframe-to-native stream extraction route
app.post('/extract/iframe', async (req, res) => {
    const { url, referer } = req.body;
    if (!url) {
        return res.status(400).json({ success: false, error: 'Missing url parameter' });
    }

    try {
        console.log(`[Scraper] Extracting iframe: ${url}`);
        const result = await iframeExtractor.extractIframe(url, referer);
        if (result) {
            res.json({ success: true, ...result });
        } else {
            res.json({ success: false, error: 'Extraction returned no stream' });
        }
    } catch (e) {
        console.error(`[Scraper] iframe extraction error: ${e.message}`);
        res.status(500).json({ success: false, error: e.message });
    }
});

// Generic headless browser stream sniffer for any provider.
// Spawns a stealth browser, navigates to the URL, and captures the first
// media manifest or video request that matches known extensions or a custom
// regex. Also supports waiting for a selector, clicking an element, and
// evaluating custom page JS before/after navigation.
//
// Cost-protection guards (when used through a paid proxy):
//  - Hard abort on first media match: the page is told to stop loading the
//    moment a manifest URL is detected, so in-flight asset requests are
//    cancelled and no segment bytes are proxied.
//  - Per-sniffer request count cap (default 60) — guards against runaway
//    page assets (popups, ad chains, infinite redirects).
//  - Video segment requests (`.ts`, `.m4s`, `.mp4` body bytes) are aborted
//    at the route level — only manifests (`.m3u8`, `.mpd`) are captured.
//  - Caller can pass a specific `proxy` (e.g. a CF worker URL) to avoid
//    random rotation that may pick a residential metered proxy.
const MAX_ASSET_REQUESTS = parseInt(process.env.HEADLESS_MAX_REQUESTS || '60', 10);
const SEGMENT_BLOCKLIST = ['.ts', '.m4s', '.mp4', '.mkv', '.webm'];

// Resource types that don't count against the asset cap. Dailymotion (and
// similar players) load 20-30 fonts + 50+ ad/tracker scripts before the user
// gestures. Counting those inflates the cap; only "interesting" requests
// (HTML, XHR/fetch to same-origin, scripts that might build the player URL)
// should burn the budget.
const CAP_EXEMPT_RESOURCE_TYPES = new Set(['font', 'image', 'stylesheet', 'media']);

// Hosts whose requests should be treated as part of the page (the player
// itself loads dozens of scripts/XHRs from these origins while initializing).
// Off-origin tracker/ads (pebed.dm-event.net, imasdk.googleapis.com,
// googlesyndication.com, vendorlist.dmcdn.net, etc.) still count.
const PAGE_ORIGIN_SUFFIXES = [
    'dailymotion.com',
    'dmcdn.net',
];

// Proxies in this Set have been observed to fail auth or refuse CONNECT in
// this process lifetime. Skip them when rotating to avoid burning retries
// on known-bad egress. Cleared only on process restart.
const BLACKLISTED_PROXIES = new Set();

app.post('/scrape/headless', async (req, res) => {
    const {
        url,
        referer,
        timeout = 30000,
        waitFor,
        waitForNetworkIdle = false,
        clickSelector,
        evaluateScript,
        watchUrlRegex,
        mediaExtensions = ['.m3u8', '.mpd', '.mp4', '.mkv', '.webm'],
        debug = false,
        proxy,
        maxRequests,
    } = req.body;

    if (!url) {
        return res.status(400).json({ success: false, error: 'Missing url parameter' });
    }

    const requestCap = Math.min(parseInt(maxRequests || MAX_ASSET_REQUESTS, 10) || MAX_ASSET_REQUESTS, 500);

    // Transient proxy errors worth retrying with a fresh connection. These
    // happen when a residential proxy drops a connection mid-handshake (rate
    // limit, sudden disconnect) or rejects auth on a stale connection.
    const TRANSIENT_PROXY_ERRORS = [
        'ERR_NETWORK_CHANGED',
        'ERR_INVALID_AUTH_CREDENTIALS',
        'ERR_TUNNEL_CONNECTION_FAILED',
        'ERR_PROXY_CONNECTION_FAILED',
    ];
    const MAX_RETRIES = 2;

    let page;
    try {
        for (let attempt = 0; attempt <= MAX_RETRIES; attempt++) {
            // On retry, force a new random proxy if caller didn't pin one.
            // Skip this on the first attempt when proxy was already pinned
            // (caller asked for that specific egress). Also avoid proxies we
            // already observed to fail auth/CONNECT in this process lifetime.
            let proxyForAttempt = proxy;
            if (attempt > 0 && !proxy && config.proxyList && config.proxyList.length > 0) {
                const pool = config.proxyList.filter(p => !BLACKLISTED_PROXIES.has(p));
                if (pool.length > 0) {
                    proxyForAttempt = pool[Math.floor(Math.random() * pool.length)];
                    console.log(`[Scraper] Headless retry ${attempt}: rotating to new proxy (pool: ${pool.length}/${config.proxyList.length})`);
                }
            }

            console.log(`[Scraper] Headless scrape${attempt > 0 ? ` (retry ${attempt})` : ''}: ${url}${proxyForAttempt ? ` (proxy: ${proxyForAttempt.split('@').pop()})` : ''}`);
            const ctxOptions = { browserType: 'chromium' };
            if (proxyForAttempt) {
                try {
                    const u = new URL(proxyForAttempt);
                    ctxOptions.proxy = { server: `${u.protocol}//${u.host}` };
                    if (u.username) ctxOptions.proxy.username = decodeURIComponent(u.username);
                    if (u.password) ctxOptions.proxy.password = decodeURIComponent(u.password);
                } catch (_) {
                    ctxOptions.proxy = { server: proxyForAttempt };
                }
            }
            const ctx = await getNewContext(ctxOptions);
            page = await ctx.newPage();

            try {
                const result = await runHeadlessStreamSniffer(page, {
                    url,
                    referer,
                    timeout,
                    waitFor,
                    waitForNetworkIdle,
                    clickSelector,
                    evaluateScript,
                    watchUrlRegex,
                    mediaExtensions,
                    debug,
                    requestCap,
                });
                res.json({ success: true, ...result });
                return;
            } catch (e) {
                const msg = e.message || '';
                const isTransient = TRANSIENT_PROXY_ERRORS.some(code => msg.includes(code));
                if (isTransient && proxyForAttempt) {
                    // Blacklist this specific proxy URL for the rest of the
                    // process so subsequent scrapes don't rotate back into it.
                    BLACKLISTED_PROXIES.add(proxyForAttempt);
                    console.log(`[Scraper] Blacklisted proxy (${proxyForAttempt.split('@').pop()}) for process lifetime`);
                }
                if (!isTransient || attempt === MAX_RETRIES) {
                    throw e;
                }
                console.log(`[Scraper] Transient error (${msg.substring(0, 60)}), retrying with fresh proxy`);
                try { await ctx.close(); } catch (_) {}
                page = null;
            }
        }
    } catch (e) {
        console.error(`[Scraper] Headless error: ${e.message}`);
        res.status(500).json({ success: false, error: e.message });
    } finally {
        if (page) {
            try { await page.context().close(); } catch (_) {}
        }
    }
});

async function runHeadlessStreamSniffer(page, options) {
    const {
        url,
        referer,
        timeout,
        waitFor,
        waitForNetworkIdle,
        clickSelector,
        evaluateScript,
        watchUrlRegex,
        mediaExtensions,
        debug = false,
        requestCap,
    } = options;

    const regex = watchUrlRegex ? new RegExp(watchUrlRegex, 'i') : null;
    const manifestExts = mediaExtensions.filter(ext => ext !== '.mp4' && ext !== '.mkv' && ext !== '.webm');
    const segmentExts = mediaExtensions.filter(ext => SEGMENT_BLOCKLIST.includes(ext));
    let requestCount = 0;

    const capturePromise = new Promise((resolve, reject) => {
        let settled = false;
        let aborting = false;
        const timer = setTimeout(() => {
            if (settled) return;
            settled = true;
            cleanup();
            reject(new Error('No media request captured before timeout'));
        }, timeout);

        const cleanup = () => {
            clearTimeout(timer);
            page.off('request', onRequest);
            page.off('response', onResponse);
        };

        // Hard stop: cancel in-flight requests and tell the page to stop firing more.
        const hardStop = async () => {
            if (aborting) return;
            aborting = true;
            try { await page.evaluate('window.stop && window.stop()').catch(() => {}); } catch (_) {}
        };

        const tryResolveUrl = (candidateUrl, sourceHeaders = {}) => {
            if (settled) return;
            settled = true;
            cleanup();
            hardStop();
            resolve({
                url: candidateUrl,
                streamType: inferStreamType(candidateUrl),
                referer: sourceHeaders.referer || referer || url,
                requestHeaders: sourceHeaders,
            });
        };

        const isSegmentUrl = (u) => {
            const lower = u.split('?')[0].toLowerCase();
            return segmentExts.some(ext => lower.endsWith(ext));
        };

        const isPageOrigin = (reqUrl) => {
            try {
                const u = new URL(reqUrl);
                return PAGE_ORIGIN_SUFFIXES.some(suffix => u.hostname === suffix || u.hostname.endsWith('.' + suffix));
            } catch (_) {
                return false;
            }
        };

        const onRequest = (request) => {
            const reqUrl = request.url();
            const resourceType = request.resourceType();
            const exemptByType = CAP_EXEMPT_RESOURCE_TYPES.has(resourceType);
            const exemptByOrigin = isPageOrigin(reqUrl);
            const countsAgainstCap = !exemptByType && !exemptByOrigin;
            if (countsAgainstCap) requestCount++;

            if (debug) {
                const tag = exemptByType ? ' (exempt)' : exemptByOrigin ? ' (page-origin)' : '';
                console.log(`[Scraper] Headless request [${resourceType}${tag}] #${requestCount}: ${reqUrl}`);
            }

            // Cost guard: abort the sniffer if too many assets are being requested.
            if (requestCount > requestCap) {
                if (!settled) {
                    settled = true;
                    cleanup();
                    hardStop();
                    reject(new Error(`Asset request cap exceeded (${requestCap})`));
                }
                return;
            }

            // Cost guard: video segment requests are aborted at the route level
            // so the body is never downloaded through the proxy.
            if (isSegmentUrl(reqUrl)) {
                if (debug) console.log(`[Scraper] Headless segment blocked: ${reqUrl}`);
                try { request.abort(); } catch (_) {}
                return;
            }

            const matchesManifest = manifestExts.some(ext => reqUrl.toLowerCase().includes(ext));
            const matchesRegex = regex && regex.test(reqUrl);
            if (!matchesManifest && !matchesRegex) return;

            tryResolveUrl(reqUrl, request.headers());
        };

        const onResponse = async (response) => {
            const reqUrl = response.url();
            const contentType = response.headers()['content-type'] || '';
            const isApiLike =
                reqUrl.includes('.json') ||
                reqUrl.includes('/api/') ||
                reqUrl.includes('/graphql') ||
                contentType.includes('json');
            if (!isApiLike) return;

            try {
                const body = await response.text();
                const found = findMediaUrlInText(body, mediaExtensions, regex);
                if (found && !isSegmentUrl(found)) {
                    if (debug) {
                        console.log(`[Scraper] Headless found media in response: ${reqUrl} -> ${found}`);
                    }
                    tryResolveUrl(found, {});
                }
            } catch (e) {
                // Response body may be binary or unreadable; ignore.
            }
        };

        page.on('request', onRequest);
        page.on('response', onResponse);
    });

    if (referer) {
        await page.setExtraHTTPHeaders({ Referer: referer });
    }

    try {
        await page.goto(url, { waitUntil: 'domcontentloaded', timeout });
    } catch (e) {
        // Goto may reject if the page is aborted by hardStop; let capturePromise
        // decide whether the catch is fatal or the match already resolved.
        if (!(await capturePromiseSafe(capturePromise))) {
            throw e;
        }
    }

    if (clickSelector) {
        try {
            await page.click(clickSelector);
        } catch (e) {
            console.log(`[Scraper] Headless click skipped: ${e.message}`);
        }
    }

    // Some players (Dailymotion, Vimeo) don't request the m3u8 until the
    // user clicks play. Dailymotion in particular gates the m3u8 fetch on a
    // real user-gesture event — the autoplay policy in headless Chromium
    // blocks `video.play()` from triggering network requests, so we have to
    // dispatch synthetic click events. The gesture detector's heuristic
    // varies (some look for `pointerdown`, some for `touchstart`, some
    // require multiple events spaced over time) so we also run a background
    // re-click loop that keeps nudging the overlay every 500ms until the
    // capturePromise resolves.
    if (!clickSelector) {
        try {
            await page.waitForSelector('video, .dmp_PlayButton, [aria-label*="Play" i], .plyr__play', {
                timeout: Math.min(5000, timeout - 2000),
            }).catch(() => {});
        } catch (_) {}
        const autoplayClick = async () => {
            try {
                await page.evaluate(() => {
                    const buttons = [
                        '.dmp_PlayButton', '.np_PlayButton', '.plyr__play', '.play', '.play-button',
                        'button[aria-label*="Play" i]', '[aria-label*="Play" i]', 'button[title*="Play" i]',
                    ];
                    for (const sel of buttons) {
                        const el = document.querySelector(sel);
                        if (!el) continue;
                        const eventInit = { bubbles: true, cancelable: true, button: 0, composed: true };
                        for (const type of ['pointerdown', 'mousedown', 'pointerup', 'mouseup', 'click']) {
                            el.dispatchEvent(new MouseEvent(type, eventInit));
                        }
                        if (typeof el.click === 'function') el.click();
                        break;
                    }
                });
            } catch (_) {}
        };
        await autoplayClick();
        // Keep re-clicking every 500ms until capturePromise resolves or we
        // run out of time. This handles the race where the overlay mounts
        // AFTER our first click attempt.
        const nudgeTimer = setInterval(autoplayClick, 500);
        // Stop nudging once capturePromise settles (or the outer timer fires).
        capturePromise.finally(() => clearInterval(nudgeTimer)).catch(() => {});
    }

    if (evaluateScript) {
        try {
            const evalResult = await page.evaluate(evaluateScript);
            if (evalResult && (evalResult.url || evalResult.streamUrl)) {
                const streamUrl = evalResult.url || evalResult.streamUrl;
                if (!isSegmentUrl_(streamUrl)) {
                    return {
                        url: streamUrl,
                        streamType: inferStreamType(streamUrl),
                        referer: referer || url,
                        evaluateResult: evalResult,
                    };
                }
            }
        } catch (e) {
            console.error(`[Scraper] Headless evaluate error: ${e.message}`);
        }
    }

    if (waitFor) {
        try {
            await page.waitForSelector(waitFor, { timeout: timeout - 5000 });
        } catch (e) {
            console.log(`[Scraper] Headless waitFor skipped: ${e.message}`);
        }
    }

    if (waitForNetworkIdle) {
        try {
            await page.waitForLoadState('networkidle', { timeout: timeout - 5000 });
        } catch (e) {
            console.log(`[Scraper] Headless networkidle skipped: ${e.message}`);
        }
    }

    return await capturePromise;
}

function isSegmentUrl_(u) {
    const lower = (u || '').split('?')[0].toLowerCase();
    return SEGMENT_BLOCKLIST.some(ext => lower.endsWith(ext));
}

async function capturePromiseSafe(p) {
    return await Promise.race([p, Promise.resolve('pending')]).then(v => v !== 'pending');
}

function inferStreamType(url) {
    const lower = url.split('?')[0].toLowerCase();
    if (lower.includes('.m3u8')) return 'hls';
    if (lower.includes('.mpd')) return 'dash';
    if (lower.includes('.mp4') || lower.includes('.mkv') || lower.includes('.webm')) return 'mp4';
    return 'mp4';
}

function findMediaUrlInText(text, mediaExtensions, regex) {
    if (!text || typeof text !== 'string') return null;
    // Prefer HLS over MP4 when both are present.
    for (const ext of ['.m3u8', '.mpd', '.mp4', '.mkv', '.webm']) {
        if (!mediaExtensions.includes(ext)) continue;
        const pattern = new RegExp(`(https?://[^"'\\s<>]+?${ext.replace('.', '\\.')}(?:\\?[^"'\\s<>]*)?)`, 'i');
        const match = text.match(pattern);
        if (match) return match[1];
    }
    if (regex) {
        const match = text.match(regex);
        if (match) return match[0];
    }
    return null;
}

app.listen(port, '127.0.0.1', () => {
    console.log(`Scraper Microservice listening on http://127.0.0.1:${port}`);
});
