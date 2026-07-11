/**
 * HUE ENGINE ❄
 * System Designation: The Architecture of Aggregation
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */

// PpVTo Stream Extraction Daemon
// Keeps a warm headless Chromium instance and extracts decrypted stream URLs
// from pooembed.eu by running the full iframe + JW Player + WASM decryption flow.
//
// Usage: node daemon.js [--port PORT] [--cache-ttl SECONDS]
// Default port: 9321, default cache TTL: 28800 (8 hours)

const http = require('http');
const { chromium } = require('playwright');

// ─── Config ──────────────────────────────────────────────────────────────────────

const PORT = parseInt(process.env.POE_PORT || process.argv.find(a => a.startsWith('--port='))?.split('=')[1] || '9321');
const CACHE_TTL_MS = parseInt(process.env.POE_CACHE_TTL || '28800000'); // 8h default
const EXTRACT_TIMEOUT_MS = 35000;
const BIND_ADDR = '127.0.0.1';

// ─── State ───────────────────────────────────────────────────────────────────────

const cache = new Map(); // matchPath → { url, ts }
let browser = null;
let healthy = false;

// ─── Browser management ──────────────────────────────────────────────────────────

async function getBrowser() {
    if (browser && browser.isConnected()) return browser;
    browser = await chromium.launch({
        headless: true,
        args: ['--no-sandbox', '--disable-blink-features=AutomationControlled'],
    });
    return browser;
}

// ─── Core: extract decrypted stream URL ───────────────────────────────────────────

async function extractStreamUrl(matchPath) {
    // Check cache
    const cached = cache.get(matchPath);
    if (cached && Date.now() - cached.ts < CACHE_TTL_MS) {
        console.error(`[cache-hit] ${matchPath}`);
        return cached.url;
    }

    const pooUrl = `https://pooembed.eu/embed/${matchPath}`;
    const b = await getBrowser();
    const context = await b.newContext({
        userAgent: 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36',
        viewport: { width: 1920, height: 1080 },
        bypassCSP: true,
        extraHTTPHeaders: {
            'Referer': `https://ppv.to/live/${matchPath}`,
            'Origin': 'https://ppv.to',
        },
    });

    try {
        const page = await context.newPage();

        // Inject capture script into the pooembed HTML page
        await page.route('https://pooembed.eu/**/*', async (route) => {
            const url = route.request().url();
            const resType = route.request().resourceType();

            try {
                const resp = await route.fetch();

                if (resType === 'document' && url.includes('/embed/')) {
                    let html = await resp.text();
                    const inject = `<script>
(function(){
var iv=setInterval(function(){
if(typeof jwplayer!=='undefined'){clearInterval(iv);
var o=jwplayer;jwplayer=function(id){var p=o(id);var s=p.setup;
p.setup=function(c){window.__u=c.file||null;s.call(this,c)};return p}}
},100)})()</script>`;

                    html = html.replace(/<head[^>]*>/i, '$&' + inject);
                    if (!/<head/i.test(html)) {
                        html = html.replace(/<html[^>]*>/i, '$&<head>' + inject + '</head>');
                    }
                    return route.fulfill({ status: resp.status(), headers: resp.headers(), body: html });
                }
                return route.fulfill({ status: resp.status(), headers: resp.headers(), body: await resp.body() });
            } catch(e) {
                return route.continue();
            }
        });

        // Load iframe wrapper
        await page.setContent(`<!DOCTYPE html>
<html><head><meta charset="utf-8"></head>
<body style="margin:0;background:#000;">
<iframe src="${pooUrl}" style="width:1280px;height:720px;border:none;"
        allow="autoplay; fullscreen; encrypted-media" allowfullscreen="true"
        referrerpolicy="no-referrer-when-downgrade"
        sandbox="allow-scripts allow-same-origin allow-forms allow-popups">
</iframe>
</body></html>`);

        // Poll for captured URL
        let streamUrl = null;
        const deadline = Date.now() + EXTRACT_TIMEOUT_MS;
        while (Date.now() < deadline && !streamUrl) {
            await new Promise(r => setTimeout(r, 500));
            const frames = page.frames();
            if (frames.length > 1) {
                try {
                    streamUrl = await frames[1].evaluate(() => window.__u);
                } catch(e) {}
            }
        }

        if (streamUrl) {
            cache.set(matchPath, { url: streamUrl, ts: Date.now() });
            console.error(`[extracted] ${matchPath} → ${streamUrl.substring(0, 80)}...`);
        }

        return streamUrl;
    } finally {
        await context.close();
    }
}

// ─── Health check ─────────────────────────────────────────────────────────────────

async function healthCheck() {
    try {
        const b = await getBrowser();
        healthy = b.isConnected();
    } catch(e) {
        healthy = false;
    }
    return healthy;
}

// ─── HTTP server ──────────────────────────────────────────────────────────────────

const server = http.createServer(async (req, res) => {
    const url = new URL(req.url, `http://${BIND_ADDR}:${PORT}`);
    const path = decodeURIComponent(url.pathname);

    // Health check
    if (path === '/health' || path === '/') {
        const h = await healthCheck();
        res.writeHead(h ? 200 : 503, { 'Content-Type': 'application/json' });
        return res.end(JSON.stringify({ healthy: h, cacheSize: cache.size, uptime: process.uptime() }));
    }

    // Cache stats
    if (path === '/cache') {
        res.writeHead(200, { 'Content-Type': 'application/json' });
        const entries = [...cache.entries()].map(([k, v]) => ({ path: k, age: Date.now() - v.ts, url: v.url.substring(0, 80) }));
        return res.end(JSON.stringify(entries));
    }

    // Clear cache
    if (path === '/cache/clear' && req.method === 'POST') {
        cache.clear();
        res.writeHead(200);
        return res.end('ok');
    }

    // Extract stream URL: GET /:matchPath
    const matchPath = path.slice(1); // remove leading /
    if (!matchPath || matchPath.includes('..')) {
        res.writeHead(400);
        return res.end('invalid path');
    }

    try {
        const streamUrl = await extractStreamUrl(matchPath);
        if (streamUrl) {
            res.writeHead(200, { 'Content-Type': 'text/plain' });
            res.end(streamUrl);
        } else {
            res.writeHead(504, { 'Content-Type': 'text/plain' });
            res.end('extraction timeout');
        }
    } catch(e) {
        console.error(`[error] ${matchPath}: ${e.message}`);
        res.writeHead(500, { 'Content-Type': 'text/plain' });
        res.end(e.message);
    }
});

// ─── Startup ──────────────────────────────────────────────────────────────────────

async function main() {
    console.error(`Starting PpVTo extraction daemon on ${BIND_ADDR}:${PORT}...`);
    await getBrowser();
    healthy = true;
    console.error(`Browser ready, cache TTL: ${CACHE_TTL_MS / 1000}s`);

    server.listen(PORT, BIND_ADDR, () => {
        console.error(`Daemon listening on ${BIND_ADDR}:${PORT}`);
    });

    // Graceful shutdown
    const shutdown = async () => {
        console.error('\nShutting down...');
        server.close();
        if (browser) await browser.close();
        process.exit(0);
    };
    process.on('SIGTERM', shutdown);
    process.on('SIGINT', shutdown);

    // Periodic cache cleanup
    setInterval(() => {
        const now = Date.now();
        for (const [k, v] of cache) {
            if (now - v.ts > CACHE_TTL_MS) cache.delete(k);
        }
    }, 300000); // every 5 min
}

main().catch(e => { console.error(e); process.exit(1); });
