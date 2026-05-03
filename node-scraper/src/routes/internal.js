const express = require('express');
const router = express.Router();

const startTime = Date.now();

// GET /internal/health
router.get('/health', (req, res) => {
  const providers = ['showbox', 'mp4hydra', 'vidzee', 'vidsrccc-playwright', 'moviesmod', 'uhdmovies', '4khdhub', 'hianime'];
  res.json({
    ok: true,
    providers,
    uptime: Date.now() - startTime,
  });
});

// GET /internal/cookies/:provider — stub (Task 5)
router.get('/cookies/:provider', async (req, res) => {
  res.status(503).json({ error: 'Not yet implemented', provider: req.params.provider });
});

// POST /internal/scrape/browser — stub (Task 4)
router.post('/scrape/browser', async (req, res) => {
  res.status(503).json({ error: 'Not yet implemented' });
});

// POST /internal/scrape/anime — stub (Task 6)
router.post('/scrape/anime', async (req, res) => {
  res.status(503).json({ error: 'Not yet implemented' });
});

module.exports = router;
