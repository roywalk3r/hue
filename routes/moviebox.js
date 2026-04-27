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

    /*
    router.get('/anime', async (req, res) => {
        try {
            const data = await movieboxApi.getSection('Anime[English Dubbed]');
            res.json({ success: true, items: data });
        } catch (e) {
            res.status(500).json({ success: false, error: e.message });
        }
    });
    */

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
            const { getPopular } = require('../utils/tmdb');

            // 1. Fetch from all sources (MovieBox + TMDB)
            const [trending, kdrama, cdrama, tmdbMovies, tmdbTv] = await Promise.all([
                movieboxApi.getTrending(1),
                movieboxApi.getSection('K-Drama'),
                movieboxApi.getSection('C-Drama'),
                getPopular('movie', 1),
                getPopular('tv', 1)
            ]);

            // 2. Prepare TMDB candidates (Find them on MovieBox first)
            const tmdbCandidates = [...(tmdbMovies || []), ...(tmdbTv || [])].slice(0, 10);
            const tmdbMatched = await Promise.all(tmdbCandidates.map(async (t) => {
                const searchResults = await movieboxApi.search(t.title || t.name, 1, t.title ? 1 : 2);
                return searchResults.items?.[0] || null;
            }));

            // 3. Combine everything into a master pool
            const rawPool = [
                ...tmdbMatched.filter(i => i !== null),
                ...(kdrama || []),
                ...(cdrama || []),
                ...(trending || [])
            ];

            // 4. Strict filtering and deduplication
            const filteredPool = rawPool.filter((item, index, self) => 
                item && item.id && index === self.findIndex((t) => t.id === item.id)
            ).filter(item => {
                const title = (item.title || '').toLowerCase();
                // Kill Western cable TV leakage
                const isWesternLeak = ['sistas', 'divorced', '202', 'all the queen', 'tyler perry'].some(word => title.includes(word));
                return !isWesternLeak;
            }).sort((a, b) => (parseFloat(b.rating) || 0) - (parseFloat(a.rating) || 0))
              .slice(0, 20);

            // 5. Parallel Enrichment
            const enrichmentPromises = filteredPool.map(async (item) => {
                try {
                    const detail = await movieboxApi.getDetail(item.detailPath);
                    if (detail && detail.subject && detail.subject.description) {
                        return {
                            ...item,
                            description: detail.subject.description,
                            trailer: detail.subject.trailer,
                            stills: detail.subject.stills,
                            subjectType: detail.subject.subjectType,
                            isTMDB: !!tmdbCandidates.find(t => (t.title || t.name) === item.title)
                        };
                    }
                } catch (e) {}
                return null;
            });

            const results = await Promise.all(enrichmentPromises);
            const enriched = results.filter(r => r !== null).slice(0, 12);

            res.json({
                success: true,
                count: enriched.length,
                items: enriched
            });
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
