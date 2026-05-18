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
const dramacoolProvider = require('./providers/dramacool');

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

// Dramacool Fallback Scraper Route
app.post('/scrape/dramacool', async (req, res) => {
    const { url } = req.body;
    if (!url) return res.status(400).json({ error: 'Missing url parameter' });
    
    try {
        console.log(`[Scraper] Scraping Dramacool: ${url}`);
        const streams = await dramacoolProvider.getStreamsByUrl(url);
        res.json({ success: true, streams: streams || [] });
    } catch (e) {
        console.error(`[Scraper] Dramacool error: ${e.message}`);
        res.status(500).json({ success: false, error: e.message });
    }
});

app.listen(port, '127.0.0.1', () => {
    console.log(`Scraper Microservice listening on http://127.0.0.1:${port}`);
});
