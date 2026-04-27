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
const { applyFilters } = require('../utils/streamFilters');
const kisskhApi = require('../utils/kisskh-api');
const kisskhProvider = require('../providers/kisskh');
const dramacoolApi = require('../utils/dramacool-api');
const dramacoolProvider = require('../providers/dramacool');
const axios = require('axios');

const RUST_URL = process.env.RUST_ENGINE_URL || 'http://127.0.0.1:24609';

async function rustFetch(path, timeout = 15000) {
    try {
        const { data } = await axios.get(`${RUST_URL}${path}`, { timeout });
        return { ok: true, data };
    } catch (e) {
        return { ok: false, error: e.message };
    }
}

module.exports = function(metrics) {
    // KissKH Routes
    router.get('/kdrama/search', async (req, res) => {
        const q = req.query.q;
        if (!q) return res.status(400).json({ success: false, error: 'Query required' });

        // Delegate to Rust engine first
        const rust = await rustFetch(`/api/drama/kisskh/search?q=${encodeURIComponent(q)}`);
        if (rust.ok && Array.isArray(rust.data)) {
            return res.json({ success: true, results: rust.data });
        }

        // Fallback to local Node.js provider
        try {
            const results = await kisskhApi.search(q);
            res.json({ success: true, results });
        } catch (e) {
            res.status(500).json({ success: false, error: e.message });
        }
    });

    router.get('/kdrama/latest', async (req, res) => {
        // Rust does not have a dedicated /latest endpoint for KissKH; use local
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

        // Delegate to Rust engine first
        const rust = await rustFetch(`/api/drama/kisskh/streams/${encodeURIComponent(showId)}/${episode}`);
        if (rust.ok && Array.isArray(rust.data) && rust.data.length > 0) {
            let streams = rust.data;
            if (config.enableProxy) {
                const serverUrl = resolveServerUrl(req);
                streams = processStreamsForProxy(streams, serverUrl);
            }
            return res.json({ success: true, showId, episode, count: streams.length, streams });
        }

        // Fallback to local Node.js provider
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

        // Delegate to Rust engine first
        const rust = await rustFetch(`/api/drama/dramacool/search?q=${encodeURIComponent(q)}`);
        if (rust.ok && Array.isArray(rust.data)) {
            return res.json({ success: true, results: rust.data });
        }

        // Fallback to local Node.js provider
        try {
            const data = await dramacoolApi.search(q, page || 1);
            res.json(data);
        } catch (e) {
            res.status(500).json({ success: false, error: e.message });
        }
    });

    router.get('/dramacool/latest', async (req, res) => {
        // Rust does not have a dedicated /latest endpoint for Dramacool; use local
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

        // Delegate to Rust engine first
        const rust = await rustFetch(`/api/drama/dramacool/streams/${encodeURIComponent(id)}/${episode}`);
        if (rust.ok && Array.isArray(rust.data) && rust.data.length > 0) {
            let streams = rust.data;
            if (config.enableProxy) {
                const serverUrl = resolveServerUrl(req);
                streams = processStreamsForProxy(streams, serverUrl);
            }
            return res.json({ success: true, id, episode, count: streams.length, streams });
        }

        // Fallback to local Node.js provider
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
