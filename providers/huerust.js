/**
 * Hue Stream API
 * Professional Streaming Aggregator
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */

const axios = require('axios');

const RUST_URL = process.env.RUST_ENGINE_URL || 'http://127.0.0.1:24609';

async function getStreams(tmdbId, type, season = 1, episode = 1) {
    const mediaType = type === 'movie' ? 'movie' : 'tv';
    const url = `${RUST_URL}/api/streams/${mediaType}/${tmdbId}?season=${season || 1}&episode=${episode || 1}`;
    try {
        const resp = await axios.get(url, { timeout: 40000 });
        const streams = Array.isArray(resp.data?.streams) ? resp.data.streams : [];
        // Mark already-proxied streams so Node.js doesn't double-wrap them
        return streams.map(s => ({
            ...s,
            skipProxy: s.url.includes('h_token='),
        }));
    } catch (e) {
        console.warn(`[HueRust] Engine call failed: ${e.message}`);
        return [];
    }
}

module.exports = { getStreams };
