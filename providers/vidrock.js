const { getNewPage } = require("../utils/browserManager");
const crypto = require('crypto');

const DOMAIN = 'https://vidrock.net';
const PASSPHRASE = 'x7k9mPqT2rWvY8zA5bC3nF6hJ2lK4mN9';

/**
 * Encrypt item ID using AES-CBC with fixed passphrase
 */
async function encryptItemId(itemId) {
    try {
        const keyData = Buffer.from(PASSPHRASE, 'utf-8');
        const iv = keyData.slice(0, 16); 
        const cipher = crypto.createCipheriv('aes-256-cbc', keyData, iv);
        let encrypted = cipher.update(itemId, 'utf8', 'base64');
        encrypted += cipher.final('base64');
        return encrypted.replace(/\+/g, '-').replace(/\//g, '_').replace(/=/g, '');
    } catch (error) {
        return null;
    }
}

/**
 * VidRock Provider (Browser-based to bypass Cloudflare 403)
 */
async function getStreams(tmdbId, mediaType, season, episode) {
    console.log(`[VidRock] Processing (Browser): TMDB ${tmdbId} (${mediaType})`);
    
    let itemId;
    let itemType;

    if (mediaType === 'tv' || mediaType === 'series') {
        itemId = `${tmdbId}_${season}_${episode}`;
        itemType = 'tv';
    } else {
        itemId = tmdbId.toString();
        itemType = 'movie';
    }

    const encrypted = await encryptItemId(itemId);
    if (!encrypted) return [];

    const targetUrl = `${DOMAIN}/api/${itemType}/${encrypted}`;
    console.log(`[VidRock] Target API URL: ${targetUrl}`);

    let page;
    try {
        page = await getNewPage({ browserType: 'chromium' });
        
        // VidRock returns a JSON object, so we look for the response body
        let responseBody = null;

        page.on('response', async (res) => {
            if (res.url() === targetUrl) {
                try {
                    responseBody = await res.json();
                } catch (e) { /* ignore */ }
            }
        });

        // We navigate to the referrer page first to set cookies/headers
        // Then we can try to fetch the API endpoint via page.evaluate
        const refererUrl = `${DOMAIN}/${mediaType === 'tv' ? 'tv' : 'movie'}/${tmdbId}`;
        await page.goto(refererUrl, { waitUntil: 'domcontentloaded', timeout: 20000 });

        // Now manually fetch the API using the browser context (which has the CF tokens)
        responseBody = await page.evaluate(async (url) => {
            try {
                const r = await fetch(url);
                return await r.json();
            } catch (e) {
                return null;
            }
        }, targetUrl);

        if (!responseBody || Object.keys(responseBody).length === 0) {
            console.log('[VidRock] No sources returned or 403 blocked');
            return [];
        }

        const streams = [];
        Object.values(responseBody).forEach(source => {
            if (source && source.url) {
                // Filter out problematic/incorrect domains
                if (source.url.includes('67streams.online')) {
                    console.log(`[VidRock] Skipping incorrect source: ${source.url}`);
                    return;
                }

                streams.push({
                    title: `VidRock - ${source.language || 'Unknown'} - ${source.url.includes('m3u8') ? 'HLS' : 'MP4'}`,
                    url: source.url,
                    quality: 'Auto',
                    provider: 'VidRock',
                    headers: {
                        'Referer': refererUrl,
                        'Origin': DOMAIN,
                        'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36'
                    }
                });
            }
        });

        return streams;

    } catch (e) {
        console.error(`[VidRock] Browser Error: ${e.message}`);
        return [];
    } finally {
        if (page) await page.context().close();
    }
}

module.exports = { getStreams };