const express = require('express');
const router = express.Router();
const tmdb = require('../utils/tmdb');

module.exports = function(metrics) {
    /**
     * Search TMDB (Universal)
     * GET /api/tmdb/search?q=query&page=1
     */
    router.get('/search', async (req, res) => {
        try {
            const { q, page } = req.query;
            if (!q) return res.status(400).json({ success: false, error: 'Query parameter "q" is required' });
            
            const results = await tmdb.search(q, parseInt(page) || 1);
            res.json(results);
        } catch (e) {
            res.status(500).json({ success: false, error: e.message });
        }
    });

    /**
     * Trending (Universal)
     * GET /api/tmdb/trending?mediaType=all&timeWindow=day
     */
    router.get('/trending', async (req, res) => {
        try {
            const { mediaType, timeWindow, page } = req.query;
            const results = await tmdb.getTrending(mediaType || 'all', timeWindow || 'day', parseInt(page) || 1);
            res.json({ results });
        } catch (e) {
            res.status(500).json({ success: false, error: e.message });
        }
    });

    /**
     * Now Playing Movies
     */
    router.get('/movie/now-playing', async (req, res) => {
        try {
            const { page } = req.query;
            const results = await tmdb.getNowPlaying(parseInt(page) || 1);
            res.json({ results });
        } catch (e) {
            res.status(500).json({ success: false, error: e.message });
        }
    });

    /**
     * Popular Movies
     */
    router.get('/movie/popular', async (req, res) => {
        try {
            const { page } = req.query;
            const results = await tmdb.getPopular('movie', parseInt(page) || 1);
            res.json({ results });
        } catch (e) {
            res.status(500).json({ success: false, error: e.message });
        }
    });

    /**
     * Popular TV
     */
    router.get('/tv/popular', async (req, res) => {
        try {
            const { page } = req.query;
            const results = await tmdb.getPopular('tv', parseInt(page) || 1);
            res.json({ results });
        } catch (e) {
            res.status(500).json({ success: false, error: e.message });
        }
    });

    return router;
};
