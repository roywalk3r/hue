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
const axios = require('axios');
const hitvApi = require('../utils/hitv-api');
const { config } = require('../utils/config');
const { resolveServerUrl } = require('../utils/server');
const { processStreamsForProxy } = require('../proxy/proxyServer');
const { getCachedStreams, addCachedStreams } = require('../utils/cache');
const kisskhDirect = require('../providers/kisskh');
const sflixDirect = require('../providers/sflix');
const movieboxDirect = require('../providers/moviebox');

const RUST_URL = process.env.RUST_ENGINE_URL || 'http://127.0.0.1:24609';

async function rustFetch(path, timeout = 15000) {
    try {
        const { data } = await axios.get(`${RUST_URL}${path}`, { timeout });
        return { ok: true, data };
    } catch (e) {
        return { ok: false, error: e.message };
    }
}

/** Transform Rust HiTV list response to match frontend expectations */
function transformRustList(rustData, page, pageSize) {
    if (!rustData || !Array.isArray(rustData.items)) return null;
    const items = rustData.items.map(item => ({
        id: item.id,
        title: item.title,
        cover: item.cover || item.backdrop,
        rating: item.rating,
        releaseDate: item.year,
        genre: item.genre,
        type: 'tv',
        description: item.overview,
    }));
    const total = rustData.total || items.length;
    const hasNext = items.length > 0 && (page * pageSize) < total;
    return {
        success: true,
        items,
        page,
        hasNext,
        total,
        totalPages: Math.ceil(total / pageSize),
    };
}

module.exports = function(metrics) {
    router.get('/browse', async (req, res) => {
        const page = parseInt(req.query.page) || 1;
        const pageSize = parseInt(req.query.pageSize) || 28;
        const country = req.query.country || 'Korea';
        const category = req.query.category || 'Drama';
        const genre = req.query.genre || 'All';
        const sort = req.query.sort || 'Hottest';
        const year = req.query.year || null;

        // Delegate to Rust engine first
        const rustPath = `/api/hitv/browse?country=${encodeURIComponent(country)}&category=${encodeURIComponent(category)}&genre=${encodeURIComponent(genre)}&sort=${encodeURIComponent(sort)}&page=${page}&page_size=${pageSize}${year ? '&year=' + year : ''}`;
        const rust = await rustFetch(rustPath, 15000);
        if (rust.ok) {
            const transformed = transformRustList(rust.data, page, pageSize);
            if (transformed) return res.json(transformed);
        }

        // Fallback to local Node.js provider
        try {
            const data = await hitvApi.browseHiTv(page, pageSize, country, category, genre, sort, year);
            res.json({ success: true, ...data });
        } catch (e) {
            res.status(500).json({ success: false, error: e.message });
        }
    });

    router.get('/latest', async (req, res) => {
        const page = parseInt(req.query.page) || 1;
        const pageSize = parseInt(req.query.pageSize) || 28;
        const country = req.query.country || 'Korea';
        const genre = req.query.genre || 'All';

        const rustPath = `/api/hitv/latest?country=${encodeURIComponent(country)}&genre=${encodeURIComponent(genre)}&page=${page}&page_size=${pageSize}`;
        const rust = await rustFetch(rustPath, 15000);
        if (rust.ok) {
            const transformed = transformRustList(rust.data, page, pageSize);
            if (transformed) return res.json(transformed);
        }

        try {
            const params = { page, country, genre, sort: 'Newest' };
            const data = await hitvApi.browseDrama(params);
            res.json({ success: true, ...data });
        } catch (e) {
            res.status(500).json({ success: false, error: e.message });
        }
    });

    router.get('/trending', async (req, res) => {
        const page = parseInt(req.query.page) || 1;
        const pageSize = parseInt(req.query.pageSize) || 28;
        const country = req.query.country || 'Korea';
        const genre = req.query.genre || 'All';

        const rustPath = `/api/hitv/trending?country=${encodeURIComponent(country)}&genre=${encodeURIComponent(genre)}&page=${page}&page_size=${pageSize}`;
        const rust = await rustFetch(rustPath, 15000);
        if (rust.ok) {
            const transformed = transformRustList(rust.data, page, pageSize);
            if (transformed) return res.json(transformed);
        }

        try {
            const params = { page, country, genre, sort: 'Recommend' };
            const data = await hitvApi.browseDrama(params);
            res.json({ success: true, ...data });
        } catch (e) {
            res.status(500).json({ success: false, error: e.message });
        }
    });

    router.get('/search', async (req, res) => {
        const { q, type, page } = req.query;
        if (!q) return res.status(400).json({ success: false, error: 'Query required' });

        const rustPage = parseInt(page) || 1;
        const rustPath = `/api/hitv/search?q=${encodeURIComponent(q)}&page=${rustPage}&page_size=60`;
        const rust = await rustFetch(rustPath, 15000);
        if (rust.ok) {
            const transformed = transformRustList(rust.data, rustPage, 60);
            if (transformed) return res.json(transformed);
        }

        try {
            const data = await hitvApi.searchHiTv(q, parseInt(type) || 2, parseInt(page) || 1);
            res.json(data);
        } catch (e) {
            res.status(500).json({ success: false, error: e.message });
        }
    });

    router.get('/library', async (req, res) => {
        // No Rust equivalent; keep local
        try {
            const page = parseInt(req.query.page) || 1;
            const pageSize = parseInt(req.query.pageSize) || 5000;
            const allDramas = await hitvApi.getLibrary();

            const total = allDramas.length;
            const start = (page - 1) * pageSize;
            const end = start + pageSize;
            const items = allDramas.slice(start, end);

            res.json({
                success: true,
                count: total,
                page,
                pageSize,
                totalPages: Math.ceil(total / pageSize),
                hasNext: end < total,
                items,
            });
        } catch (e) {
            res.status(500).json({ success: false, error: 'Failed to load library data' });
        }
    });

    router.get('/hero', async (req, res) => {
        const rust = await rustFetch('/api/hitv/hero', 10000);
        if (rust.ok) {
            const transformed = transformRustList(rust.data, 1, 10);
            if (transformed) return res.json(transformed);
        }

        try {
            const dramas = await hitvApi.getHero();
            res.json({ success: true, count: dramas.length, items: dramas });
        } catch (e) {
            res.status(500).json({ success: false, error: 'Failed to load hero data' });
        }
    });

    router.get('/drama', async (req, res) => {
        const page = parseInt(req.query.page) || 1;
        const pageSize = parseInt(req.query.pageSize) || 500;
        const country = req.query.country || 'Korea';
        const genre = req.query.genre || 'All';
        const sort = req.query.sort || 'Hottest';
        const year = req.query.year || null;

        const rustPath = `/api/hitv/drama?country=${encodeURIComponent(country)}&genre=${encodeURIComponent(genre)}&sort=${encodeURIComponent(sort)}&page=${page}&page_size=${pageSize}${year ? '&year=' + year : ''}`;
        const rust = await rustFetch(rustPath, 15000);
        if (rust.ok) {
            const transformed = transformRustList(rust.data, page, pageSize);
            if (transformed) return res.json(transformed);
        }

        try {
            const params = { page, pageSize, country, genre, sort, year };
            const data = await hitvApi.browseDrama(params);
            res.json({ success: true, ...data });
        } catch (e) {
            res.status(500).json({ success: false, error: e.message });
        }
    });

    // Info endpoint: Rust doesn't return episodes/detailPath, keep local
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
        let { id, episode } = req.params;
        let targetTitle = req.query.title;

        // Support for slug format: the-judge-from-hell-4180063205928174912
        if (id.includes('-') && !/^\d+$/.test(id)) {
            const parts = id.split('-');
            const numericId = parts.pop();
            if (/^\d+$/.test(numericId)) {
                targetTitle = targetTitle || parts.join(' ').replace(/-/g, ' ');
                id = numericId;
                console.log(`[HiTV] Parsed slug: title="${targetTitle}", id=${id}`);
            }
        }

        try {
            const cached = await getCachedStreams('hitv', id, 1, episode);
            if (cached && cached.length > 0) {
                let finalStreams = cached;
                if (config.enableProxy) {
                    const serverUrl = resolveServerUrl(req);
                    finalStreams = processStreamsForProxy(finalStreams, serverUrl);
                }
                return res.json({ success: true, source: 'cache', episode, streams: finalStreams });
            }

            const info = await hitvApi.getHiTvDetail(id);
            targetTitle = targetTitle || (info ? info.title : null);

            if (!targetTitle || targetTitle === 'Unknown') {
                return res.status(404).json({ success: false, error: 'Show title not found. Please provide ?title= param.' });
            }

            const fetchWithTimeout = (promise, name, ms = 15000) =>
                Promise.race([
                    promise,
                    new Promise(resolve => setTimeout(() => resolve([]), ms)),
                ]);

            const [kissStreams, sflixStreams, movieboxStreams] = await Promise.all([
                fetchWithTimeout(kisskhDirect.getStreamsDirect(targetTitle, episode).catch(() => []), 'Zenith', 30000),
                fetchWithTimeout((async () => {
                    if (info && info.detailPath) return sflixDirect.getStreamsBySlug(id, info.detailPath, episode);
                    return sflixDirect.getStreamsDirect(targetTitle, episode, 1);
                })().catch(() => []), 'Nova', 35000),
                fetchWithTimeout(movieboxDirect.getStreamsDirect(targetTitle, episode, 1).catch(() => []), 'Nebula', 25000),
            ]);

            let streams = [...kissStreams, ...sflixStreams, ...movieboxStreams];
            if (streams.length > 0) await addCachedStreams('hitv', id, 1, episode, streams);
            if (config.enableProxy) {
                const serverUrl = resolveServerUrl(req);
                streams = processStreamsForProxy(streams, serverUrl);
            }
            res.json({ success: true, id, episode, count: streams.length, streams });
        } catch (e) {
            res.status(500).json({ success: false, error: e.message });
        }
    });

    // MASSIVE DYNAMIC DISCOVERY
    router.get('/:country/:sort?/:genre?', async (req, res) => {
        try {
            const { country, sort, genre } = req.params;
            const page = parseInt(req.query.page) || 1;
            const pageSize = parseInt(req.query.pageSize) || 500;
            const category = req.query.category || 'Drama';

            const sortMap = {
                latest: 'Newest',
                hottest: 'Hottest',
                foryou: 'ForYou',
                trending: 'Recommend',
                popular: 'Hottest',
            };
            const mappedSort = sortMap[sort?.toLowerCase()] || sort || 'ForYou';

            const rustPath = `/api/hitv/${encodeURIComponent(country)}/${encodeURIComponent(mappedSort)}/${encodeURIComponent(genre || 'All')}?category=${encodeURIComponent(category)}&page=${page}&page_size=${pageSize}`;
            const rust = await rustFetch(rustPath, 15000);
            if (rust.ok) {
                const transformed = transformRustList(rust.data, page, pageSize);
                if (transformed) return res.json(transformed);
            }

            const params = {
                page,
                pageSize,
                country: country || 'All',
                sort: mappedSort,
                genre: genre || 'All',
                year: req.query.year || null,
            };

            const data = await hitvApi.browseDrama(params);
            res.json({ success: true, ...data });
        } catch (e) {
            res.status(500).json({ success: false, error: e.message });
        }
    });

    return router;
};
