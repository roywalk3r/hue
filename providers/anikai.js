const CryptoJS = require('crypto-js');
const fetch = require('node-fetch');
const browserManager = require('../utils/browserManager');

/**
 * Anikai Provider (HiAnime Alternative)
 * Optimized for 16GB VPS Speed.
 */

const HARDCODED_MEGACLOUD_KEY = '3709ad8892f413166b796a10c7fb86018bd1be1c7ae6f4d2cfc3fdc299cb3205';
const KEY_URL = 'https://raw.githubusercontent.com/itzzzme/megacloud-keys/refs/heads/main/key.txt';

async function getRemoteKey() {
    try {
        const res = await fetch(KEY_URL);
        const data = await res.text();
        return data.trim();
    } catch (err) {
        return HARDCODED_MEGACLOUD_KEY;
    }
}

async function getAnimeStream(slug, episode) {
    const url = `https://anikai.to/watch/${slug}?ep=${episode}`;
    console.log(`[Anikai] Turbo: ${slug} ep ${episode}`);
    
    let page;
    try {
        // Reuse context if possible, but keep stealth active
        const context = await browserManager.getNewContext();
        page = await context.newPage();
        
        let targetSources = null;

        // Fastest possible way to intercept: listen to ALL traffic
        context.on('response', async res => {
            if (res.url().includes('getSources')) {
                try {
                    const data = await res.json();
                    if (data.sources) targetSources = data;
                } catch(e) {}
            }
        });

        // 1. Navigate directly to watch page with episode query
        // This is 2x faster than clicking UI
        console.log('[Anikai] Fast Nav...');
        await page.goto(url, { waitUntil: 'domcontentloaded', timeout: 30000 });

        // 2. Click the first server IMMEDIATELY when it appears
        console.log('[Anikai] Triggering Player...');
        await page.waitForSelector('.server', { timeout: 15000 });
        await page.evaluate(() => {
            const s = document.querySelector('.server');
            if (s) s.click();
        });

        // 3. Fast Wait for interception
        for (let i = 0; i < 20; i++) {
            if (targetSources) break;
            await page.waitForTimeout(500);
        }

        if (targetSources) {
            console.log('[Anikai] Intercepted!');
            const key = await getRemoteKey();
            const encrypted = targetSources.sources;
            
            if (typeof encrypted === 'string') {
                const decrypted = CryptoJS.AES.decrypt(encrypted, key).toString(CryptoJS.enc.Utf8);
                const finalSources = JSON.parse(decrypted);
                return {
                    success: true,
                    sources: finalSources,
                    subtitles: targetSources.tracks || [],
                    provider: 'Anikai-Turbo'
                };
            }
        }
        
        // Final fallback: Return any iframe
        const iframe = await page.evaluate(() => document.querySelector('iframe')?.src);
        return iframe ? { success: true, iframe, provider: 'Anikai-Iframe' } : { success: false };

    } catch (err) {
        console.error(`[Anikai] Turbo Failed: ${err.message}`);
        return { success: false, error: err.message };
    } finally {
        if (page) await page.close();
    }
}

module.exports = { getAnimeStream };
