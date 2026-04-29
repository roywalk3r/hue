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
const { config } = require('../utils/config');
const { resolveServerUrl } = require('../utils/server');
const { processStreamsForProxy } = require('../proxy/proxyServer');
const { getCachedStreams, addCachedStreams } = require('../utils/cache');
const { listProviders, getProvider } = require('../providers/registry');
const { applyFilters } = require('../utils/streamFilters');
const { getDetails, resolveImdbId } = require('../utils/tmdb');
const { getAnimeStream, getServers: getAnimeServers } = require('../services/hianime');
const logger = require('../utils/logger');

// In-flight request coalescing: same key → same Promise
const pendingFetches = new Map();

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

    // Dedicated Drama Endpoint — delegates to Rust engine first, then KissKH / Dramacool fallback
    router.get('/drama/:type/:tmdbId', async (req,res) => {
        const { type, tmdbId } = req.params;
        const season = req.query.season ? Number(req.query.season) : null;
        const episode = req.query.episode ? Number(req.query.episode) : null;

        try {
            if (metrics) metrics.streamRequests++;
            const streamsResults = [];
            const providerTimings = {};

            // 1. Try Rust engine via huerust first (handles all providers including drama)
            const huerust = getProvider('huerust');
            if (huerust && huerust.enabled) {
                try {
                    const t0 = Date.now();
                    const r = await huerust.fetch({ tmdbId, type: 'tv', season, episode, filters:{} });
                    providerTimings['huerust'] = Date.now()-t0;
                    if (Array.isArray(r) && r.length) {
                        streamsResults.push(...r);
                        console.log(`[api/drama] Rust engine returned ${r.length} streams`);
                    }
                } catch (e) {
                    console.warn(`[api/drama] Rust engine failed:`, e.message);
                }
            }

            // 2. Fallback to individual Node.js drama providers if Rust returned nothing
            if (!streamsResults.length) {
                const dramaProviders = ['kisskh', 'dramacool'];
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
            }

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

    // SSE Live streaming endpoint — streams results as providers complete
    router.get('/:type/:tmdbId/live', async (req, res) => {
        const { type, tmdbId } = req.params;
        if (!['movie','series','tv'].includes(type)) {
            return res.status(400).json({ success:false, error:'INVALID_TYPE' });
        }
        const season = req.query.season ? Number(req.query.season) : null;
        const episode = req.query.episode ? Number(req.query.episode) : null;
        const noCache = req.query.nocache === '1';

        res.setHeader('Content-Type', 'text/event-stream');
        res.setHeader('Cache-Control', 'no-cache');
        res.setHeader('Connection', 'keep-alive');
        res.flushHeaders();

        // Keepalive: browsers close SSE after ~30s of silence
        const heartbeat = setInterval(() => {
            if (!res.writableEnded) res.write(': heartbeat\n\n');
        }, 15000);
        res.on('close', () => clearInterval(heartbeat));

        const sendEvent = (event, data) => {
            res.write(`event: ${event}\ndata: ${JSON.stringify(data)}\n\n`);
        };

        try {
            if (metrics) metrics.streamRequests++;
            const tmdbType = type === 'movie' ? 'movie' : 'tv';

            // Check cache first
            if (!noCache) {
                const cached = await getCachedStreams(type, tmdbId, season, episode);
                if (cached && cached.length > 0) {
                    let streams = applyFilters(cached, 'aggregate', config.minQualities, config.excludeCodecs);
                    if (config.enableProxy) {
                        const serverUrl = resolveServerUrl(req);
                        streams = processStreamsForProxy(streams, serverUrl);
                    }
                    sendEvent('cached', { streams, count: streams.length, cached: true });
                    sendEvent('done', { total: streams.length, cached: true });
                    res.end();
                    return;
                }
            }

            let tmdbData = null;
            try {
                tmdbData = await getDetails(tmdbType, tmdbId);
            } catch (e) {
                logger.warn(`[api/live] Failed to pre-fetch TMDB details for ${tmdbId}:`, e.message);
            }

            const imdbId = await resolveImdbId(tmdbType, tmdbId);

            const dramaProviders = ['dramacool', 'orient'];
            const selectedProviders = (config.defaultProviders.length ? config.defaultProviders : listProviders().map(p=>p.name))
                .filter(name => !dramaProviders.includes(name.toLowerCase()));

            const { isProviderHealthy, recordSuccess, recordFailure } = require('../utils/providerCircuitBreaker');
            const rawResults = [];

            const providerPromises = selectedProviders.map(name => {
                const prov = getProvider(name);
                if (!prov || !prov.enabled) return null;
                if (!isProviderHealthy(prov.realName)) {
                    logger.info(`[api/live] Skipping unhealthy provider: ${name}`);
                    return null;
                }

                return (async () => {
                    try {
                        const t0 = Date.now();
                        const r = await Promise.race([
                            prov.fetch({ tmdbId, type, season, episode, imdbId, tmdbData, filters:{} }),
                            new Promise((_, reject) => setTimeout(() => reject(new Error('Provider timeout')), prov.isFast ? 15000 : 60000))
                        ]);
                        const elapsedMs = Date.now() - t0;
                        const results = Array.isArray(r) ? r : [];

                        if (results.length > 0) {
                            recordSuccess(prov.realName);
                            const NO_CACHE_STREAM_PROVIDERS = ['Blaze', 'Aura', 'Cinema', 'Nova', 'Huerust'];
                            const cacheableResults = results.filter(s => !NO_CACHE_STREAM_PROVIDERS.includes(s.provider));
                            if (cacheableResults.length > 0) await addCachedStreams(type, tmdbId, season, episode, cacheableResults);
                            rawResults.push(...results);

                            // Huerust returns streams with per-provider names — emit one event per group
                            const groups = new Map();
                            for (const s of results) {
                                const p = s.provider || prov.name;
                                if (!groups.has(p)) groups.set(p, []);
                                groups.get(p).push(s);
                            }

                            for (const [provName, groupStreams] of groups) {
                                let display = applyFilters(groupStreams, 'aggregate', config.minQualities, config.excludeCodecs);
                                if (config.enableProxy) {
                                    const serverUrl = resolveServerUrl(req);
                                    display = processStreamsForProxy(display, serverUrl);
                                }
                                sendEvent('provider', { provider: provName, streams: display, count: display.length, elapsed_ms: elapsedMs });
                            }
                        } else {
                            recordFailure(prov.realName);
                        }
                    } catch (e) {
                        logger.error(`[api/live] provider ${name} failed or timed out:`, e.message);
                        recordFailure(prov.realName);
                    }
                })();
            }).filter(Boolean);

            await Promise.all(providerPromises);

            // Deduplicate by URL
            const urlMap = new Map();
            rawResults.forEach(s => { if(s.url) urlMap.set(s.url, s); });
            const deduped = Array.from(urlMap.values());

            sendEvent('done', { total: deduped.length, cached: false });
            res.end();
        } catch (e) {
            logger.error('[api/live] SSE error:', e.message);
            sendEvent('error', { message: e.message });
            res.end();
        }
    });

    // Aggregate streams across all enabled providers
    router.get('/:type/:tmdbId', async (req,res) => {
        const { type, tmdbId } = req.params;
        const noCache = req.query.nocache === '1';
        if (!['movie','series','tv'].includes(type)) return res.status(400).json({ success:false, error:'INVALID_TYPE' });
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
            
            const cachedStreams = noCache ? null : await getCachedStreams(type, tmdbId, season, episode);
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

            // Coalesce duplicate in-flight requests for the same content
            const fetchKey = `${type}:${tmdbId}:${season || 0}:${episode || 0}`;
            if (!noCache && pendingFetches.has(fetchKey)) {
                const result = await pendingFetches.get(fetchKey);
                let streams = applyFilters(result, 'aggregate', config.minQualities, config.excludeCodecs);
                if (config.enableProxy) streams = processStreamsForProxy(streams, resolveServerUrl(req));
                return res.json({ success: true, tmdbId, imdbId, count: streams.length, providerTimings: {}, streams, totalDurationMs: 0, status: 'Complete' });
            }

            const tStart = Date.now();
            const providerTimings = {};
            const streamsResults = [];

            // Register this fetch so concurrent identical requests coalesce
            let resolvePending;
            const pendingPromise = new Promise(r => { resolvePending = r; });
            if (!noCache) pendingFetches.set(fetchKey, pendingPromise);

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
                            new Promise((_, reject) => setTimeout(() => reject(new Error('Provider timeout')), name === 'huerust' ? 40000 : (isFast ? 15000 : 60000)))
                        ]);
                        providerTimings[name] = Date.now()-t0;
                        const results = Array.isArray(r) ? r : [];
                        
                        if (results.length > 0) {
                            recordSuccess(prov.realName);
                            const NO_CACHE_STREAM_PROVIDERS = ['Blaze', 'Aura', 'Cinema', 'Nova', 'Huerust'];
                            const cacheableResults = results.filter(s => !NO_CACHE_STREAM_PROVIDERS.includes(s.provider));
                            if (cacheableResults.length > 0) await addCachedStreams(type, tmdbId, season, episode, cacheableResults);
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
            
            const fastDeadline = new Promise(resolve => setTimeout(() => resolve('FAST_DEADLINE'), 10000));
            const slowDeadline = new Promise(resolve => setTimeout(() => resolve('DEADLINE'), 35000));
            
            await Promise.race([ Promise.all(fastProviderPromises), fastDeadline ]);
            
            if (streamsResults.length === 0) {
                // If no fast results, wait longer for any results
                await Promise.race([ Promise.all([...fastProviderPromises, ...slowProviderPromises]), slowDeadline ]);
            }

            // Prefer live streamsResults (includes no-cache providers like Huerust/Cinema/Aura).
            // Only fall back to cache if the live fetch returned nothing at all.
            const cachedAtEnd = streamsResults.length === 0 ? await getCachedStreams(type, tmdbId, season, episode) : null;
            const finalStreams = streamsResults.length > 0 ? streamsResults : (cachedAtEnd || []);
            const totalDuration = Date.now() - tStart;

            // Resolve coalesced waiters and clean up
            if (resolvePending) { resolvePending(finalStreams); pendingFetches.delete(fetchKey); }

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
                status: Date.now() - tStart > 34000 ? "Partial" : "Complete"
            });
        } catch (e) {
            if (metrics) metrics.lastError = e.message;
            res.status(500).json({ success:false, error:'INTERNAL_ERROR', message:e.message });
        }
    });

    return router;
};
