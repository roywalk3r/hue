/**
 * Hue Stream API
 * Professional Streaming Aggregator
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */

require('dotenv').config();
const express = require('express');
const cors = require('cors');
const compression = require('compression');
const path = require('path');
const os = require('os');
const { config, saveConfigPatch, OVERRIDE_PATH } = require('./utils/config');
const { authenticate, issueSession, requireAuth, getSession, updatePassword } = require('./utils/auth');
const logger = require('./utils/logger');
const stealth = require('./utils/stealth');
const { resolveServerUrl } = require('./utils/server');
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

// --- Compression (Brotli preferred, gzip fallback) ---
app.use(compression({
  filter: (req, res) => {
    if (req.headers['x-no-compression']) return false;
    return compression.filter(req, res);
  },
  level: 6,
  chunkSize: 16 * 1024,
}));

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
// All API routes are now handled by the Rust engine
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

// Scalar modern API reference UI
const { apiReference } = require('@scalar/express-api-reference');
app.use('/api/docs/scalar', apiReference({
    spec: { url: '/api/docs/openapi.json' },
    theme: 'default',
    hideDownloadButton: true,
    favicon: '',
    pageTitle: 'Hue Stream API Docs',
}));

app.get('/api/docs', (req, res) => {
  res.redirect(302, '/api/docs/scalar');
});

// --- Error Handling ---
app.use((req, res) => {
    logger.warn(`[404] Not Found: ${req.method} ${req.url}`);
    res.status(404).json({ success: false, error: 'NOT_FOUND', path: req.url });
});

// For Vercel deployment: export the app
module.exports = app;

// Server Start
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
