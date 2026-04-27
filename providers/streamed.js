/**
 * Hue Stream API
 * Professional Streaming Aggregator
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */

/**
 * Streamed.pk Provider
 * Dedicated provider for streamed.pk sports aggregator
 */

const axios = require('axios');
const { resolveWithPlaywright } = require('../utils/playwrightResolver');

class StreamedProvider {
    constructor() {
        this.baseUrl = 'https://streamed.pk';
        this.apiBase = 'https://streamed.pk/api';
        this.mobileUA = 'Mozilla/5.0 (iPhone; CPU iPhone OS 18_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.5 Mobile/15E148 Safari/604.1';
        this.headers = {
            'User-Agent': this.mobileUA,
            'Referer': 'https://streamed.pk/',
            'Origin': 'https://streamed.pk'
        };
    }

    /**
     * Extract match name from path or query
     */
    extractMatchName(ctx) {
        const { matchName, id, req } = ctx;
        const input = matchName || id || '';
        
        if (input.includes('streamed.pk/watch/')) {
            const m = input.match(/watch\/([^/?#]+)/);
            if (m) return m[1];
        }
        
        if (input) return input;

        if (req && req.path) {
            const match = req.path.match(/\/sports-stream\/(.+?)(?:\/|$)/);
            if (match) return match[1].replace(/\/$/, '');
        }
        return null;
    }

    /**
     * Get all matches across all categories
     */
    async getMatches() {
        try {
            // Get categories first
            const sportsRes = await axios.get(`${this.apiBase}/sports`, { headers: this.headers, timeout: 5000 });
            const categories = Array.isArray(sportsRes.data) ? sportsRes.data : [];
            
            const matchPromises = categories.map(async (cat) => {
                try {
                    const res = await axios.get(`${this.apiBase}/matches/${cat.id}`, { headers: this.headers, timeout: 5000 });
                    return Array.isArray(res.data) ? res.data : [];
                } catch (e) {
                    return [];
                }
            });

            // Also check popular
            const popularPromise = axios.get(`${this.apiBase}/matches/live/popular`, { headers: this.headers, timeout: 5000 })
                .then(res => Array.isArray(res.data) ? res.data : [])
                .catch(() => []);

            const allResults = await Promise.all([...matchPromises, popularPromise]);
            const flatMatches = allResults.flat();
            
            // Deduplicate by ID
            const unique = new Map();
            flatMatches.forEach(m => {
                if (m && m.id) unique.set(m.id, m);
            });
            
            return Array.from(unique.values());
        } catch (error) {
            console.error('[Streamed] Failed to fetch matches:', error.message);
            return [];
        }
    }

    /**
     * Get streams for a specific match
     */
    async getStreams(ctx) {
        const query = this.extractMatchName(ctx);
        if (!query) return [];

        console.log(`[Streamed] Searching for: ${query}`);
        
        const matches = await this.getMatches();
        const cleanQuery = query.toLowerCase().replace(/-/g, ' ');
        
        const bestMatch = matches.find(m => {
            const title = (m.title || '').toLowerCase();
            return title.includes(cleanQuery) || cleanQuery.includes(title) || m.id.includes(query);
        });

        if (!bestMatch) {
            console.log(`[Streamed] No match found for "${query}"`);
            return [];
        }

        console.log(`[Streamed] Found match: ${bestMatch.title} (ID: ${bestMatch.id})`);
        
        // Fetch streams from each source
        const streams = [];
        const sources = bestMatch.sources || [];
        
        for (const src of sources) {
            try {
                const url = `${this.apiBase}/stream/${src.source}/${src.id}`;
                const res = await axios.get(url, { headers: this.headers, timeout: 5000 });
                if (Array.isArray(res.data)) {
                    for (const s of res.data) {
                        if (s.embedUrl) {
                            streams.push({
                                title: `${bestMatch.title} - ${s.language} (Stream ${s.streamNo})`,
                                embedUrl: s.embedUrl,
                                quality: s.hd ? 'HD' : 'SD',
                                language: s.language,
                                viewers: s.viewers,
                                source: src.source
                            });
                        }
                    }
                }
            } catch (e) {
                console.error(`[Streamed] Source fetch failed for ${src.id}:`, e.message);
            }
        }

        if (streams.length === 0) return [];

        // Now resolve the embedUrls using Playwright
        const resolvedStreams = [];
        // Resolve first 2 in parallel for better speed/stability
        const limit = streams.slice(0, 2);

        const resolutionPromises = limit.map(async (s) => {
            try {
                console.log(`[Streamed] Resolving embed: ${s.embedUrl}`);
                // Use a tighter timeout for individual embed resolution
                const resolved = await resolveWithPlaywright(s.embedUrl, 'https://streamed.pk/');
                
                if (resolved && resolved.streams && resolved.streams.length > 0) {
                    return resolved.streams.map((rs, idx) => ({
                        title: s.title + (resolved.streams.length > 1 ? ` (Alt ${idx + 1})` : ''),
                        url: rs.url,
                        quality: rs.quality || s.quality,
                        provider: 'Field',
                        source: 'streamed',
                        isLive: true,
                        headers: rs.headers,
                        originalUrl: rs.url,
                        proxyMode: 'url',
                        isProxied: true
                    }));
                }
            } catch (e) {
                console.error(`[Streamed] Resolution failed for ${s.embedUrl}:`, e.message);
            }
            return null;
        });

        const results = await Promise.all(resolutionPromises);
        results.forEach(r => {
            if (r) resolvedStreams.push(...r);
        });

        return resolvedStreams;
    }
}

async function getStreamedStreams(ctx) {
    const provider = new StreamedProvider();
    return await provider.getStreams(ctx);
}

module.exports = { getStreamedStreams };
