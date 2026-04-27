/**
 * Hue Stream API
 * Professional Streaming Aggregator
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */

const axios = require('axios');

// Confirmed AES-CBC decoder for VidZee encrypted link format.
// Format (after initial atob in site script): ivBase64:cipherBase64
// ivBase64 -> CryptoJS Base64 parsed to WordArray (IV)
// Key: UTF-8 bytes of "qrincywincyspider" padded with nulls to 32 bytes (AES-256-CBC)
// Cipher: base64 ciphertext, PKCS7 padding, mode CBC.
let cryptoJs; // lazy load to avoid cost if not needed
function decodeVidZeeToken(token, debug) {
    try {
        if (typeof token !== 'string') return null;
        if (/^https?:\/\//i.test(token)) return null; // already a URL
        
        // Normalize base64 first (handle URL-safe base64 as well)
        let normalized = token.replace(/-/g, '+').replace(/_/g, '/');
        normalized = normalized.replace(/[^A-Za-z0-9+/=]/g, '');
        const pad = normalized.length % 4;
        const fixed = pad ? normalized + '='.repeat(4 - pad) : normalized;

        let raw;
        let buffer;
        
        try {
            // First get the buffer from base64
            buffer = Buffer.from(fixed, 'base64');
            
            // Try multiple encoding strategies in order of preference
            try {
                // Try UTF-8 first (standard approach)
                raw = buffer.toString('utf8');
            } catch (e) {
                try {
                    // Fall back to binary encoding if UTF-8 fails
                    raw = buffer.toString('binary');
                    console.log('[VidZee] Fallback to binary encoding for token');
                } catch (e2) {
                    try {
                        // Last resort: latin1 (handles all byte values)
                        raw = buffer.toString('latin1');
                        console.log('[VidZee] Fallback to latin1 encoding for token');
                    } catch (e3) {
                        console.log('[VidZee] Failed to decode base64 with any encoding');
                        return null;
                    }
                }
            }
        } catch (e) {
            console.log('[VidZee] Failed to decode base64:', e.message);
            return null;
        }
        
        // Extra validation and sanitization
        if (!raw) return null;
        
        // Aggressive sanitization of non-printable and control characters
        raw = raw.replace(/[\u0000-\u001F\u007F-\u009F]/g, '');
        
        // Make sure we still have a valid structure after sanitization
        if (!raw.includes(':')) return null;
        const [ivB64, cipherB64] = raw.split(':');
        if (!ivB64 || !cipherB64) return null;
        
        // Normalize inner base64 strings
        const normalizeInner = (s) => {
            let n = s.replace(/-/g, '+').replace(/_/g, '/').replace(/[^A-Za-z0-9+/=]/g, '');
            const p = n.length % 4;
            return p ? n + '='.repeat(4 - p) : n;
        };
        
        const fixedIv = normalizeInner(ivB64);
        const fixedCipher = normalizeInner(cipherB64);

        if (!cryptoJs) cryptoJs = require('crypto-js');
        const iv = cryptoJs.enc.Base64.parse(fixedIv.trim());
        const keyStr = 'qrincywincyspider';
        // Pad key with nulls to 32 bytes
        const keyUtf8 = cryptoJs.enc.Utf8.parse(keyStr.padEnd(32, '\0'));
        // Decrypt with proper error handling for crypto-js malformed UTF-8 errors
        let decrypted;
        try {
            const decryptedWords = cryptoJs.AES.decrypt(fixedCipher.trim(), keyUtf8, {
                iv,
                mode: cryptoJs.mode.CBC,
                padding: cryptoJs.pad.Pkcs7
            });
            
            // First try standard UTF-8 decoding
            try {
                decrypted = decryptedWords.toString(cryptoJs.enc.Utf8);
            } catch (utf8Error) {
                // Fall back to Latin1 if UTF-8 fails
                decrypted = decryptedWords.toString(cryptoJs.enc.Latin1);
                if (debug) console.log('[VidZee] Fell back to Latin1 decoding');
            }
        } catch (cryptoError) {
            if (debug) console.log('[VidZee] Crypto decryption failed:', cryptoError.message);
            return null;
        }
        
        if (!decrypted) return null;
        if (!/^https?:\/\//i.test(decrypted)) {
            if (debug) console.log('[VidZee] decrypted but not a URL', decrypted.slice(0,80));
            return null;
        }
        if (debug) console.log('[VidZee] AES decoded token', { tokenSnippet: token.slice(0, 24)+'...', url: decrypted.slice(0,120) });
        return decrypted.trim();
    } catch (e) {
        // Only log errors if debug is enabled
        if (debug) console.log('[VidZee] AES decode error', e.message);
        return null;
    }
}

// Removed unused parseArgs helper

const getVidZeeStreams = async (tmdbId, mediaType, seasonNum, episodeNum) => {
    if (!tmdbId) {
        console.error('[VidZee] Error: TMDB ID (tmdbId) is required.');
        return [];
    }

    if (!mediaType || (mediaType !== 'movie' && mediaType !== 'tv')) {
        console.error('[VidZee] Error: mediaType is required and must be either "movie" or "tv".');
        return [];
    }

    if (mediaType === 'tv') {
        if (!seasonNum) {
            console.error('[VidZee] Error: Season (seasonNum) is required for TV shows.');
            return [];
        }
        if (!episodeNum) {
            console.error('[VidZee] Error: Episode (episodeNum) is required for TV shows.');
            return [];
        }
    }

    const servers = [1,2,3,4,5,6,7,8,9,10]; // VidZee server identifiers

    const streamPromises = servers.map(async (sr) => {
        let targetApiUrl = `https://player.vidzee.wtf/api/server?id=${tmdbId}&sr=${sr}`;

        if (mediaType === 'tv') {
            targetApiUrl += `&ss=${seasonNum}&ep=${episodeNum}`;
        }

        let finalApiUrl;
        const headers = {
            'Referer': `https://player.vidzee.wtf/embed/movie/${tmdbId}`
        };
        const timeout = 20000; // Increased timeout for better reliability

        finalApiUrl = targetApiUrl;

        console.log(`[VidZee] Fetching from server ${sr}: ${targetApiUrl}`);

        try {
            const response = await axios.get(finalApiUrl, {
                headers: headers,
                timeout: timeout
            });

            const responseData = response.data;

            if (!responseData || typeof responseData !== 'object') {
                console.error(`[VidZee S${sr}] Error: Invalid response data from API.`);
                return [];
            }
            
            if (responseData.tracks) {
                delete responseData.tracks;
            }

            let apiSources = [];
            if (responseData.url && Array.isArray(responseData.url)) {
                apiSources = responseData.url;
            } else if (responseData.link && typeof responseData.link === 'string') {
                apiSources = [responseData];
            }

            if (!apiSources || apiSources.length === 0) {
                console.log(`[VidZee S${sr}] No stream sources found in API response.`);
                return [];
            }

            const streams = apiSources.map(sourceItem => {
                const label = sourceItem.name || sourceItem.type || 'VidZee';
                let quality = String(label).match(/^\d+$/) ? `${label}p` : label;
                if (!/(\d{3,4})p/.test(quality.toLowerCase())) {
                    quality = '720p';
                }
                const language = sourceItem.language || sourceItem.lang;
                let rawLink = sourceItem.link;
                const debug = process.env.VIDZEE_DEBUG === '1';
                const decoded = decodeVidZeeToken(rawLink, debug);
                if (decoded && /^https?:\/\//i.test(decoded)) {
                    if (debug) console.log('[VidZee] decoded link', { beforeSample: rawLink.slice(0,40)+'...', after: decoded.slice(0,80) });
                    rawLink = decoded;
                }
                return {
                    name: `VidZee Server${sr} - ${quality} - ${language}`,
                    title: `VidZee Server${sr} - ${quality} - ${language}`,
                    url: rawLink,
                    quality: quality,
                    provider: "VidZee",
                    headers: { 
                        'Referer': 'https://core.vidzee.wtf/'
                    }
                };
            }).filter(stream => stream.url);

            console.log(`[VidZee S${sr}] Successfully extracted ${streams.length} streams. Qualities: ${streams.map(s=>s.quality).join(', ')}`);
            return streams;

        } catch (error) {
            if (error.response) {
                console.error(`[VidZee S${sr}] Error fetching: ${error.response.status} ${error.response.statusText}`);
            } else if (error.request) {
                console.error(`[VidZee S${sr}] Error fetching: No response received.`);
            } else {
                console.error(`[VidZee S${sr}] Error fetching:`, error.message);
            }
            return [];
        }
    });

    const allStreamsNested = await Promise.all(streamPromises);
    const allStreams = allStreamsNested.flat();

    console.log(`[VidZee] Found a total of ${allStreams.length} streams from servers ${servers.join(', ')}.`);
    return allStreams;
};

module.exports = { getVidZeeStreams }; 