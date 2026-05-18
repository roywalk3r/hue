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

require('dotenv').config();
const express = require('express');
const cors = require('cors');
const path = require('path');
const os = require('os');
const { config, saveConfigPatch, OVERRIDE_PATH } = require('./utils/config');
const { authenticate, issueSession, requireAuth, getSession, updatePassword } = require('./utils/auth');
const logger = require('./utils/logger');
const stealth = require('./utils/stealth');
const { resolveServerUrl } = require('./utils/server');
const { createProxyRoutes, processStreamsForProxy } = require('./proxy/proxyServer');
const TTLCache = require('ttl-cache');

// Auto-generate key if missing
if (!process.env.APP_SECRET) {
  try { require('./scripts/generate-key'); } catch(e) {}
}

// Polyfill File for libraries that expect it
if (typeof File === 'undefined') {
  global.File = class { constructor() { throw new Error("File polyfill: not implemented"); } };
}

// Register secrets in logger
if (config.tmdbApiKeys) config.tmdbApiKeys.forEach(k => logger.registerSecret(k));
if (process.env.TMDB_KEY) logger.registerSecret(process.env.TMDB_KEY);

const app = express();

// --- CORS & Body Parsing ---
const allowedOrigins = process.env.ALLOWED_ORIGINS ? process.env.ALLOWED_ORIGINS.split(',').map(o => o.trim()) : [];
const corsOptions = {
    origin: function (origin, callback) {
        if (!origin || allowedOrigins.length === 0 || allowedOrigins.includes(origin)) {
            callback(null, true);
        } else {
            console.warn(`[CORS] Blocked: ${origin}`);
            callback(new Error('Not allowed by CORS'));
        }
    }
};

app.use(cors(corsOptions));
app.options('*', cors(corsOptions));
app.use(express.json());

// --- Metrics & Logger ---
const metrics = {
  startTime: Date.now(),
  requestsTotal: 0,
  streamRequests: 0,
  providerCalls: {},
  lastRequestAt: null,
  lastError: null,
  streamsReturned: 0,
  tmdbToImdbLookups: 0,
  uptime: Date.now()
};

function getClientIp(req){
  return (req.headers['x-forwarded-for'] || req.connection.remoteAddress || '').split(',')[0].trim();
}

app.use((req, res, next) => {
    const ip = getClientIp(req);
    metrics.requestsTotal++;
    metrics.lastRequestAt = Date.now();
    logger.info(`[request] ${req.method} ${req.url} from ${stealth.anonymize(ip)}`);
    next();
});

// --- Modular Routes ---
const hitvRoutes = require('./routes/hitv');
const movieboxRoutes = require('./routes/moviebox');
const streamsRoutes = require('./routes/streams');
const sportsRoutes = require('./routes/sports');
const dramaRoutes = require('./routes/drama');
const systemRoutes = require('./routes/system');
const proxyRoutes = require('./routes/proxy');
const livetvRoutes = require('./routes/livetv');
const playRoutes = require('./routes/play');
const tmdbRoutes = require('./routes/tmdb');
const animeRoutes = require('./routes/anime');

// Mount API routes
app.use('/api/tmdb', tmdbRoutes(metrics));
app.use('/api/anime', animeRoutes(metrics));
app.use('/api/hitv', hitvRoutes(metrics));
app.use('/api/sflix', hitvRoutes(metrics)); // Alias for Sflix Discovery
app.use('/api/moviebox', movieboxRoutes(metrics));
app.use('/api/streams', streamsRoutes(metrics));
app.use('/api/play', playRoutes(metrics));
app.use('/api', sportsRoutes(metrics)); 
app.use('/api', dramaRoutes(metrics)); 
app.use('/api', systemRoutes(metrics));
app.use('/api/proxy', proxyRoutes);
app.use('/api/livetv', livetvRoutes);

// Mount Legacy/Root routes
app.use('/', sportsRoutes(metrics)); // /sports-stream, /livetv legacy

// --- Proxy Configuration ---
if (config.enableProxy) {
  logger.info('[startup] enableProxy flag active: mounting proxy routes');
  createProxyRoutes(app); // Registers /m3u8-proxy, /ts-proxy, /sub-proxy at root
}
app.post('/auth/login', (req,res) => {
  const { username, password } = req.body || {};
  if (!authenticate(username, password)) return res.status(401).json({ success:false, error:'INVALID_CREDENTIALS' });
  const token = issueSession(username);
  res.setHeader('Set-Cookie', `session=${token}; HttpOnly; SameSite=Lax; Path=/; Max-Age=${12*60*60}`);
  res.json({ success:true, username });
});

// --- Static UI ---
app.get('/config.html', (req,res) => {
  if (!getSession(req)) return res.redirect(302, '/');
  res.sendFile(path.join(process.cwd(),'public','config.html'));
});

app.use(express.static(path.join(process.cwd(),'public')));

// Root login page
app.get('/', (req,res) => res.sendFile(path.join(process.cwd(),'public','index.html')));

// --- Documentation ---
const swaggerUi = require('swagger-ui-express');
const buildOpenApiSpec = require('./utils/openapi-spec');

app.get('/api/docs/openapi.json', (req, res) => {
  res.setHeader('Cache-Control', 'no-store');
  res.json(buildOpenApiSpec());
});

app.get('/api/docs/swagger.json', (req, res) => {
  res.setHeader('Cache-Control', 'no-store');
  res.json(buildOpenApiSpec());
});

app.use('/api/docs/swagger', swaggerUi.serve, swaggerUi.setup(null, { swaggerOptions: { url: '/api/docs/swagger.json' } }));

// Scalar modern API reference UI + Server Start
(async () => {
  try {
    const { apiReference } = await import('@scalar/express-api-reference');
    app.use('/api/docs/scalar', apiReference({
        spec: { url: '/api/docs/openapi.json' },
        theme: 'none',
        hideDownloadButton: true,
        favicon: '',
        pageTitle: 'HUE ENGINE ⬢ API REFERENCE',
        customCss: `
:root {
  --scalar-color-1: #E0E0E0;
  --scalar-color-2: #aaa;
  --scalar-color-3: #777;
  --scalar-color-accent: #1A5E3A;
  --scalar-background-1: #0A0A0A;
  --scalar-background-2: #121212;
  --scalar-background-3: #1a1a1a;
  --scalar-border-color: #2A2A2A;
  --scalar-font: 'JetBrains Mono', 'Fira Code', monospace;
}
.scalar-card { border: 1px solid var(--scalar-border-color) !important; }
.sidebar { background: var(--scalar-background-2) !important; border-right: 1px solid var(--scalar-border-color) !important; }
      `
    }));
  } catch (e) {
    console.warn('[startup] Scalar API docs unavailable:', e.message);
  }

  app.get('/api/docs', (req, res) => {
    res.redirect(302, '/api/docs/scalar');
  });

  // --- Error Handling ---
  app.use((req, res) => {
      logger.warn(`[404] Not Found: ${req.method} ${req.url}`);
      res.status(404).json({ success: false, error: 'NOT_FOUND', path: req.url });
  });

  // Server Start (after all middleware is registered)
  if (process.env.NODE_ENV !== 'production' || !process.env.VERCEL) {
    const PORT = config.port;
    app.listen(PORT, '0.0.0.0', () => {
      console.log(`Hue Stream API listening on http://0.0.0.0:${PORT}`);

      // Start background token worker to keep "Fast Paths" warm
      try {
        const { startTokenWorker } = require('./services/tokenWorker');
        startTokenWorker();

        const { startWarmupWorker } = require('./services/warmupWorker');
        startWarmupWorker();
      } catch (e) {
        console.error('[startup] Failed to start background workers:', e.message);
      }
    });
  }
})();

// For Vercel deployment: export the app immediately
module.exports = app;
