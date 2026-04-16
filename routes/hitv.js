const express = require('express');
const router = express.Router();
const hitvApi = require('../utils/hitv-api');
const { config } = require('../utils/config');
const { resolveServerUrl } = require('../utils/server');
const { processStreamsForProxy } = require('../proxy/proxyServer');
const { getCachedStreams, addCachedStreams } = require('../utils/cache');
const kisskhDirect = require('../providers/kisskh');
const sflixDirect = require('../providers/sflix');

module.exports = function(metrics) {
    router.get('/browse', async (req, res) => {
        try {
            const page = req.query.page || 1;
            const pageSize = req.query.pageSize || 28;
            const country = req.query.country || "Korea";
            const category = req.query.category || "Drama";
            const genre = req.query.genre || "All";
            const sort = req.query.sort || "Hottest";
            const data = await hitvApi.browseHiTv(page, pageSize, country, category, genre, sort);
            res.json({ success: true, ...data });
        } catch (e) {
            res.status(500).json({ success: false, error: e.message });
        }
    });

    router.get('/latest', async (req, res) => {
        try {
            const params = {
                page: req.query.page || 1,
                country: req.query.country || "Korea",
                genre: req.query.genre || "All",
                sort: "Newest"
            };
            const data = await hitvApi.browseDrama(params);
            res.json({ success: true, ...data });
        } catch (e) {
            res.status(500).json({ success: false, error: e.message });
        }
    });

    router.get('/trending', async (req, res) => {
        try {
            const params = {
                page: req.query.page || 1,
                country: req.query.country || "Korea",
                genre: req.query.genre || "All",
                sort: "Recommend"
            };
            const data = await hitvApi.browseDrama(params);
            res.json({ success: true, ...data });
        } catch (e) {
            res.status(500).json({ success: false, error: e.message });
        }
    });

    router.get('/search', async (req, res) => {
        const { q, type, page } = req.query;
        if (!q) return res.status(400).json({ success: false, error: 'Query required' });
        try {
            const data = await hitvApi.searchHiTv(q, parseInt(type) || 2, parseInt(page) || 1);
            res.json(data);
        } catch (e) {
            res.status(500).json({ success: false, error: e.message });
        }
    });

    router.get('/library', async (req, res) => {
        try {
            const dramas = await hitvApi.getLibrary();
            res.json({ success: true, count: dramas.length, items: dramas });
        } catch (e) {
            res.status(500).json({ success: false, error: 'Failed to load library data' });
        }
    });

    router.get('/hero', async (req, res) => {
        try {
            const dramas = await hitvApi.getHero();
            res.json({ success: true, count: dramas.length, items: dramas });
        } catch (e) {
            res.status(500).json({ success: false, error: 'Failed to load hero data' });
        }
    });

    router.get('/drama', async (req, res) => {
        try {
            const params = {
                page: req.query.page || 1,
                country: req.query.country || "Korea",
                genre: req.query.genre || "All",
                sort: req.query.sort || "Hottest",
                year: req.query.year || null
            };
            const data = await hitvApi.browseDrama(params);
            res.json({ success: true, ...data });
        } catch (e) {
            res.status(500).json({ success: false, error: e.message });
        }
    });

    router.get('/info/:id', async (req, res) => {
        try {
            const data = await hitvApi.getHiTvDetail(req.params.id);
            if (!data) return res.status(404).json({ success: false, error: 'Not found' });
            res.json({ success: true, ...data });
        } catch (e) {
            res.status(500).json({ success: false, error: e.message });
        }
    });

    router.get('/countries', (req, res) => res.json(hitvApi.getHiTvCountries()));
    router.get('/categories', (req, res) => res.json(hitvApi.getHiTvCategories()));
    router.get('/genres', (req, res) => res.json(hitvApi.getHiTvGenres()));
    router.get('/sorts', (req, res) => res.json(hitvApi.getHiTvSorts()));

    router.get('/play/:id/:episode', async (req, res) => {
        const { id, episode } = req.params;
        try {
            const cached = getCachedStreams('hitv', id, 1, episode);
            if (cached && cached.length > 0) {
                let finalStreams = cached;
                if (config.enableProxy) {
                    const serverUrl = resolveServerUrl(req);
                    finalStreams = processStreamsForProxy(finalStreams, serverUrl);
                }
                return res.json({ success: true, source: 'cache', episode, streams: finalStreams });
            }

            const info = await hitvApi.getHiTvDetail(id);
            let targetTitle = req.query.title || (info ? info.title : null);

            if (!targetTitle || targetTitle === "Unknown") {
                return res.status(404).json({ success: false, error: 'Show title not found. Please provide ?title= param.' });
            }

            const fetchWithTimeout = (promise, name, ms = 15000) => 
                Promise.race([
                    promise,
                    new Promise(resolve => setTimeout(() => resolve([]), ms))
                ]);

            const [kissStreams, sflixStreams] = await Promise.all([
                fetchWithTimeout(kisskhDirect.getStreamsDirect(targetTitle, episode).catch(() => []), "Zenith", 30000),
                fetchWithTimeout((async () => {
                    if (info && info.detailPath) return sflixDirect.getStreamsBySlug(id, info.detailPath, episode);
                    return sflixDirect.getStreamsDirect(targetTitle, episode, 1);
                })().catch(() => []), "Nova", 35000)
            ]);

            let streams = [...kissStreams, ...sflixStreams];
            if (streams.length > 0) addCachedStreams('hitv', id, 1, episode, streams);
            if (config.enableProxy) {
                const serverUrl = resolveServerUrl(req);
                streams = processStreamsForProxy(streams, serverUrl);
            }
            res.json({ success: true, id, episode, count: streams.length, streams });
        } catch (e) {
            res.status(500).json({ success: false, error: e.message });
        }
    });

    return router;
};
