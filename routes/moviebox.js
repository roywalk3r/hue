const express = require('express');
const router = express.Router();
const movieboxApi = require('../utils/moviebox-api');
const { config } = require('../utils/config');
const { resolveServerUrl } = require('../utils/server');
const { processStreamsForProxy } = require('../proxy/proxyServer');

// Helper to update metrics (we'll need to find a way to share metrics)
// For now, we'll assume a global or pass it in.
// Alternatively, we can export a function that takes metrics.

module.exports = function(metrics) {
    router.get('/home', async (req, res) => {
        try {
            const data = await movieboxApi.getHome();
            res.json({ success: true, ...data });
        } catch (e) {
            res.status(500).json({ success: false, error: e.message });
        }
    });

    router.get('/discover', async (req, res) => {
        try {
            const { channelId } = req.query;
            let data;
            if (channelId) {
                data = await movieboxApi.filter({ channelId: parseInt(channelId) });
                res.json({ success: true, ...data });
            } else {
                data = await movieboxApi.getDiscover();
                res.json({ success: true, sections: data });
            }
        } catch (e) {
            res.status(500).json({ success: false, error: e.message });
        }
    });

    router.get('/section', async (req, res) => {
        try {
            const title = req.query.title;
            if (!title) return res.status(400).json({ success: false, error: 'title query required' });
            const data = await movieboxApi.getSection(title);
            res.json({ success: true, items: data });
        } catch (e) {
            res.status(500).json({ success: false, error: e.message });
        }
    });

    router.get('/trending', async (req, res) => {
        try {
            const data = await movieboxApi.getTrending();
            res.json({ success: true, items: data });
        } catch (e) {
            res.status(500).json({ success: false, error: e.message });
        }
    });

    router.get('/popular-movie', async (req, res) => {
        try {
            const data = await movieboxApi.getSection('Popular Movie');
            res.json({ success: true, items: data });
        } catch (e) {
            res.status(500).json({ success: false, error: e.message });
        }
    });

    router.get('/popular-series', async (req, res) => {
        try {
            const data = await movieboxApi.getSection('Popular Series');
            res.json({ success: true, items: data });
        } catch (e) {
            res.status(500).json({ success: false, error: e.message });
        }
    });

    router.get('/hot-shorts', async (req, res) => {
        try {
            const data = await movieboxApi.getSection('🔥Hot Short TV');
            res.json({ success: true, items: data });
        } catch (e) {
            res.status(500).json({ success: false, error: e.message });
        }
    });

    router.get('/anime', async (req, res) => {
        try {
            const data = await movieboxApi.getSection('Anime[English Dubbed]');
            res.json({ success: true, items: data });
        } catch (e) {
            res.status(500).json({ success: false, error: e.message });
        }
    });

    router.get('/sa', async (req, res) => {
        try {
            const data = await movieboxApi.getSection('SA Drama');
            res.json({ success: true, items: data });
        } catch (e) {
            res.status(500).json({ success: false, error: e.message });
        }
    });

    router.get('/thai', async (req, res) => {
        try {
            const data = await movieboxApi.getSection('Thai-Drama');
            res.json({ success: true, items: data });
        } catch (e) {
            res.status(500).json({ success: false, error: e.message });
        }
    });

    router.get('/turkish', async (req, res) => {
        try {
            const data = await movieboxApi.getSection('Turkish Drama');
            res.json({ success: true, items: data });
        } catch (e) {
            res.status(500).json({ success: false, error: e.message });
        }
    });

    router.get('/western', async (req, res) => {
        try {
            const data = await movieboxApi.getSection('Western TV');
            res.json({ success: true, items: data });
        } catch (e) {
            res.status(500).json({ success: false, error: e.message });
        }
    });

    router.get('/animation', async (req, res) => {
        try {
            const data = await movieboxApi.getSection('Animation Collection');
            res.json({ success: true, items: data });
        } catch (e) {
            res.status(500).json({ success: false, error: e.message });
        }
    });

    router.get('/black-shows', async (req, res) => {
        try {
            const data = await movieboxApi.getSection('Must-watch Black Shows');
            res.json({ success: true, items: data });
        } catch (e) {
            res.status(500).json({ success: false, error: e.message });
        }
    });

    router.get('/hero', async (req, res) => {
        try {
            // Specifically target curated Asian Drama rows from the homepage
            const [kdrama, cdrama] = await Promise.all([
                movieboxApi.getSection('K-Drama'),
                movieboxApi.getSection('C-Drama')
            ]);

            const mixed = [
                ...(kdrama || []).map(i => ({ ...i, fromMovieBox: true, region: 'K-Drama' })),
                ...(cdrama || []).map(i => ({ ...i, fromMovieBox: true, region: 'C-Drama' }))
            ];

            // Filter: Only keep actual live-action Dramas (Exclude Donghua/Anime/Animation)
            const filteredPool = mixed.filter(item => {
                const genre = (item.genre || '').toLowerCase();
                return !genre.includes('animation') && !genre.includes('anime');
            });

            // Sort by rating (highest first)
            const pool = filteredPool
                .sort((a, b) => {
                    const rA = parseFloat(a.rating) || 0;
                    const rB = parseFloat(b.rating) || 0;
                    return rB - rA;
                })
                .slice(0, 30); // Pool of top 30 potential hero candidates

            const enriched = [];
            // First pass: Find items with high-quality trailers
            for (const item of pool) {
                if (enriched.length >= 5) break;
                try {
                    const detail = await movieboxApi.getDetail(item.detailPath);
                    if (detail && detail.subject && detail.subject.trailer && detail.subject.trailer.videoAddress && detail.subject.trailer.videoAddress.url) {
                        enriched.push({
                            ...item,
                            description: detail.subject.description,
                            trailer: detail.subject.trailer,
                            stills: detail.subject.stills,
                            subjectType: detail.subject.subjectType
                        });
                    }
                } catch (e) {}
            }

            // Second pass: Fill with high-rated items if trailers are missing
            if (enriched.length < 5) {
                for (const item of pool) {
                    if (enriched.length >= 5) break;
                    if (enriched.find(e => e.id === item.id)) continue;
                    try {
                        const detail = await movieboxApi.getDetail(item.detailPath);
                        if (detail && detail.subject) {
                            enriched.push({
                                ...item,
                                description: detail.subject.description,
                                trailer: detail.subject.trailer,
                                stills: detail.subject.stills,
                                subjectType: detail.subject.subjectType
                            });
                        }
                    } catch (e) {}
                }
            }

            res.json({ success: true, items: enriched });
        } catch (e) {
            res.status(500).json({ success: false, error: e.message });
        }
    });

    router.get('/bollywood', async (req, res) => {
        try {
            const { type, page } = req.query;
            const data = await movieboxApi.getIndianContent(type || 'movie', parseInt(page) || 1);
            res.json({ success: true, ...data });
        } catch (e) {
            res.status(500).json({ success: false, error: e.message });
        }
    });

    router.get('/shorts', async (req, res) => {
        try {
            const { page } = req.query;
            const data = await movieboxApi.getShorts(parseInt(page) || 1);
            res.json({ success: true, ...data });
        } catch (e) {
            res.status(500).json({ success: false, error: e.message });
        }
    });

    router.get('/filter', async (req, res) => {
        try {
            const { page, perPage, channelId, genre, country, year, sort } = req.query;
            const data = await movieboxApi.filter({
                page: parseInt(page) || 1,
                perPage: parseInt(perPage) || 28,
                channelId: parseInt(channelId) || 2,
                genre, country, year, sort
            });
            res.json({ success: true, ...data });
        } catch (e) {
            res.status(500).json({ success: false, error: e.message });
        }
    });

    router.get('/info', async (req, res) => {
        try {
            const path = req.query.path;
            if (!path) return res.status(400).json({ success: false, error: 'path query required' });
            const data = await movieboxApi.getDetail(path);
            res.json({ success: true, data });
        } catch (e) {
            res.status(500).json({ success: false, error: e.message });
        }
    });

    router.get('/search', async (req, res) => {
        try {
            const { keyword, page, type } = req.query;
            if (!keyword) return res.status(400).json({ success: false, error: 'keyword query required' });
            const data = await movieboxApi.search(keyword, parseInt(page) || 1, parseInt(type) || 0);
            res.json({ success: true, ...data });
        } catch (e) {
            res.status(500).json({ success: false, error: e.message });
        }
    });

    router.get('/play/:id/*', async (req, res) => {
        const id = req.params.id;
        const path = req.params[0]; // Captures everything after :id/
        let season = req.query.season !== undefined ? parseInt(req.query.season) : null;
        let episode = req.query.episode !== undefined ? parseInt(req.query.episode) : null;
        
        try {
            if (metrics) metrics.streamRequests++;
            
            // If season or episode missing, fetch detail to determine type
            if (season === null || episode === null) {
                console.log(`[api] MovieBox play: Missing season/episode for ${path}, fetching details...`);
                const detail = await movieboxApi.getDetail(path);
                if (detail && detail.subject) {
                    if (detail.subject.subjectType === 1) { // Movie
                        season = 0;
                        episode = 0;
                    } else {
                        // For series, default to S1E1 if not provided
                        season = season || 1;
                        episode = episode || 1;
                    }
                } else {
                    return res.status(404).json({ success: false, error: 'Content not found' });
                }
            }

            const data = await movieboxApi.getStreams(id, path, season, episode);
            if (!data) return res.status(404).json({ success: false, error: 'Streams not found' });

            const streams = [];
            
            // Extract subtitles
            let subtitles = [];
            const firstStream = data.streams && data.streams.length > 0 ? data.streams[0] : (data.hls && data.hls.length > 0 ? data.hls[0] : null);
            
            if (firstStream) {
                try {
                    const captions = await movieboxApi.getCaptions(id, path, firstStream.id || '');
                    subtitles = (captions || []).map(c => ({
                        url: c.url,
                        lang: c.lan || 'en',
                        label: c.lanName || 'English'
                    }));
                } catch (capErr) {
                    console.warn(`[api] Caption fetch failed:`, capErr.message);
                }
            }

            // Add headers for proxying
            const isMovie = (season === 0 && episode === 0);
            const typeFolder = isMovie ? 'movies' : 'series';
            const typeLabel = isMovie ? 'movie' : 'series';
            
            let refererPath = path.startsWith('/') ? path.substring(1) : path;
            refererPath = refererPath.replace('movies/detail/', 'movies/').replace('series/detail/', 'series/');
            const streamReferer = `https://movibox.net/spa/videoPlayPage/${refererPath}?id=${id}&type=/${typeLabel}/detail&detailSe=&detailEp=&lang=en`;
            
            const mbHeaders = movieboxApi.getStreamHeaders(streamReferer);

            if (data.streams && Array.isArray(data.streams)) {
                data.streams.forEach(s => {
                    streams.push({
                        title: `MovieBox - ${s.resolutions}p (MP4)`,
                        url: s.url,
                        quality: `${s.resolutions}p`,
                        format: 'mp4',
                        subtitles: subtitles.length > 0 ? subtitles : undefined,
                        headers: mbHeaders
                    });
                });
            }

            if (data.hls && Array.isArray(data.hls)) {
                data.hls.forEach(s => {
                    streams.push({
                        title: `MovieBox - Auto (HLS)`,
                        url: s.url,
                        quality: 'Auto',
                        format: 'hls',
                        subtitles: subtitles.length > 0 ? subtitles : undefined,
                        headers: mbHeaders
                    });
                });
            }

            let finalStreams = streams;
            if (metrics) metrics.streamsReturned += finalStreams.length;

            if (config.enableProxy) {
                const serverUrl = resolveServerUrl(req);
                finalStreams = processStreamsForProxy(finalStreams, serverUrl);
            }

            res.json({
                success: true,
                id,
                path,
                count: finalStreams.length,
                streams: finalStreams
            });
        } catch (e) {
            if (metrics) metrics.lastError = e.message;
            res.status(500).json({ success: false, error: e.message });
        }
    });

    return router;
};
