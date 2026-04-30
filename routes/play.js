/**
 * Hue Stream API
 * Professional Streaming Aggregator
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */

const express = require('express');
const router = express.Router();
const { getCachedStreams } = require('../utils/cache');
const { getProvider } = require('../providers/registry');
const { resolveServerUrl } = require('../utils/server');
const { processStreamsForProxy } = require('../proxy/proxyServer');
const { resolveImdbId } = require('../utils/tmdb');
const { applyFilters } = require('../utils/streamFilters');
const { config } = require('../utils/config');

const { getHiTvDetail } = require('../utils/hitv-api');

module.exports = function(metrics) {
    /**
     * Smart Play Gateway
     * GET /api/play/:type/:tmdbId?season=1&episode=1
     */
    router.get('/:type/:tmdbId', async (req, res) => {
        let { type, tmdbId } = req.params;
        const season = req.query.season ? parseInt(req.query.season) : 1;
        const episode = req.query.episode ? parseInt(req.query.episode) : 1;

        try {
            // --- 0. SMART REVERSE MAPPING ---
            // If the ID looks like a HiTV/MovieBox internal ID (> 10 digits)
            if (/^\d{15,}$/.test(tmdbId)) {
                console.log(`[SmartPlay] Internal ID detected: ${tmdbId}. Resolving to TMDB...`);
                try {
                    const detail = await getHiTvDetail(tmdbId);
                    if (detail && detail.title) {
                        const cleanTitle = detail.title
                            .replace(/\s*S\d+(-S\d+)?/gi, '') // Remove S1, S1-S2
                            .replace(/\s*Season\s*\d+/gi, '')
                            .split(' (Ep')[0].split(' - ')[0].trim();

                        console.log(`[SmartPlay] Resolved Title: "${cleanTitle}". Searching TMDB...`);
                        const tmdbResults = await resolveImdbId(type === 'movie' ? 'movie' : 'tv', cleanTitle, true); // Overloaded helper

                        // We'll use search here instead of resolveImdbId since we have title, not ID
                        const results = await require('../utils/tmdb').search(cleanTitle, type === 'movie' ? 'movie' : 'tv');
                        if (results && results.length > 0) {
                            console.log(`[SmartPlay] Reverse Mapping SUCCESS: ${tmdbId} -> ${results[0].id} (${results[0].name || results[0].title})`);
                            tmdbId = results[0].id.toString();
                        }
                    }
                } catch (mapErr) {
                    console.error(`[SmartPlay] Reverse mapping failed:`, mapErr.message);
                }
            }

            // 1. Check Cache First (Fastest)
            let streams = await getCachedStreams(type, tmdbId, season, episode);

            // 2. Try Rust engine first via huerust provider
            if (!streams || streams.length === 0) {
                const huerust = getProvider('huerust');
                if (huerust && huerust.enabled) {
                    try {
                        console.log(`[SmartPlay] Delegating to Rust engine via huerust...`);
                        const rustStreams = await huerust.fetch({ tmdbId, type, season, episode, filters: {} });
                        if (Array.isArray(rustStreams) && rustStreams.length > 0) {
                            streams = rustStreams;
                            console.log(`[SmartPlay] Rust engine returned ${streams.length} streams`);
                        }
                    } catch (rustErr) {
                        console.warn(`[SmartPlay] Rust engine failed:`, rustErr.message);
                    }
                }
            }

            // 3. Fallback to individual Node.js providers if Rust returned nothing
            if (!streams || streams.length === 0) {
                const isDramaId = /^\d{15,}$/.test(tmdbId) || req.query.dp?.includes('drama');

                // Priority providers based on content type
                const fastProviders = isDramaId
                    ? ['moviebox', 'vixsrc', 'autoembed'] // Fast generic fallback
                    : ['vixsrc', 'moviebox', 'autoembed'];

                // Add specialized Drama providers if it's a drama or from HiTV
                const specializedProviders = isDramaId ? ['kisskh', 'dramacool'] : [];

                const imdbId = await resolveImdbId(type === 'movie' ? 'movie' : 'tv', tmdbId);

                // Combine and fetch
                const allToFetch = [...specializedProviders, ...fastProviders];
                const fetchPromises = [...new Set(allToFetch)].map(async name => {
                    const prov = getProvider(name);
                    if (!prov || !prov.enabled) return [];
                    try {
                        console.log(`[SmartPlay] Triggering specialized fetch: ${name}`);
                        return await prov.fetch({ tmdbId, type, season, episode, imdbId, filters: {} });
                    } catch (e) { return []; }
                });

                const results = await Promise.all(fetchPromises);
                streams = results.flat();
            }

            if (!streams || streams.length === 0) {
                return res.status(404).json({ success: false, error: 'No streams found' });
            }

            // 4. Filter and Sort
            streams = applyFilters(streams, 'aggregate', config.minQualities, config.excludeCodecs);

            // Sort to prefer 1080p and non-CAM
            streams.sort((a, b) => {
                const qA = parseInt(a.quality) || 0;
                const qB = parseInt(b.quality) || 0;
                return qB - qA;
            });

            // 5. Validate the best stream before returning (Pre-flight)
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
                        console.log(`[SmartPlay] Stream from ${s.provider} has ${s.subtitles?.length || 0} subtitles before proxy`);
                        const proxied = processStreamsForProxy([s], serverUrl);
                        finalStream = proxied[0];
                        console.log(`[SmartPlay] Validated stream from ${s.provider}, ${finalStream.subtitles?.length || 0} subtitles after proxy`);
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

            // 6. Redirect or Return JSON
            if (req.query.redirect === '1') {
                return res.redirect(302, finalStream.url);
            }

            res.json({
                success: true,
                tmdbId, // This is now the resolved numeric ID
                realTmdbId: tmdbId, // Explicit field for confirmation
                title: finalStream.title,
                url: finalStream.url,
                quality: finalStream.quality,
                provider: finalStream.provider,
                format: finalStream.format,
                subtitles: finalStream.subtitles || [],
                headers: finalStream.headers
            });

        } catch (e) {
            res.status(500).json({ success: false, error: e.message });
        }
    });

    return router;
};
