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
const { getProvider } = require('../providers/registry');
const { applyFilters } = require('../utils/streamFilters');
const aisportsApi = require('../utils/aisports-api');

function resolveSportsMatchName(req) {
    const base = req.params?.matchName ?? req.params?.id ?? '';
    return `${base}${req.params?.[0] || ''}`;
}

function createSportsRoutes(metrics) {
    async function handleSportsRequest(req, res) {
        let matchName = resolveSportsMatchName(req);
        if (!matchName) return res.status(400).json({ success: false, error: 'Match name required' });

        try {
            if (metrics) metrics.streamRequests++;
            
            // Direct ID handling
            let directId = null;
            if (/^\d{15,}$/.test(matchName)) directId = matchName;
            else if (matchName.includes('id=')) {
                const m = matchName.match(/id=(\d{15,})/);
                if (m) directId = m[1];
            }

            const requestPath = `${req.baseUrl || ''}${req.path || ''}`;
            const isAiSportsPlayRoute = requestPath.startsWith('/api/aisports/play/') || requestPath.startsWith('/aisports/play/');
            if (directId && !isAiSportsPlayRoute) return res.redirect(`/api/aisports/play/${directId}`);

            const isPpvRoute = req.path.startsWith('/api/ppv/streams/');
            const isPpvUri = /^[a-z0-9-]+\/\d{4}-\d{2}-\d{2}\/[a-z0-9-]+$/i.test(matchName);
            const sportsProviders = (isPpvRoute || isPpvUri) ? ['ppv'] : ['aisports', 'livesport', 'livetv', 'ppv'];
            
            const streamsResults = [];
            const providerTimings = {};

            await Promise.all(sportsProviders.map(async name => {
                const prov = getProvider(name);
                if (!prov || !prov.enabled) return;
                
                if (metrics) metrics.providerCalls[prov.name] = (metrics.providerCalls[prov.name] || 0) + 1;
                try {
                    const t0 = Date.now();
                    const r = await prov.fetch({ req, type: 'sports', matchName, filters: {} });
                    providerTimings[prov.name] = Date.now() - t0;
                    if (Array.isArray(r)) streamsResults.push(...r);
                } catch (e) {
                    console.error(`[api] sports provider ${name} failed:`, e.message);
                }
            }));

            let streams = applyFilters(streamsResults, 'sports', config.minQualities, config.excludeCodecs);
            if (metrics) metrics.streamsReturned += streams.length;
            
            if (config.enableProxy) {
                const serverUrl = resolveServerUrl(req);
                streams = processStreamsForProxy(streams, serverUrl);
            }
            
            res.json({ success: true, matchName, count: streams.length, providerTimings, streams });
        } catch (e) {
            if (metrics) metrics.lastError = e.message;
            res.status(500).json({ success: false, error: 'INTERNAL_ERROR', message: e.message });
        }
    }

    // Routes
    router.get('/aisports/leagues', async (req, res) => {
        try {
            const data = await aisportsApi.getLeagues();
            res.json({ success: true, list: data });
        } catch (e) {
            res.status(500).json({ success: false, error: e.message });
        }
    });

    router.get('/aisports/list/:leagueId?', async (req, res) => {
        try {
            const leagueId = req.params.leagueId || 0;
            const { status, type } = req.query;
            const data = await aisportsApi.getMatchList(leagueId, status, type);
            res.json({ success: true, ...data });
        } catch (e) {
            res.status(500).json({ success: false, error: e.message });
        }
    });

    router.get('/aisports/flow/:id', async (req, res) => {
        try {
            const data = await aisportsApi.getMatchFlow(req.params.id, req.query.pageSize, req.query.lastSort);
            res.json({ success: true, ...data });
        } catch (e) {
            res.status(500).json({ success: false, error: e.message });
        }
    });

    router.get('/aisports/play/:id', handleSportsRequest);
    router.get('/ppv/streams/:matchName*', handleSportsRequest);
    
    // Fallback/Legacy routes mounted at root in apiServer.js
    router.get('/sports-stream/:matchName*', handleSportsRequest);
    router.get('/livetv/:matchName*', handleSportsRequest);

    return router;
}

module.exports = createSportsRoutes;
module.exports.resolveSportsMatchName = resolveSportsMatchName;
