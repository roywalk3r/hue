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
            `https://streamed.pk/watch/${matchName}/echo/1`,
            `https://streamed.pk/watch/${matchName}/alpha/1`,
            `https://streamed.pk/watch/${matchName}/bravo/1`
        ];

        // First try to get live matches to find the match
        try {
            const LivesportAPIScraper = require('../livesport-api-scraper');
            const api = new LivesportAPIScraper();
            // Note: api.getLiveMatches might still be using livesport.su, might need update later
            const liveMatches = await api.getLiveMatches();
            
            // Find matching match by name
            const match = liveMatches.find(m => 
                m.id.toLowerCase().includes(matchName.toLowerCase()) ||
                m.title.toLowerCase().includes(matchName.replace(/-/g, ' ').toLowerCase())
            );

            if (match) {
                // Pick the first available source
                const source = match.sources && match.sources.length > 0 ? match.sources[0].source : 'echo';
                return `https://streamed.pk/watch/${match.id}/${source}/1`;
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
                const m3u8Url = stream.m3u8Url || stream.url;
                
                return {
                    title: stream.title,
                    url: m3u8Url,
                    quality: stream.quality,
                    language: stream.language,
                    source: stream.category,
                    provider: 'livesport',
                    // Headers for the proxy
                    headers: {
                        'Referer': 'https://embedsports.top/',
                        'Origin': 'https://embedsports.top',
                        'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36'
                    },
                    subtitles: [],
                    isLive: true,
                    viewers: stream.viewers,
                    // Add unique identifier for each stream
                    id: `${stream.category}-${index}`,
                    // Use the actual stream URL as watchUrl for reference
                    watchUrl: stream.url,
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
