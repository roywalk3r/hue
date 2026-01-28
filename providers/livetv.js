const scraper = require('../services/livetvScraper');

/**
 * LiveTV Provider
 * Compatible with the registry system
 */
async function getLivetvStreams(ctx) {
    const { matchName, req } = ctx;
    
    // Determine query: either from matchName param or query string
    const query = matchName || (req && req.query.q);
    
    if (!query) {
        return [];
    }

    console.log(`[LiveTV] Provider invoked for: ${query}`);

    try {
        // 1. Search for the match
        const matches = await scraper.search(query);
        if (matches.length === 0) {
            console.log(`[LiveTV] No match found for "${query}"`);
            return [];
        }

        const bestMatch = matches[0];
        console.log(`[LiveTV] Found match: ${bestMatch.title}`);

        // 2. Get variants
        const variants = await scraper.getStreamVariants(bestMatch.url);
        if (variants.length === 0) {
             console.log(`[LiveTV] No variants found`);
             return [];
        }

        // 3. Resolve a few variants (max 2 to save time/resources)
        const streams = [];
        const toResolve = variants.slice(0, 2);

        for (const variant of toResolve) {
            const resolved = await scraper.resolveStream(variant.url);
            
            if (resolved && resolved.url) {
                // If we need to proxy, the main API will handle it if we return the URL
                // But if headers are required, we might need the proxy to handle them.
                // The main API's proxyServer.js usually handles generic m3u8 proxying 
                // via /proxy?url=... if enabled.
                
                streams.push({
                    title: `[LiveTV] ${bestMatch.title} - ${variant.language}`,
                    url: resolved.url,
                    headers: resolved.headers,
                    quality: variant.quality,
                    isLive: true,
                    provider: 'livetv',
                    source: 'livetv.sx'
                });
            } else if (resolved && resolved.error) {
                console.log(`[LiveTV] Failed to resolve variant: ${resolved.error}`);
            }
        }

        return streams;

    } catch (e) {
        console.error(`[LiveTV] Provider error: ${e.message}`);
        return [];
    }
}

module.exports = { getLivetvStreams };
