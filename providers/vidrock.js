/**
 * Hue Stream API
 * Professional Streaming Aggregator
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */

const { getNewPage } = require("../utils/browserManager");
const crypto = require('crypto');
const axios = require('axios');

const DOMAIN = 'https://vidrock.net';
const PASSPHRASE = 'x7k9mPqT2rWvY8zA5bC3nF6hJ2lK4mN9';

/**
 * Fetch subtitles from VidRock's subtitle API (sub.vdrk.site)
 */
async function fetchSubtitles(type, tmdbId, season, episode) {
    try {
        // Construct subtitle API URL based on media type - using the correct sub.vdrk.site endpoint
        const subtitleUrl = type === 'tv'
            ? `https://sub.vdrk.site/v2/tv/${tmdbId}/${season}/${episode}`
            : `https://sub.vdrk.site/v2/movie/${tmdbId}`;

        console.log(`[VidRock] Fetching subtitles from: ${subtitleUrl}`);

        const response = await axios.get(subtitleUrl, {
            headers: {
                'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36',
                'Referer': `${DOMAIN}/${type === 'tv' ? 'tv' : 'movie'}/${tmdbId}`,
            },
            timeout: 10000,
        });

        if (response.data && Array.isArray(response.data)) {
            return response.data.map(sub => ({
                url: sub.url || sub.file,
                label: sub.label || sub.language || 'Subtitle',
                lang: sub.lang || sub.language || 'en',
            })).filter(s => s.url);
        }

        // Handle object response format
        if (response.data && typeof response.data === 'object') {
            const subtitles = [];
            Object.keys(response.data).forEach(key => {
                const sub = response.data[key];
                if (sub && sub.url) {
                    subtitles.push({
                        url: sub.url,
                        label: sub.label || sub.language || key,
                        lang: sub.language || sub.lang || 'en',
                    });
                }
            });
            if (subtitles.length > 0) {
                return subtitles;
            }
        }

        return [];
    } catch (e) {
        console.log(`[VidRock] Subtitle fetch failed: ${e.message}`);
        return [];
    }
}

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
        const refererUrl = `${DOMAIN}/${mediaType === 'tv' ? 'tv' : 'movie'}/${tmdbId}`;
        await page.goto(refererUrl, { waitUntil: 'commit', timeout: 15000 });
        
        // Wait a few seconds for cookies/tokens to settle OR for responseBody to be captured via listener
        for (let i = 0; i < 10; i++) {
            if (responseBody) break;
            await new Promise(r => setTimeout(r, 500));
        }

        // If not captured via listener, try manual fetch
        if (!responseBody) {
            console.log('[VidRock] Falling back to manual evaluate fetch...');
            responseBody = await page.evaluate(async (url) => {
                try {
                    const res = await fetch(url);
                    if (!res.ok) return null;
                    return await res.json();
                } catch (e) {
                    return null;
                }
            }, targetUrl);
        }

        if (!responseBody || Object.keys(responseBody).length === 0) {
            console.log('[VidRock] No sources returned or 403 blocked');
            return [];
        }

        const streams = [];
        let subtitles = [];

        console.log(`[VidRock] Starting subtitle extraction, responseBody keys: ${Object.keys(responseBody).join(', ')}`);
        console.log(`[VidRock] responseBody.tracks: ${responseBody.tracks ? 'exists' : 'missing'}, responseBody.subtitles: ${responseBody.subtitles ? 'exists' : 'missing'}`);

        // Extract subtitles if present in response
        if (responseBody.tracks || responseBody.subtitles) {
            const tracks = responseBody.tracks || responseBody.subtitles;
            if (Array.isArray(tracks)) {
                subtitles = tracks
                    .filter(t => t.file && (t.kind === 'captions' || t.kind === 'subtitles'))
                    .map(t => ({
                        url: t.file,
                        label: t.label || t.language || 'Subtitle',
                        lang: t.language || 'en'
                    }));
                console.log(`[VidRock] Found ${subtitles.length} subtitle tracks in API response`);
            } else {
                console.log(`[VidRock] tracks/subtitles is not an array, type: ${typeof tracks}`);
            }
        } else {
            console.log(`[VidRock] No tracks/subtitles in responseBody, will fetch from API`);
        }

        // If no subtitles in response, try fetching from subtitle API
        console.log(`[VidRock] Current subtitles count: ${subtitles.length}`);
        if (subtitles.length === 0) {
            const subtitleType = mediaType === 'tv' || mediaType === 'series' ? 'tv' : 'movie';
            console.log(`[VidRock] Fetching subtitles for ${subtitleType} ${tmdbId}, season=${season}, episode=${episode}`);
            try {
                subtitles = await fetchSubtitles(subtitleType, tmdbId, season, episode);
                if (subtitles.length > 0) {
                    console.log(`[VidRock] Found ${subtitles.length} subtitles from subtitle API`);
                } else {
                    console.log(`[VidRock] Subtitle API returned empty result`);
                }
            } catch (subErr) {
                console.error(`[VidRock] Subtitle fetch failed:`, subErr.message);
            }
        } else {
            console.log(`[VidRock] Skipping API fetch, already have ${subtitles.length} subtitles`);
        }

        Object.values(responseBody).forEach(source => {
            if (source && source.url) {
                // Filter out problematic/incorrect domains
                if (source.url.includes('67streams.online')) {
                    console.log(`[VidRock] Skipping incorrect source: ${source.url}`);
                    return;
                }

                const isHls = source.url.includes('m3u8');
                streams.push({
                    title: `VidRock - ${source.language || 'Unknown'} - ${isHls ? 'HLS' : 'MP4'}`,
                    url: source.url,
                    quality: 'Auto',
                    provider: 'VidRock',
                    format: isHls ? 'hls' : 'mp4',
                    headers: {
                        'Referer': refererUrl,
                        'Origin': DOMAIN,
                        'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36'
                    },
                    subtitles: subtitles.length > 0 ? subtitles : undefined
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

module.exports = { getStreams, fetchSubtitles };