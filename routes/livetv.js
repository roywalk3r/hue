const express = require('express');
const router = express.Router();
const scraper = require('../services/livetvScraper');

// GET /api/livetv
router.get('/', async (req, res) => {
    try {
        const { sportId } = req.query;
        const sports = await scraper.getSports();
        const matches = await scraper.getMatches(sportId);
        res.json({ sports, matches });
    } catch (error) {
        res.status(500).json({ error: error.message });
    }
});

// GET /api/livetv/sports
router.get('/sports', async (req, res) => {
    try {
        const sports = await scraper.getSports();
        res.json({ sports });
    } catch (error) {
        res.status(500).json({ error: error.message });
    }
});

// GET /api/livetv/matches
router.get('/matches', async (req, res) => {
    try {
        const { sportId } = req.query;
        const matches = await scraper.getMatches(sportId);
        res.json({ matches });
    } catch (error) {
        res.status(500).json({ error: error.message });
    }
});

// GET /api/livetv/match
router.get('/match', async (req, res) => {
    try {
        const { url, resolve, stream } = req.query;
        if (!url) return res.status(400).json({ error: 'Missing url parameter' });

        const variants = await scraper.getStreamVariants(url);

        // Streaming Mode (SSE)
        if (stream === 'true') {
            res.setHeader('Content-Type', 'text/event-stream');
            res.setHeader('Cache-Control', 'no-cache');
            res.setHeader('Connection', 'keep-alive');
            res.flushHeaders();

            for (const variant of variants) {
                // If the client disconnected, stop processing
                if (res.writableEnded) break;

                const resolved = await scraper.resolveStream(variant.url);
                
                if (resolved.error) {
                    res.write(`data: ${JSON.stringify({ ...variant, error: resolved.error })}\n\n`);
                } else if (resolved.streams && resolved.streams.length > 0) {
                    for (const s of resolved.streams) {
                        const result = { ...variant };
                        result.url = s.url;
                        result.headers = s.headers;
                        if (s.quality !== 'Auto') result.quality = s.quality;
                        result.title = `${result.title} (${s.quality})`;

                        if (result.url && result.url.startsWith('http')) {
                            const host = req.get('host') || 'localhost:3000';
                            const protocol = req.protocol;
                            const proxyHeaders = result.headers || { 'Referer': variant.url };
                            result.proxyUrl = `${protocol}://${host}/api/proxy?url=${encodeURIComponent(result.url)}&headers=${encodeURIComponent(JSON.stringify(proxyHeaders))}`;
                        }
                        
                        res.write(`data: ${JSON.stringify(result)}\n\n`);
                    }
                }
            }
            res.end();
            return;
        }

        // Standard JSON Mode
        const resolvedStreams = [];

        for (const variant of variants) {
            // If resolution is not requested, just add the variant info
            if (resolve !== 'true') {
                resolvedStreams.push(variant);
                continue;
            }

            const resolved = await scraper.resolveStream(variant.url);
            
            if (resolved.error) {
                resolvedStreams.push({ ...variant, error: resolved.error });
            } else if (resolved.streams && resolved.streams.length > 0) {
                // We found multiple streams (e.g. qualities) for this variant
                for (const stream of resolved.streams) {
                    const result = { ...variant }; // Copy base variant info (language, etc)
                    
                    // Overwrite with actual stream data
                    result.url = stream.url;
                    result.headers = stream.headers;
                    // Append quality if found by resolver
                    if (stream.quality !== 'Auto') {
                        result.quality = stream.quality; 
                    }
                    result.title = `${result.title} (${stream.quality})`; // Differentiate titles

                    // Generate proxy URL
                    if (result.url && result.url.startsWith('http')) {
                        const host = req.get('host') || 'localhost:3000';
                        const protocol = req.protocol;
                        const proxyHeaders = result.headers || { 'Referer': variant.url };
                        result.proxyUrl = `${protocol}://${host}/api/proxy?url=${encodeURIComponent(result.url)}&headers=${encodeURIComponent(JSON.stringify(proxyHeaders))}`;
                    }
                    
                    resolvedStreams.push(result);
                }
            }
        }

        res.json({ streams: resolvedStreams });
    } catch (error) {
        // If headers are already sent (streaming started but failed), we can't send 500 JSON
        if (res.headersSent) {
            res.write(`event: error\ndata: ${JSON.stringify({ error: error.message })}\n\n`);
            res.end();
        } else {
            res.status(500).json({ error: error.message });
        }
    }
});

module.exports = router;