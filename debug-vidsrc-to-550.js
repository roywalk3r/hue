#!/usr/bin/env node

/**
 * Debug script: extract and test a working stream for vidsrc.to movie 550.
 */

require('dotenv').config();

const { getNewContext } = require('./utils/browserManager');
const vidsrcEmbedProvider = require('./providers/vidsrc-embed');
const vidsrcXyzProvider = require('./providers/vidsrc-xyz');

const TMDB_ID = '550';
const TARGET_URL = `https://vidsrc.to/embed/movie/${TMDB_ID}`;

async function testStream(url, headers = {}) {
    console.log(`\n[Tester] HEAD ${url.substring(0, 120)}...`);
    try {
        const res = await fetch(url, {
            method: 'HEAD',
            headers,
            redirect: 'follow',
        });
        console.log(`[Tester] Status: ${res.status}, Content-Type: ${res.headers.get('content-type') || 'n/a'}, Length: ${res.headers.get('content-length') || 'n/a'}`);
        return res.ok;
    } catch (e) {
        console.error(`[Tester] HEAD failed: ${e.message}`);
        return false;
    }
}

async function fetchMasterM3u8(url, headers = {}) {
    console.log(`\n[Tester] GET master playlist ${url.substring(0, 120)}...`);
    try {
        const res = await fetch(url, { headers });
        const text = await res.text();
        console.log(`[Tester] Status ${res.status}, length ${text.length}`);
        if (!text.trim().startsWith('#EXTM3U')) {
            console.warn('[Tester] Response does not look like an M3U8');
            return null;
        }
        console.log('[Tester] First 600 chars:\n' + text.substring(0, 600));
        return text;
    } catch (e) {
        console.error(`[Tester] GET failed: ${e.message}`);
        return null;
    }
}

async function sniffVidsrcTo(url) {
    console.log(`\n[Sniffer] Navigating ${url}`);
    const ctx = await getNewContext({ browserType: 'chromium' });
    const page = await ctx.newPage();

    const result = await new Promise((resolve, reject) => {
        let settled = false;
        const timer = setTimeout(() => {
            if (settled) return;
            settled = true;
            cleanup();
            reject(new Error('No media request captured before timeout'));
        }, 45000);

        const cleanup = () => {
            clearTimeout(timer);
            page.off('request', onRequest);
            page.off('response', onResponse);
        };

        const onRequest = (request) => {
            const reqUrl = request.url();
            const resourceType = request.resourceType();
            console.log(`[Sniffer] req [${resourceType}] ${reqUrl.substring(0, 140)}`);

            if (reqUrl.toLowerCase().includes('.m3u8') || reqUrl.includes('obeliskoverture')) {
                if (!settled) {
                    settled = true;
                    cleanup();
                    resolve({
                        url: reqUrl,
                        headers: request.headers(),
                        streamType: 'hls',
                    });
                }
            }
        };

        const onResponse = async (response) => {
            const reqUrl = response.url();
            const ct = response.headers()['content-type'] || '';
            if ((reqUrl.toLowerCase().includes('.m3u8') || ct.includes('mpegurl') || reqUrl.includes('obeliskoverture')) && !settled) {
                try {
                    const text = await response.text();
                    if (text.trim().startsWith('#EXTM3U')) {
                        settled = true;
                        cleanup();
                        resolve({
                            url: reqUrl,
                            headers: response.request().headers(),
                            streamType: 'hls',
                            bodyPreview: text.substring(0, 500),
                        });
                    }
                } catch (_) {}
            }
        };

        page.on('request', onRequest);
        page.on('response', onResponse);

        page.goto(url, { waitUntil: 'domcontentloaded', timeout: 45000 }).catch(() => {});

        // Click any play overlay after a short delay
        setTimeout(async () => {
            try {
                await page.evaluate(() => {
                    const buttons = [
                        '#pl_but', '.play-btn', '.vjs-big-play-button',
                        '.dmp_PlayButton', '.plyr__play', '.play', '.play-button',
                        'button[aria-label*="Play" i]', '[aria-label*="Play" i]',
                    ];
                    for (const sel of buttons) {
                        const el = document.querySelector(sel);
                        if (el) {
                            el.click();
                            return;
                        }
                    }
                    const video = document.querySelector('video');
                    if (video) video.click();
                });
            } catch (_) {}
        }, 3000);
    });

    await ctx.close();
    return result;
}

async function main() {
    console.log(`=== Vidsrc.to Debug for TMDB ${TMDB_ID} ===`);

    // 1. Try Nebula provider (vidsrc-embed.ru)
    console.log('\n--- Nebula (vidsrc-embed.ru) ---');
    try {
        const nebulaStreams = await vidsrcEmbedProvider.getStreams(TMDB_ID, 'movie', null, null);
        console.log(`[Nebula] Got ${nebulaStreams.length} stream(s)`);
        if (nebulaStreams.length > 0) {
            console.log(JSON.stringify(nebulaStreams[0], null, 2));
            await testStream(nebulaStreams[0].url, nebulaStreams[0].headers || {});
            await fetchMasterM3u8(nebulaStreams[0].url, nebulaStreams[0].headers || {});
        }
    } catch (e) {
        console.error('[Nebula] Error:', e.message);
    }

    // 2. Try direct vidsrc.xyz extraction
    console.log('\n--- Solar (vidsrc.xyz) ---');
    try {
        const solarStreams = await vidsrcXyzProvider.getStreams(TMDB_ID, 'movie', null, null);
        console.log(`[Solar] Got ${solarStreams.length} server result(s)`);
        if (solarStreams.length > 0 && solarStreams[0].streams) {
            console.log(JSON.stringify(solarStreams[0].streams[0], null, 2));
            await testStream(solarStreams[0].streams[0].url, { Referer: solarStreams[0].referer });
            await fetchMasterM3u8(solarStreams[0].streams[0].url, { Referer: solarStreams[0].referer });
        }
    } catch (e) {
        console.error('[Solar] Error:', e.message);
    }

    // 3. Headless sniff vidsrc.to directly
    console.log('\n--- Vidsrc.to headless sniff ---');
    try {
        const sniffed = await sniffVidsrcTo(TARGET_URL);
        console.log('[Sniffer] Captured:', JSON.stringify(sniffed, null, 2));

        const cleanHeaders = {};
        for (const [k, v] of Object.entries(sniffed.headers || {})) {
            if (!k.startsWith(':')) cleanHeaders[k] = v;
        }
        if (!cleanHeaders['user-agent']) cleanHeaders['user-agent'] = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36';
        if (!cleanHeaders['referer']) cleanHeaders['referer'] = 'https://vidsrc.to/';

        await testStream(sniffed.url, cleanHeaders);
        await fetchMasterM3u8(sniffed.url, cleanHeaders);
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
