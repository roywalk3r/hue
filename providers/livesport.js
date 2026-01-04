/**
 * Livesport Provider
 * Fetches live sports streams from Livesport.su
 */

const LivesportWatchScraper = require('../livesport-watch-scraper');

class LivesportProvider {
    constructor() {
        this.scraper = new LivesportWatchScraper();
    }

    /**
     * Extract match name from path
     * /sports-stream/sunderland-vs-leeds-united -> sunderland-vs-leeds-united
     */
    extractMatchName(path) {
        // Remove /sports-stream/ and any trailing slashes
        const match = path.match(/\/sports-stream\/(.+?)(?:\/|$)/);
        return match ? match[1].replace(/\/$/, '') : null;
    }

    /**
     * Convert match name to watch URL
     */
    async getWatchUrl(matchName) {
        // Try different URL patterns
        const patterns = [
            `https://livesport.su/watch/${matchName}`,
            `https://livesport.su/watch/${matchName}/admin/1`,
            `https://livesport.su/watch/${matchName}/1`
        ];

        // First try to get live matches to find the match
        try {
            const LivesportAPIScraper = require('../livesport-api-scraper');
            const api = new LivesportAPIScraper();
            const liveMatches = await api.getLiveMatches();
            
            // Find matching match by name
            const match = liveMatches.find(m => 
                m.id.toLowerCase().includes(matchName.toLowerCase()) ||
                m.title.toLowerCase().includes(matchName.replace(/-/g, ' ').toLowerCase())
            );

            if (match) {
                return `https://livesport.su/watch/${match.id}`;
            }
        } catch (e) {
            console.log('[Livesport] API search failed, trying direct URL');
        }

        // Fallback to direct URL patterns
        return patterns[0];
    }

    /**
     * Main fetch function - compatible with provider registry
     */
    async getLivesportStreams(ctx) {
        const { req, matchName } = ctx;
        if (!req) {
            console.log('[Livesport] No request object found');
            return [];
        }

        // Use matchName from context if available, otherwise extract from path
        const finalMatchName = matchName || this.extractMatchName(req.path);
        if (!finalMatchName) {
            console.log('[Livesport] No match name found');
            return [];
        }

        console.log(`[Livesport] Fetching streams for: ${finalMatchName}`);

        try {
            // Get the watch URL
            const watchUrl = await this.getWatchUrl(finalMatchName);
            console.log(`[Livesport] Using URL: ${watchUrl}`);

            // Scrape the streams
            const result = await this.scraper.scrapeWatchUrl(watchUrl);
            
            if (!result || result.error) {
                console.log(`[Livesport] Failed to scrape: ${result?.error || 'Unknown error'}`);
                return [];
            }

            // Convert to standard stream format
            const streams = result.streams.map((stream, index) => {
                // Use the sports proxy server
                const m3u8Url = stream.m3u8Url || stream.url;
                const proxyUrl = m3u8Url ? `http://localhost:6001/sports-proxy?url=${encodeURIComponent(m3u8Url)}` : stream.url;
                
                return {
                    title: stream.title,
                    url: proxyUrl,
                    quality: stream.quality,
                    language: stream.language,
                    source: stream.category,
                    provider: 'livesport',
                    // No headers needed - proxy handles them
                    headers: undefined,
                    subtitles: [],
                    isLive: true,
                    viewers: stream.viewers,
                    // Add unique identifier for each stream
                    id: `${stream.category}-${index}`,
                    // Use the actual stream URL as watchUrl for reference
                    watchUrl: stream.url,
                    // Store original M3U8 URL for reference
                    originalUrl: m3u8Url
                };
            });

            console.log(`[Livesport] Found ${streams.length} streams`);
            return streams;

        } catch (error) {
            console.error('[Livesport] Error:', error.message);
            return [];
        }
    }
}

// Export the function for the registry
async function getLivesportStreams() {
    // This will be called with context from the registry
    const provider = new LivesportProvider();
    return await provider.getLivesportStreams(arguments[0]);
}

module.exports = { getLivesportStreams };
