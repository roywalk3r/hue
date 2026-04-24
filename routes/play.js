const express = require('express');
const router = express.Router();
const { getCachedStreams } = require('../utils/cache');
const { getProvider } = require('../providers/registry');
const { resolveServerUrl } = require('../utils/server');
const { processStreamsForProxy } = require('../proxy/proxyServer');
const { resolveImdbId } = require('../utils/tmdb');
const { applyFilters } = require('../utils/streamFilters');
const { config } = require('../utils/config');

module.exports = function(metrics) {
    /**
     * Smart Play Gateway
     * GET /api/play/:type/:tmdbId?season=1&episode=1
     * 
     * Returns a direct, proxied HLS/MP4 link for immediate playback.
     */
    router.get('/:type/:tmdbId', async (req, res) => {
        const { type, tmdbId } = req.params;
        const season = req.query.season ? parseInt(req.query.season) : 1;
        const episode = req.query.episode ? parseInt(req.query.episode) : 1;

        try {
            // 1. Check Cache First (Fastest)
            let streams = getCachedStreams(type, tmdbId, season, episode);
            
            // 2. If not cached, trigger FAST providers only
            if (!streams || streams.length === 0) {
                const fastProviders = ['vixsrc', 'moviebox', 'autoembed'];
                const imdbId = await resolveImdbId(type === 'movie' ? 'movie' : 'tv', tmdbId);
                
                const fastPromises = fastProviders.map(async name => {
                    const prov = getProvider(name);
                    if (!prov || !prov.enabled) return [];
                    try {
                        return await prov.fetch({ tmdbId, type, season, episode, imdbId, filters: {} });
                    } catch (e) { return []; }
                });

                const results = await Promise.all(fastPromises);
                streams = results.flat();
            }

            if (!streams || streams.length === 0) {
                return res.status(404).json({ success: false, error: 'No streams found' });
            }

            // 3. Filter and Sort
            streams = applyFilters(streams, 'aggregate', config.minQualities, config.excludeCodecs);
            
            // Sort to prefer 1080p and non-CAM
            streams.sort((a, b) => {
                const qA = parseInt(a.quality) || 0;
                const qB = parseInt(b.quality) || 0;
                return qB - qA;
            });

            // 4. Validate the best stream before returning (Pre-flight)
            const serverUrl = resolveServerUrl(req);
            let finalStream = null;

            console.log(`[SmartPlay] Validating ${streams.length} candidate streams...`);

            for (const s of streams) {
                try {
                    const originalUrl = s.url.includes('url=') ? decodeURIComponent(s.url.split('url=')[1].split('&')[0]) : s.url;
                    
                    // Ultra-fast 2s timeout check
                    const response = await require('node-fetch')(originalUrl, { 
                        method: 'HEAD', 
                        headers: s.headers || {},
                        timeout: 2000 
                    });

                    if (response.ok) {
                        const proxied = processStreamsForProxy([s], serverUrl);
                        finalStream = proxied[0];
                        console.log(`[SmartPlay] Validated stream from ${s.provider}`);
                        break; 
                    }
                } catch (e) {
                    console.log(`[SmartPlay] Skipping broken stream from ${s.provider}: ${e.message}`);
                }
            }

            if (!finalStream) {
                // If all validation failed, fallback to the first one anyway
                const proxied = processStreamsForProxy([streams[0]], serverUrl);
                finalStream = proxied[0];
            }

            // 5. Redirect or Return JSON
            if (req.query.redirect === '1') {
                return res.redirect(302, finalStream.url);
            }

            res.json({
                success: true,
                tmdbId,
                title: finalStream.title,
                url: finalStream.url,
                quality: finalStream.quality,
                subtitles: finalStream.subtitles || [],
                headers: finalStream.headers
            });

        } catch (e) {
            res.status(500).json({ success: false, error: e.message });
        }
    });

    return router;
};
