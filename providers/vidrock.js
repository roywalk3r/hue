const axios = require('axios');
const crypto = require('crypto');

const DOMAIN = 'https://vidrock.net';
const PASSPHRASE = 'x7k9mPqT2rWvY8zA5bC3nF6hJ2lK4mN9';

/**
 * Encrypt item ID using AES-CBC with fixed passphrase
 * Matches logic: AES-CBC, IV=first 16 bytes of key, PKCS7 padding
 */
async function encryptItemId(itemId) {
    try {
        // Key is the passphrase
        // In Node.js crypto, we can use the raw buffer
        const keyData = Buffer.from(PASSPHRASE, 'utf-8');
        const iv = keyData.slice(0, 16); // IV is first 16 bytes

        const cipher = crypto.createCipheriv('aes-256-cbc', keyData, iv);
        
        let encrypted = cipher.update(itemId, 'utf8', 'base64');
        encrypted += cipher.final('base64');

        // The original code uses a specific URL-safe replacement
        // base64: + -> -, / -> _, remove padding =
        return encrypted.replace(/\+/g, '-').replace(/\//g, '_').replace(/=/g, '');
    } catch (error) {
        console.error('[VidRock] Encryption error:', error.message);
        throw error;
    }
}

async function getLink(tmdbId, mediaType, season, episode) {
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
    return `${DOMAIN}/api/${itemType}/${encrypted}`;
}

async function getStreams(tmdbId, mediaType, season, episode) {
    console.log(`[VidRock] Processing: TMDB ${tmdbId} (${mediaType})`);
    
    try {
        const link = await getLink(tmdbId, mediaType, season, episode);
        console.log(`[VidRock] API URL: ${link}`);

        const headers = {
            'Accept': 'application/json, text/plain, */*',
            'Referer': `${DOMAIN}/movie/${tmdbId}`,
            'Origin': DOMAIN,
            'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36'
        };

        const response = await axios.get(link, { headers, timeout: 10000 });
        const sources = response.data;

        if (!sources || Object.keys(sources).length === 0) {
            console.log('[VidRock] No sources returned');
            return [];
        }

        const streams = [];
        Object.values(sources).forEach(source => {
            if (source && source.url) {
                streams.push({
                    title: `VidRock - ${source.language || 'Unknown'} - ${source.url.includes('m3u8') ? 'HLS' : 'MP4'}`,
                    url: source.url,
                    quality: 'Auto', // VidRock usually provides auto/master playlists
                    provider: 'VidRock',
                    headers: {
                        'Referer': `${DOMAIN}/movie/${tmdbId}`,
                        'Origin': DOMAIN,
                        'User-Agent': headers['User-Agent']
                    }
                });
            }
        });

        return streams;

    } catch (e) {
        console.error(`[VidRock] Error: ${e.message}`);
        if (e.response && e.response.status === 404) {
             console.log('[VidRock] Media not found (404)');
        }
        return [];
    }
}

module.exports = { getStreams };
