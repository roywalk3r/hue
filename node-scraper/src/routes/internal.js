const express = require('express');
const router = express.Router();

const startTime = Date.now();

// Provider registry — maps provider names to their scrape functions
const providerScrapers = {
  showbox: require('../providers/showbox').getStreamsFromTmdbId,
  'showbox-playwright': require('../providers/showbox-playwright').getStreamsFromTmdbId,
  mp4hydra: require('../providers/mp4hydra').getMP4HydraStreams,
  vidzee: require('../providers/vidzee').getVidZeeStreams,
  'vidsrccc-playwright': require('../providers/vidsrccc-playwright').getVidsrcStreams,
  moviesmod: require('../providers/moviesmod').getMoviesModStreams,
  uhdmovies: require('../providers/uhdmovies').getUHDMoviesStreams,
  '4khdhub': require('../providers/4khdhub').get4KHDHubStreams,
};

const ACTIVE_PROVIDERS = Object.keys(providerScrapers);

// GET /internal/health
router.get('/health', (req, res) => {
  res.json({
    ok: true,
    providers: ACTIVE_PROVIDERS,
    uptime: Date.now() - startTime,
  });
});

const { getCookies } = require('../services/cookieWorker');

// GET /internal/cookies/:provider
router.get('/cookies/:provider', async (req, res) => {
  const { provider } = req.params;
  const entry = getCookies(provider);
  if (!entry) {
    return res.status(503).json({
      error: 'Cookies not available',
      provider,
    });
  }
  res.json({ provider, cookies: entry.cookies, expires: entry.expires });
});

// POST /internal/scrape/browser
router.post('/scrape/browser', async (req, res) => {
  const { provider, tmdbId, type, season, episode } = req.body;

  if (!provider || !tmdbId || !type) {
    return res.status(400).json({ success: false, error: 'Missing required fields: provider, tmdbId, type' });
  }

  const scraper = providerScrapers[provider];
  if (!scraper) {
    return res.status(400).json({ success: false, error: `Unknown provider: ${provider}` });
  }

  const mediaType = type === 'movie' ? 'movie' : 'tv';
  try {
    const result = await scraper(mediaType, tmdbId, season || null, episode || null);
    const streams = Array.isArray(result) ? result : [];
    res.json({ success: true, streams });
  } catch (e) {
    console.error(`[internal] ${provider} scrape failed:`, e.message);
    res.json({ success: true, streams: [] });
  }
});

const { getAnimeStream } = require('../providers/hianime');

// POST /internal/scrape/anime
router.post('/scrape/anime', async (req, res) => {
  const { slug, episode } = req.body;

  if (!slug || !episode) {
    return res.status(400).json({ success: false, error: 'Missing required fields: slug, episode' });
  }

  try {
    const data = await getAnimeStream(slug, episode);
    if (!data || !data.stream) {
      return res.json({ success: false, error: 'No stream found' });
    }
    res.json({
      success: true,
      stream: {
        url: data.stream.link?.file || data.stream.sources?.[0]?.url,
        server: data.stream.server || 'unknown',
        tracks: data.stream.tracks || [],
      },
    });
  } catch (e) {
    console.error(`[internal] Anime scrape failed:`, e.message);
    res.json({ success: false, error: e.message });
  }
});

module.exports = router;
