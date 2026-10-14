#!/usr/bin/env node

/**
 * Improved debug script for vidsrc.to movie 550.
 * Uses Chromium everywhere and aggressively hunts for the obeliskoverture
 * token generator + master.m3u8 chain seen in production traces.
 */

require('dotenv').config();

const { getNewContext } = require('./utils/browserManager');

const TMDB_ID = '550';
const TARGET_URL = `https://vidsrc.to/embed/movie/${TMDB_ID}`;
const UA = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36';

async function fetchJson(url, headers = {}) {
    try {
        const res = await fetch(url, { headers });
        if (!res.ok) return null;
        const ct = res.headers.get('content-type') || '';
        if (ct.includes('application/json')) return await res.json();
        const text = await res.text();
        try { return JSON.parse(text); } catch (_) { return { raw: text };
        }
    } catch (e) {
        console.error(`[Fetch] ${e.message}`);
        return null;
    }
}

async function testStream(url, headers = {}) {
    console.log(`\n[Tester] HEAD ${url.substring(0, 140)}...`);
    try {
        const res = await fetch(url, { method: 'HEAD', headers, redirect: 'follow' });
        console.log(`[Tester] Status: ${res.status}, Content-Type: ${res.headers.get('content-type') || 'n/a'}, Length: ${res.headers.get('content-length') || 'n/a'}`);
        return res.ok;
    } catch (e) {
        console.error(`[Tester] HEAD failed: ${e.message}`);
        return false;
    }
}

async function fetchMasterM3u8(url, headers = {}) {
    console.log(`\n[Tester] GET ${url.substring(0, 140)}...`);
    try {
        const res = await fetch(url, { headers, redirect: 'follow' });
        const text = await res.text();
        console.log(`[Tester] Status ${res.status}, length ${text.length}`);
        if (!text.trim().startsWith('#EXTM3U')) {
            console.warn('[Tester] Not an M3U8');
            return null;
        }
        console.log('[Tester] First 700 chars:\n' + text.substring(0, 700));
        return text;
    } catch (e) {
        console.error(`[Tester] GET failed: ${e.message}`);
        return null;
    }
}

async function waitForMedia(page, timeoutMs = 60000) {
    return new Promise((resolve, reject) => {
        let settled = false;
        let aborting = false;
        const candidates = [];

        const hardStop = async () => {
            if (aborting) return;
            aborting = true;
            try { await page.evaluate('window.stop && window.stop()').catch(() => {}); } catch (_) {}
        };

        const timer = setTimeout(() => {
            if (settled) return;
            settled = true;
            cleanup();
            hardStop();
            reject(new Error(`No media request captured before timeout; candidates: ${JSON.stringify(candidates)}`));
        }, timeoutMs);

        const cleanup = () => {
            clearTimeout(timer);
            page.off('request', onRequest);
            page.off('response', onResponse);
        };

        const maybeResolve = (url, headers = {}) => {
            if (settled) return;
            if (!url || url.includes('{v')) return;
            const lower = url.toLowerCase();
            if (!lower.includes('.m3u8') && !lower.includes('obeliskoverture') && !lower.includes('/generate.php')) return;
            candidates.push(url);
            if (lower.includes('.m3u8')) {
                settled = true;
                cleanup();
                hardStop();
                resolve({ url, headers });
            }
        };

        const onRequest = (request) => {
            const reqUrl = request.url();
            maybeResolve(reqUrl, request.headers());
        };

        const onResponse = async (response) => {
            const reqUrl = response.url();
            const ct = response.headers()['content-type'] || '';
            maybeResolve(reqUrl, response.request().headers());

            // If an API/JSON response contains an M3U8 or obelisk URL, surface it.
            if (ct.includes('json') || reqUrl.includes('/generate.php') || reqUrl.includes('/pl/')) {
                try {
                    const body = await response.text();
                    const matches = body.match(/https?:\/\/[^\s"'<>]+(?:\.m3u8|\/generate\.php|\/pl\/)[^\s"'<>]*/gi);
                    if (matches) matches.forEach(u => maybeResolve(u, {}));
                } catch (_) {}
            }
        };

        page.on('request', onRequest);
        page.on('response', onResponse);
    });
}

async function sniffVidsrcTo(url) {
    const ctx = await getNewContext({ browserType: 'chromium' });
    const page = await ctx.newPage();

    // Block heavy non-essential resources to reduce noise and cost.
    await page.route('**/*', (route) => {
        const type = route.request().resourceType();
        const reqUrl = route.request().url();
        if (type === 'image' || type === 'font' || reqUrl.includes('histats.com') || reqUrl.includes('dtscout.com') || reqUrl.includes('rtmark.net') || reqUrl.includes('luugy.com') || reqUrl.includes('llvpn.com') || reqUrl.includes('cloudflareinsights.com')) {
            return route.abort();
        }
        route.continue();
    });

    // Click loop to trigger lazy players.
    let nudgeHandle = null;
    const startNudge = () => {
        nudgeHandle = setInterval(async () => {
            try {
                await page.evaluate(() => {
                    const sels = ['#pl_but', '.play-btn', '.vjs-big-play-button', '.dmp_PlayButton', '.plyr__play', '.play', '.play-button', 'button[aria-label*="Play" i]', '[aria-label*="Play" i]', 'video'];
                    for (const sel of sels) {
                        const el = document.querySelector(sel);
                        if (el) { el.click(); return; }
                    }
                });
            } catch (_) {}
        }, 800);
    };

    try {
        startNudge();
        const gotoPromise = page.goto(url, { waitUntil: 'domcontentloaded', timeout: 60000 });
        const mediaPromise = waitForMedia(page, 90000);

        // Wait for either navigation or media capture.
        await gotoPromise.catch(() => {});
        const result = await mediaPromise;
        clearInterval(nudgeHandle);
        await ctx.close();
        return result;
    } catch (e) {
        clearInterval(nudgeHandle);
        await ctx.close();
        throw e;
    }
}

async function probeObeliskGenerate(initialUrl) {
    // The trace shows /generate.php returning a JWT, then /pl/<blob>/master.m3u8?token=<jwt>.
    console.log(`\n[Obelisk] Probing generate endpoint ${initialUrl.substring(0, 140)}...`);
    const data = await fetchJson(initialUrl, {
        'User-Agent': UA,
        'Referer': 'https://cloudorchestranova.com/',
        'Accept': '*/*',
        'Origin': 'https://cloudorchestranova.com',
    });
    if (!data) {
        console.log('[Obelisk] No JSON from generate endpoint');
        return null;
    }
    console.log('[Obelisk] Response keys:', Object.keys(data).join(', '));
    const token = data.token || data.jwt || data.url;
    if (!token) return null;

    // If data.url is already a full m3u8, return it.
    if (typeof token === 'string' && token.includes('.m3u8')) {
        return token;
    }

    // Otherwise build the master.m3u8 URL from the initial path pattern.
    try {
        const u = new URL(initialUrl);
        // /generate.php under /pl/<blob>/
        const prefix = u.pathname.replace('/generate.php', '');
        const master = `${u.origin}${prefix}/master.m3u8?token=${encodeURIComponent(token)}`;
        return master;
    } catch (_) {
        return null;
    }
}

async function main() {
    console.log(`=== Vidsrc.to Debug for TMDB ${TMDB_ID} (v2) ===`);

    try {
        const result = await sniffVidsrcTo(TARGET_URL);
        console.log('\n[Sniffer] Captured:', JSON.stringify(result, null, 2));

        const rawHeaders = result.headers || {};
        const headers = {};
        for (const [k, v] of Object.entries(rawHeaders)) {
            if (!k.startsWith(':')) headers[k] = v;
        }
        if (!headers['user-agent']) headers['user-agent'] = UA;
        if (!headers['referer']) headers['referer'] = 'https://cloudorchestranova.com/';

        let streamUrl = result.url;

        // If we only captured generate.php, derive the master m3u8.
        if (streamUrl.includes('/generate.php')) {
            const derived = await probeObeliskGenerate(streamUrl);
            if (derived) {
                console.log('[Obelisk] Derived master URL:', derived);
                streamUrl = derived;
            }
        }

        await testStream(streamUrl, headers);
        const m3u8 = await fetchMasterM3u8(streamUrl, headers);

        if (m3u8) {
            // Pull first variant and HEAD-check it.
            const variantMatch = m3u8.match(/^(https?:\/\/[^\n\r]+\.m3u8[^\n\r]*)$/m);
            if (variantMatch) {
                console.log('\n[Tester] First variant:');
                await testStream(variantMatch[1], headers);
            }
        }
    } catch (e) {
        console.error('[Sniffer] Error:', e.message);
    }

    console.log('\n=== Done ===');
    process.exit(0);
}

main().catch((e) => {
    console.error('Unhandled:', e);
    process.exit(1);
});
