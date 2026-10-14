#!/usr/bin/env node

/**
 * Debug script v3: follow the vidsrc.to iframe chain explicitly.
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
        try { return JSON.parse(text); } catch (_) { return { raw: text }; }
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
            console.log('[Tester] Raw first 300:', text.substring(0, 300));
            return null;
        }
        console.log('[Tester] First 700 chars:\n' + text.substring(0, 700));
        return text;
    } catch (e) {
        console.error(`[Tester] GET failed: ${e.message}`);
        return null;
    }
}

async function waitForMedia(page, timeoutMs = 90000) {
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
            reject(new Error(`No media request captured; candidates: ${JSON.stringify(candidates)}`));
        }, timeoutMs);

        const cleanup = () => {
            clearTimeout(timer);
            page.off('request', onRequest);
            page.off('response', onResponse);
        };

        const maybeResolve = (url, headers = {}) => {
            if (settled || !url || url.includes('{v')) return;
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
            console.log(`[Sniffer] req ${request.resourceType()} ${reqUrl.substring(0, 130)}`);
            maybeResolve(reqUrl, request.headers());
        };

        const onResponse = async (response) => {
            const reqUrl = response.url();
            const ct = response.headers()['content-type'] || '';
            maybeResolve(reqUrl, response.request().headers());
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

async function nudgePlay(page) {
    try {
        await page.evaluate(() => {
            const sels = ['#pl_but', '.play-btn', '.vjs-big-play-button', '.dmp_PlayButton', '.plyr__play', '.play', '.play-button', 'button[aria-label*="Play" i]', '[aria-label*="Play" i]', 'video'];
            for (const sel of sels) {
                const el = document.querySelector(sel);
                if (el) { el.click(); return; }
            }
        });
    } catch (_) {}
}

async function findIframeSrc(page) {
    // Try main frame first
    for (let i = 0; i < 30; i++) {
        await new Promise(r => setTimeout(r, 1000));
        const frames = page.frames();
        for (const frame of frames) {
            const url = frame.url();
            if (url.includes('cloudorchestranova.com') || url.includes('rcp/')) {
                console.log(`[Frame] Found RCP iframe: ${url}`);
                return url;
            }
        }
        // Also evaluate in main frame for iframe src
        try {
            const src = await page.evaluate(() => {
                const iframes = Array.from(document.querySelectorAll('iframe'));
                const target = iframes.find(f => f.src && (f.src.includes('rcp') || f.src.includes('cloudorchestranova')));
                return target ? target.src : null;
            });
            if (src) {
                console.log(`[Frame] iframe src attribute: ${src}`);
                return src;
            }
        } catch (_) {}
    }
    return null;
}

async function sniffChain(url) {
    const ctx = await getNewContext({ browserType: 'chromium' });
    const page = await ctx.newPage();

    try {
        console.log(`\n[Chain] Step 1: ${url}`);
        await page.goto(url, { waitUntil: 'domcontentloaded', timeout: 60000 });

        const rcpUrl = await findIframeSrc(page);
        if (!rcpUrl) {
            console.log('[Chain] No RCP iframe found, staying on original page and waiting for media...');
            const result = await waitForMedia(page, 90000);
            await ctx.close();
            return { step: 1, result };
        }

        // Step 2: navigate directly to the RCP iframe page.
        console.log(`\n[Chain] Step 2: navigate to RCP iframe ${rcpUrl}`);
        const rcpPage = await ctx.newPage();
        const mediaPromise = waitForMedia(rcpPage, 90000);

        // nudge loop
        const nudgeTimer = setInterval(() => nudgePlay(rcpPage), 800);

        try {
            await rcpPage.goto(rcpUrl, { waitUntil: 'domcontentloaded', timeout: 60000 });
        } catch (_) {}

        let result;
        try {
            result = await mediaPromise;
        } finally {
            clearInterval(nudgeTimer);
        }

        await ctx.close();
        return { step: 2, result };
    } catch (e) {
        await ctx.close();
        throw e;
    }
}

async function probeObeliskGenerate(initialUrl) {
    console.log(`\n[Obelisk] GET ${initialUrl.substring(0, 140)}...`);
    const data = await fetchJson(initialUrl, {
        'User-Agent': UA,
        'Referer': 'https://cloudorchestranova.com/',
        'Accept': '*/*',
        'Origin': 'https://cloudorchestranova.com',
    });
    if (!data) {
        console.log('[Obelisk] No response');
        return null;
    }
    console.log('[Obelisk] Response:', JSON.stringify(data).substring(0, 500));
    const token = data.token || data.jwt || data.url;
    if (!token) return null;
    if (typeof token === 'string' && token.includes('.m3u8')) return token;
    try {
        const u = new URL(initialUrl);
        const prefix = u.pathname.replace('/generate.php', '');
        return `${u.origin}${prefix}/master.m3u8?token=${encodeURIComponent(token)}`;
    } catch (_) {
        return null;
    }
}

async function main() {
    console.log(`=== Vidsrc.to Debug for TMDB ${TMDB_ID} (v3) ===`);

    try {
        const { step, result } = await sniffChain(TARGET_URL);
        console.log(`\n[Sniffer] Captured at step ${step}:`, JSON.stringify(result, null, 2));

        const rawHeaders = result.headers || {};
        const headers = {};
        for (const [k, v] of Object.entries(rawHeaders)) {
            if (!k.startsWith(':')) headers[k] = v;
        }
        if (!headers['user-agent']) headers['user-agent'] = UA;
        if (!headers['referer']) headers['referer'] = 'https://cloudorchestranova.com/';

        let streamUrl = result.url;
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
