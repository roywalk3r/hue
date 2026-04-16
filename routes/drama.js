const express = require('express');
const router = express.Router();
const { config } = require('../utils/config');
const { resolveServerUrl } = require('../utils/server');
const { processStreamsForProxy } = require('../proxy/proxyServer');
const { applyFilters } = require('../utils/streamFilters');
const kisskhApi = require('../utils/kisskh-api');
const kisskhProvider = require('../providers/kisskh');
const dramacoolApi = require('../utils/dramacool-api');
const dramacoolProvider = require('../providers/dramacool');

module.exports = function(metrics) {
    // KissKH Routes
    router.get('/kdrama/search', async (req, res) => {
        const q = req.query.q;
        if (!q) return res.status(400).json({ success: false, error: 'Query required' });
        try {
            const results = await kisskhApi.search(q);
            res.json({ success: true, results });
        } catch (e) {
            res.status(500).json({ success: false, error: e.message });
        }
    });

    router.get('/kdrama/latest', async (req, res) => {
        try {
            const page = req.query.page || 1;
            const data = await kisskhApi.getList({ page, order: 2 });
            res.json({ success: true, ...data });
        } catch (e) {
            res.status(500).json({ success: false, error: e.message });
        }
    });

    router.get('/kdrama/streams/:id/:episode?', async (req, res) => {
        const showId = req.params.id;
        const episode = req.params.episode || 1;
        try {
            if (metrics) metrics.streamRequests++;
            const streams = await kisskhProvider.getStreamsDirect(showId, episode);
            let finalStreams = applyFilters(streams, 'kisskh', config.minQualities, config.excludeCodecs);
            if (metrics) metrics.streamsReturned += finalStreams.length;
            if (config.enableProxy) {
                const serverUrl = resolveServerUrl(req);
                finalStreams = processStreamsForProxy(finalStreams, serverUrl);
            }
            res.json({ success: true, showId, episode, count: finalStreams.length, streams: finalStreams });
        } catch (e) {
            res.status(500).json({ success: false, error: e.message });
        }
    });

    // Dramacool Routes
    router.get('/dramacool/search', async (req, res) => {
        const { q, page } = req.query;
        if (!q) return res.status(400).json({ success: false, error: 'Query required' });
        try {
            const data = await dramacoolApi.search(q, page || 1);
            res.json(data);
        } catch (e) {
            res.status(500).json({ success: false, error: e.message });
        }
    });

    router.get('/dramacool/latest', async (req, res) => {
        try {
            const data = await dramacoolApi.getLatest(req.query.page || 1);
            res.json(data);
        } catch (e) {
            res.status(500).json({ success: false, error: e.message });
        }
    });

    router.get('/dramacool/browse/streams/:id/:episode', async (req, res) => {
        let { id, episode } = req.params;
        id = id.replace(/https?:\/\/dramacool\.sh\//, '').replace(/^\/|\/$/g, '');
        const url = `https://dramacool.sh/${id}-episode-${episode}/`;
        try {
            if (metrics) metrics.streamRequests++;
            const streams = await dramacoolProvider.getStreamsByUrl(url);
            let finalStreams = applyFilters(streams, 'dramacool', config.minQualities, config.excludeCodecs);
            if (metrics) metrics.streamsReturned += finalStreams.length;
            if (config.enableProxy) {
                const serverUrl = resolveServerUrl(req);
                finalStreams = processStreamsForProxy(finalStreams, serverUrl);
            }
            res.json({ success: true, id, episode, count: finalStreams.length, streams: finalStreams });
        } catch (e) {
            res.status(500).json({ success: false, error: e.message });
        }
    });

    return router;
};
