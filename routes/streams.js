const express = require('express');
const router = express.Router();
const { config } = require('../utils/config');
const { resolveServerUrl } = require('../utils/server');
const { processStreamsForProxy } = require('../proxy/proxyServer');
const { getCachedStreams, addCachedStreams } = require('../utils/cache');
const { listProviders, getProvider } = require('../providers/registry');
const { applyFilters } = require('../utils/streamFilters');
const { getDetails, resolveImdbId } = require('../utils/tmdb');
const { getAnimeStream, getServers: getAnimeServers } = require('../services/hianime');
const logger = require('../utils/logger');

module.exports = function(metrics) {
    // Shared Anime Proxy Payload Helper (if not in a common util)
    function proxyAnimeSuccessPayload(req, data) {
        if (!config.enableProxy || !data.stream || !data.stream.sources) return data;
        const serverUrl = resolveServerUrl(req);
        
        const proxiedSources = data.stream.sources.map(s => ({
            ...s,
            url: buildHeaderProxyUrl(req, s.url, { Referer: data.stream.referer || 'https://megacloud.tv' }, chooseAnimeProxyRoute(s.url, 'stream'))
        }));

        const proxiedSubtitles = (data.stream.tracks || []).map(t => ({
            ...t,
            file: buildHeaderProxyUrl(req, t.file, { Referer: data.stream.referer || 'https://megacloud.tv' }, chooseAnimeProxyRoute(t.file, 'subtitle'))
        }));

        return {
            ...data,
            stream: {
                ...data.stream,
                sources: proxiedSources,
                tracks: proxiedSubtitles
            }
        };
    }

    function buildHeaderProxyUrl(req, targetUrl, headers = {}, routePath = '/api/proxy') {
        const baseUrl = `${resolveServerUrl(req)}${routePath}`;
        const params = new URLSearchParams();
        params.set('url', targetUrl);
        if (headers && Object.keys(headers).length > 0) params.set('headers', JSON.stringify(headers));
        return `${baseUrl}?${params.toString()}`;
    }

    function chooseAnimeProxyRoute(targetUrl, kind = 'stream') {
        const normalized = typeof targetUrl === 'string' ? targetUrl.toLowerCase() : '';
        if (kind === 'subtitle') return '/sub-proxy';
        if (/\.m3u8(\?|$)/i.test(normalized)) return '/m3u8-proxy';
        if (/\.(mp4|m4s|ts|mkv)(\?|$)/i.test(normalized)) return '/ts-proxy';
        return '/api/proxy';
    }

    // --- Endpoints ---

    // Dedicated Drama Endpoint (KissKH + Dramacool only)
    router.get('/drama/:type/:tmdbId', async (req,res) => {
        const { type, tmdbId } = req.params;
        const season = req.query.season ? Number(req.query.season) : null;
        const episode = req.query.episode ? Number(req.query.episode) : null;
        
        try {
            if (metrics) metrics.streamRequests++;
            const dramaProviders = ['kisskh', 'dramacool'];
            const streamsResults = [];
            const providerTimings = {};

            await Promise.all(dramaProviders.map(async name => {
                const prov = getProvider(name);
                if (!prov || !prov.enabled) return;
                
                if (metrics) metrics.providerCalls[name] = (metrics.providerCalls[name]||0)+1;
                try {
                    const t0 = Date.now();
                    const r = await prov.fetch({ tmdbId, type: 'tv', season, episode, imdbId: null, filters:{} });
                    providerTimings[name] = Date.now()-t0;
                    if (Array.isArray(r)) streamsResults.push(...r);
                } catch (e) {
                    console.error(`[api] drama provider ${name} failed:`, e.message);
                }
            }));

            let streams = streamsResults;
            streams = applyFilters(streams, 'drama', config.minQualities, config.excludeCodecs);
            if (metrics) metrics.streamsReturned += streams.length;

            if (config.enableProxy) {
                const serverUrl = resolveServerUrl(req);
                streams = processStreamsForProxy(streams, serverUrl);
            }

            res.json({ success: true, tmdbId, type: 'drama', count: streams.length, providerTimings, streams });
        } catch (e) {
            if (metrics) metrics.lastError = e.message;
            res.status(500).json({ success:false, error:'INTERNAL_ERROR', message:e.message });
        }
    });

    // Silent handler for disabled anime fetches
    router.all('/anime/*', (req, res) => {
        res.json({ success: true, streams: [], message: 'Anime fetches disabled' });
    });

    // Aggregate streams across all enabled providers
    router.get('/:type/:tmdbId', async (req,res) => {
        const { type, tmdbId } = req.params;
        const noCache = req.query.nocache === '1';
        if (!['movie','series'].includes(type)) return res.status(400).json({ success:false, error:'INVALID_TYPE' });
        const season = req.query.season ? Number(req.query.season) : null;
        const episode = req.query.episode ? Number(req.query.episode) : null;
        
        try {
            if (metrics) metrics.streamRequests++;
            const tmdbType = type === 'movie' ? 'movie' : 'tv';
            
            let tmdbData = null;
            try {
                tmdbData = await getDetails(tmdbType, tmdbId);
            } catch (e) {
                logger.warn(`[api] Failed to pre-fetch TMDB details for ${tmdbId}:`, e.message);
            }

            const imdbId = await resolveImdbId(tmdbType, tmdbId);
            if (imdbId && metrics) metrics.tmdbToImdbLookups++;
            
            const cachedStreams = noCache ? null : getCachedStreams(type, tmdbId, season, episode);
            if (cachedStreams && cachedStreams.length > 0) {
                let streams = applyFilters(cachedStreams, 'aggregate', config.minQualities, config.excludeCodecs);
                if (config.enableProxy) {
                    const serverUrl = resolveServerUrl(req);
                    streams = processStreamsForProxy(streams, serverUrl);
                }
                return res.json({ success: true, tmdbId, imdbId, count: streams.length, streams, fromCache: true });
            }
            
            const dramaProviders = ['dramacool', 'orient'];
            const selectedProviders = (config.defaultProviders.length ? config.defaultProviders : listProviders().map(p=>p.name))
                .filter(name => !dramaProviders.includes(name.toLowerCase()));

            const tStart = Date.now();

            const fastProviderPromises = [];
            const slowProviderPromises = [];

            const { isProviderHealthy, recordSuccess, recordFailure } = require('../utils/providerCircuitBreaker');

            selectedProviders.forEach(name => {
                const prov = getProvider(name);
                if (!prov || !prov.enabled) return;
                
                // Skip providers that are currently "tripped" (repeatedly failing)
                if (!isProviderHealthy(prov.realName)) {
                    logger.info(`[api] Skipping unhealthy provider: ${name}`);
                    return;
                }

                const isFast = prov.isFast;
                if (metrics) metrics.providerCalls[name] = (metrics.providerCalls[name]||0)+1;

                const promise = (async () => {
                    try {
                        const t0 = Date.now();
                        const r = await Promise.race([
                            prov.fetch({ tmdbId, type, season, episode, imdbId, tmdbData, filters:{ } }),
                            new Promise((_, reject) => setTimeout(() => reject(new Error('Provider timeout')), isFast ? 15000 : 60000))
                        ]);
                        providerTimings[name] = Date.now()-t0;
                        const results = Array.isArray(r) ? r : [];
                        
                        if (results.length > 0) {
                            recordSuccess(prov.realName);
                            addCachedStreams(type, tmdbId, season, episode, results);
                            streamsResults.push(...results);
                        } else {
                            // Empty results count as a failure for the circuit breaker
                            recordFailure(prov.realName);
                        }
                        return results;
                    } catch (e) {
                        logger.error(`[api] provider ${name} failed or timed out:`, e.message);
                        recordFailure(prov.realName);
                        return [];
                    }
                })();

                if (isFast) fastProviderPromises.push(promise);
                else slowProviderPromises.push(promise);
            });

            // Strategy: 
            // 1. Wait for all FAST providers (max 5s)
            // 2. If we have results, return.
            // 3. Otherwise wait for the remaining providers up to the deadline.
            
            const fastDeadline = new Promise(resolve => setTimeout(() => resolve('FAST_DEADLINE'), 5000));
            const slowDeadline = new Promise(resolve => setTimeout(() => resolve('DEADLINE'), 15000));
            
            await Promise.race([ Promise.all(fastProviderPromises), fastDeadline ]);
            
            if (streamsResults.length === 0) {
                // If no fast results, wait longer for any results
                await Promise.race([ Promise.all([...fastProviderPromises, ...slowProviderPromises]), slowDeadline ]);
            }

            const cachedAtEnd = getCachedStreams(type, tmdbId, season, episode);
            const finalStreams = (cachedAtEnd && cachedAtEnd.length > 0) ? cachedAtEnd : streamsResults;
            const totalDuration = Date.now() - tStart;

            let streams = finalStreams;
            streams = applyFilters(streams, 'aggregate', config.minQualities, config.excludeCodecs);
            if (metrics) metrics.streamsReturned += streams.length;
            
            if (config.enableProxy) {
                const serverUrl = resolveServerUrl(req);
                streams = processStreamsForProxy(streams, serverUrl);
            }
            
            res.json({ 
                success: true, 
                tmdbId, 
                imdbId, 
                count: streams.length, 
                providerTimings, 
                streams,
                totalDurationMs: totalDuration,
                status: Date.now() - tStart > 14000 ? "Partial" : "Complete"
            });
        } catch (e) {
            if (metrics) metrics.lastError = e.message;
            res.status(500).json({ success:false, error:'INTERNAL_ERROR', message:e.message });
        }
    });

    return router;
};
