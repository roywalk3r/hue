/**
 * API Server for Livesport Scraper
 * Deploy to: Vercel, Netlify Functions, or any Node.js host
 */

const express = require('express');
const cors = require('cors');
const LivesportWatchScraper = require('../livesport-watch-scraper');

const app = express();
const PORT = process.env.PORT || 3000;

// Middleware
app.use(cors());
app.use(express.json());

// Initialize scraper
const scraper = new LivesportWatchScraper();

// API endpoint to scrape streams
app.post('/api/scrape', async (req, res) => {
    try {
        const { url } = req.body;
        
        if (!url) {
            return res.status(400).json({ error: 'URL is required' });
        }
        
        if (!url.includes('livesport.su/watch/')) {
            return res.status(400).json({ error: 'Invalid Livesport URL' });
        }
        
        console.log(`Scraping: ${url}`);
        const results = await scraper.scrapeWatchUrl(url);
        
        if (!results) {
            return res.status(500).json({ error: 'Failed to scrape streams' });
        }
        
        res.json(results);
        
    } catch (error) {
        console.error('Scrape error:', error);
        res.status(500).json({ error: error.message });
    }
});

// Health check
app.get('/api/health', (req, res) => {
    res.json({ status: 'ok', timestamp: new Date().toISOString() });
});

// Serve static files
app.use(express.static('public'));

// For Vercel deployment
module.exports = app;

// For local development
if (require.main === module) {
    app.listen(PORT, () => {
        console.log(`🚀 Livesport API Server running on port ${PORT}`);
        console.log(`📺 Open: http://localhost:${PORT}`);
    });
}
