/**
 * Hue Stream API
 * Professional Streaming Aggregator
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */

// Polyfill: undici 7.x requires global File (Node.js v20+). We run v18.
if (typeof global.File === 'undefined') {
  global.File = class File {
    constructor(bits, name, options = {}) {
      this.name = name;
      this.size = 0;
      this.type = options.type || '';
      this.lastModified = options.lastModified || Date.now();
    }
    text() { return Promise.resolve(''); }
    arrayBuffer() { return Promise.resolve(new ArrayBuffer(0)); }
    stream() { return new ReadableStream(); }
    slice() { return new Blob([]); }
  };
}

const express = require('express');
const sflixProvider = require('./providers/sflix');
const kissasianProvider = require('./providers/kissasian');
const { decrypt: videasyDecrypt } = require('./providers/videasy/decrypt');
const { getNewPage } = require('./utils/browserManager');
const vidsrcEmbedProvider = require('./providers/vidsrc-embed');
const iframeExtractor = require('./providers/iframeExtractor');

const app = express();
const port = process.env.SCRAPER_PORT || 24610;

app.use(express.json());

app.get('/health', (req, res) => {
    res.json({ status: 'ok', service: 'Playwright Scraper Microservice' });
});

// SFlix Fallback Scraper Route
app.post('/scrape/sflix', async (req, res) => {
    const { url, isMovie, season, episode } = req.body;
    if (!url) return res.status(400).json({ error: 'Missing url parameter' });
    
    try {
        console.log(`[Scraper] Scraping SFlix: ${url}`);
        const streams = await sflixProvider.extractFromUrlWithPlaywright(url, isMovie, season, episode);
        res.json({ success: true, streams: streams || [] });
    } catch (e) {
        console.error(`[Scraper] SFlix error: ${e.message}`);
        res.status(500).json({ success: false, error: e.message });
    }
});

// Videasy Decryption Route
app.post('/scrape/videasy', async (req, res) => {
    const { hexResponse, tmdbId } = req.body;
    if (!hexResponse || !tmdbId) {
        return res.status(400).json({ error: 'Missing hexResponse or tmdbId parameter' });
    }

    try {
        console.log(`[Scraper] Decrypting Videasy for TMDB ${tmdbId}`);
        const decrypted = await videasyDecrypt(hexResponse.trim(), String(tmdbId));

        if (!decrypted || !decrypted.sources || !Array.isArray(decrypted.sources)) {
            return res.json({ success: false, streams: [], subtitles: [] });
        }

        const streams = decrypted.sources
            .filter(s => s.url)
            .map(s => ({
                url: s.url,
                quality: s.quality || 'Auto'
            }));

        const subtitles = (decrypted.subtitles || [])
            .filter(s => s.url)
            .map(s => ({
                url: s.url,
                lang: s.lang || 'en',
                label: s.language || s.lang || 'English'
            }));

        console.log(`[Scraper] Videasy decrypted ${streams.length} streams, ${subtitles.length} subtitles`);
        res.json({ success: true, streams, subtitles });
    } catch (e) {
        console.error(`[Scraper] Videasy error: ${e.message}`);
        res.status(500).json({ success: false, error: e.message, streams: [], subtitles: [] });
    }
});

// KissAsian Fallback Scraper Route
app.post('/scrape/kissasian', async (req, res) => {
    const { url, title, season, episode } = req.body;

    try {
        if (url) {
            console.log(`[Scraper] Scraping KissAsian URL: ${url}`);
            const streams = await kissasianProvider.getStreamsByUrl(url);
            res.json({ success: true, streams: streams || [] });
        } else if (title) {
            console.log(`[Scraper] Scraping KissAsian: ${title} S${season || 1}E${episode || 1}`);
            const streams = await kissasianProvider.getStreams(null, 'tv', season, episode);
            res.json({ success: true, streams: streams || [] });
        } else {
            res.status(400).json({ error: 'Missing url or title parameter' });
        }
    } catch (e) {
        console.error(`[Scraper] KissAsian error: ${e.message}`);
        res.status(500).json({ success: false, error: e.message });
    }
});

// Vidsrc-Embed Browser Scraper Route
app.post('/scrape/vidsrc-embed', async (req, res) => {
    const { tmdbId, mediaType, season, episode } = req.body;
    if (!tmdbId || !mediaType) {
        return res.status(400).json({ error: 'Missing tmdbId or mediaType parameter' });
    }

    try {
        console.log(`[Scraper] Scraping Vidsrc-Embed: ${mediaType} ${tmdbId}`);
        const streams = await vidsrcEmbedProvider.getStreams(tmdbId, mediaType, season, episode);
        if (streams && streams.length > 0) {
            const mapped = streams.map(s => ({
                url: s.url,
                quality: s.quality || 'Auto',
                subtitles: s.subtitles || [],
                headers: s.headers || {}
            }));
            res.json({ success: true, streams: mapped });
        } else {
            res.json({ success: false, streams: [] });
        }
    } catch (e) {
        console.error(`[Scraper] Vidsrc-Embed error: ${e.message}`);
        res.status(500).json({ success: false, error: e.message, streams: [] });
    }
});

// Iframe-to-native stream extraction route
app.post('/extract/iframe', async (req, res) => {
    const { url, referer } = req.body;
    if (!url) {
        return res.status(400).json({ success: false, error: 'Missing url parameter' });
    }

    try {
        console.log(`[Scraper] Extracting iframe: ${url}`);
        const result = await iframeExtractor.extractIframe(url, referer);
        if (result) {
            res.json({ success: true, ...result });
        } else {
            res.json({ success: false, error: 'Extraction returned no stream' });
        }
    } catch (e) {
        console.error(`[Scraper] iframe extraction error: ${e.message}`);
        res.status(500).json({ success: false, error: e.message });
    }
});

app.listen(port, '127.0.0.1', () => {
    console.log(`Scraper Microservice listening on http://127.0.0.1:${port}`);
});
