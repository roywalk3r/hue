require('dotenv').config();
const express = require('express');
const internalRoutes = require('./routes/internal');

const app = express();
app.use(express.json());

const PORT = process.env.SCRAPER_PORT || 24610;

// Only internal routes — no public endpoints
app.use('/internal', internalRoutes);

// Health check at root for Docker
app.get('/health', (req, res) => res.json({ ok: true }));

app.listen(PORT, '127.0.0.1', () => {
  console.log(`[scraper] Node scraper service listening on 127.0.0.1:${PORT}`);
});

// Start cookie rotation (best-effort — won't block startup if browser is unavailable)
try {
  const { startCookieWorker } = require('./services/cookieWorker');
  startCookieWorker();
} catch (e) {
  console.warn('[scraper] Cookie worker failed to start:', e.message);
}
