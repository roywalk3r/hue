const express = require('express');
const router = express.Router();
const hianime = require('../services/hianime');

module.exports = function(metrics) {
    /**
     * Search Anime
     * GET /api/anime/search?q=query&page=1
     */
    router.get('/search', async (req, res) => {
        try {
            const { q, page } = req.query;
            if (!q) return res.status(400).json({ success: false, error: 'Missing query' });
            const result = await hianime.search(q, parseInt(page) || 1);
            res.json({ success: true, ...result });
        } catch (e) {
            res.status(500).json({ success: false, error: e.message });
        }
    });

    /**
     * Latest Anime
     * GET /api/anime/latest?page=1
     */
    router.get('/latest', async (req, res) => {
        try {
            const { page } = req.query;
            const result = await hianime.getLatest(parseInt(page) || 1);
            res.json({ success: true, ...result });
        } catch (e) {
            res.status(500).json({ success: false, error: e.message });
        }
    });

    /**
     * Category Feed (TV, Movie, OVA, ONA)
     * GET /api/anime/category?type=tv&page=1
     */
    router.get('/category', async (req, res) => {
        try {
            const { type, page, limit } = req.query;
            const result = await hianime.getByCategory(type || 'tv', parseInt(page) || 1, parseInt(limit) || 20);
            res.json({ success: true, ...result });
        } catch (e) {
            res.status(500).json({ success: false, error: e.message });
        }
    });

    /**
     * Home Feed
     * GET /api/anime/home
     */
    router.get('/home', async (req, res) => {
        try {
            const result = await hianime.getHome();
            res.json({ success: true, ...result });
        } catch (e) {
            res.status(500).json({ success: false, error: e.message });
        }
    });

    /**
     * Get Servers for an Anime Episode
     * GET /api/anime/servers?id=slug-episode-hash
     */
    router.get('/servers', async (req, res) => {
        try {
            const { id } = req.query;
            if (!id) return res.status(400).json({ success: false, error: 'Missing ID parameter' });
            
            const result = await hianime.getServers(id);
            res.json({ success: true, ...result });
        } catch (e) {
            res.status(500).json({ success: false, error: e.message });
        }
    });

    /**
     * Get Direct Stream for an Anime Episode
     * GET /api/anime/stream?id=slug-episode-hash&server=HD-1&type=sub&episode=1
     */
    router.get('/stream', async (req, res) => {
        try {
            const { id, server, type, episode } = req.query;
            if (!id) return res.status(400).json({ success: false, error: 'Missing ID parameter' });

            const result = await hianime.getAnimeStream({ 
                id, 
                server, 
                type, 
                episode: parseInt(episode) || 1 
            });
            if (result.error) {
                return res.status(404).json({ success: false, ...result });
            }
            res.json({ success: true, ...result });
        } catch (e) {
            res.status(500).json({ success: false, error: e.message });
        }
    });

    return router;
};
