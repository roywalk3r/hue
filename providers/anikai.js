/**
 * Hue Stream API
 * Professional Streaming Aggregator
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */

const CryptoJS = require('crypto-js');
const fetch = require('node-fetch');
const browserManager = require('../utils/browserManager');

/**
 * Anikai Provider (Advanced 2026 Edition)
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
        const context = await browserManager.getNewContext();
        page = await context.newPage();
        let targetSources = null;

        context.on('response', async res => {
            if (res.url().includes('getSources')) {
                try {
                    const data = await res.json();
                    if (data.sources) targetSources = data;
                } catch(e) {}
            }
        });

        await page.goto(url, { waitUntil: 'domcontentloaded', timeout: 30000 });
        await page.waitForSelector('.server', { timeout: 15000 });
        await page.evaluate(() => {
            const s = document.querySelector('.server');
            if (s) s.click();
        });

        for (let i = 0; i < 20; i++) {
            if (targetSources) break;
            await page.waitForTimeout(500);
        }

        if (targetSources) {
            const key = await getRemoteKey();
            const encrypted = targetSources.sources;
            
            if (typeof encrypted === 'string') {
                const decrypted = CryptoJS.AES.decrypt(encrypted, key).toString(CryptoJS.enc.Utf8);
                return {
                    success: true,
                    sources: JSON.parse(decrypted),
                    tracks: targetSources.tracks || [],
                    provider: 'Anikai-Turbo'
                };
            }
        }
        
        const iframe = await page.evaluate(() => document.querySelector('iframe')?.src);
        return iframe ? { success: true, iframe, provider: 'Anikai-Iframe' } : { success: false };

    } catch (err) {
        return { success: false, error: err.message };
    } finally {
        if (page) await page.close();
    }
}

module.exports = { getAnimeStream };
