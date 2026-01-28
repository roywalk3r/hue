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
// Auto-generate key if missing
if (!process.env.APP_SECRET) {
  try { require('./scripts/generate-key'); } catch(e) {}
}
// Polyfill File for environments/libraries that expect it
if (typeof File === 'undefined') {
  global.File = class { constructor() { throw new Error("File polyfill: not implemented"); } };
}
const cors = require('cors');
const os = require('os');
const { config, saveConfigPatch, OVERRIDE_PATH } = require('./utils/config');
const { authenticate, issueSession, requireAuth, getSession, updatePassword } = require('./utils/auth');

// CORS Configuration
const allowedOrigins = process.env.ALLOWED_ORIGINS ? process.env.ALLOWED_ORIGINS.split(',').map(o => o.trim()) : [];

const corsOptions = {
    origin: function (origin, callback) {
        // Allow requests with no origin (like mobile apps or curl requests)
        if (!origin) return callback(null, true);
        
        // If no specific origins are defined, allow ALL (default dev behavior)
        if (allowedOrigins.length === 0) {
            return callback(null, true);
        }

        if (allowedOrigins.includes(origin)) {
            callback(null, true);
        } else {
            console.warn(`[CORS] Blocked request from origin: ${origin}`);
            callback(new Error('Not allowed by CORS'));
        }
    }
};

const app = express();
app.use(cors(corsOptions));
// Also apply CORS explicitly to options preflight
app.options('*', cors(corsOptions));

const path = require('path');
const { listProviders, getProvider, getCookieStats } = require('./providers/registry');
const { createProxyRoutes, processStreamsForProxy } = require('./proxy/proxyServer');
const { resolveImdbId } = require('./utils/tmdb');
const { applyFilters } = require('./utils/streamFilters');
const { getCachedStreams, setCachedStreams, addCachedStreams, clearCache } = require('./utils/cache');
const kisskhApi = require('./utils/kisskh-api');
const kisskhProvider = require('./providers/kisskh');
const dramacoolApi = require('./utils/dramacool-api');
const dramacoolProvider = require('./providers/dramacool');
const livetvRoutes = require('./routes/livetv');
const proxyRoutes = require('./routes/proxy');
const logger = require('./utils/logger');
const stealth = require('./utils/stealth');

// Register secrets
if (config.tmdbApiKeys) config.tmdbApiKeys.forEach(k => logger.registerSecret(k));
if (process.env.TMDB_KEY) logger.registerSecret(process.env.TMDB_KEY);

const swaggerUi = require('swagger-ui-express');
const redoc = require('redoc-express');
const swaggerDef = require('./utils/swagger-def');

// Serve the raw OpenAPI spec for Redoc to consume
app.get('/api/docs/swagger.json', (req, res) => res.json(swaggerDef));

// Scalar API Reference (Modern Documentation)
app.get('/api/docs', (req, res) => {
  res.send(`
    <!doctype html>
    <html>
      <head>
        <title>Hue Stream API Documentation</title>
        <meta charset="utf-8" />
        <meta name="viewport" content="width=device-width, initial-scale=1" />
        <style>body { margin: 0; }</style>
      </head>
      <body>
        <script
          id="api-reference"
          data-url="/api/docs/swagger.json"
          data-configuration='{"theme": "elysiajs"}'></script>
        <script src="https://cdn.jsdelivr.net/npm/@scalar/api-reference"></script>
      </body>
    </html>
  `);
});

// Mount Swagger UI as an alternative
app.use('/api/docs/swagger', swaggerUi.serve, swaggerUi.setup(swaggerDef));

function resolveServerUrl(req) {
  const host = req.get('host') || 'localhost';
  const forwardedProto = (req.headers['x-forwarded-proto'] || '').split(',')[0].trim();
  let protocol = forwardedProto || req.protocol || 'http';

  // Always use HTTP for local loopback hosts; avoids HTTPS->HTTP mismatch (EPROTO) when listener is plain HTTP.
  if (/^(localhost|127\.0\.0\.1|\[?::1\]?)(:\d+)?$/i.test(host)) {
    protocol = 'http';
  }

  return `${protocol}://${host}`;
}

// Conditionally mount proxy routes early so downstream handlers can use them
if (config.enableProxy) {
  logger.info('[startup] enableProxy flag active: mounting proxy routes');
  createProxyRoutes(app);
} else {
  logger.info('[startup] enableProxy flag disabled: proxy routes not mounted');
}

// Mount Proxy Route (M3U8 Rewriter)
app.use('/api/proxy', proxyRoutes);

// Mount LiveTV routes
app.use('/api/livetv', livetvRoutes);

// --- Simple In-Memory Rate Limiting for /auth/login ---
const loginAttempts = new Map(); // key: ip, value: { count, first, last, lockedUntil }
const MAX_ATTEMPTS_WINDOW = 5; // attempts allowed
const WINDOW_MS = 10 * 60 * 1000; // 10 minutes window
const BASE_LOCK_MS = 5 * 60 * 1000; // 5 minutes base lock

function getClientIp(req){
  return (req.headers['x-forwarded-for'] || req.connection.remoteAddress || '').split(',')[0].trim();
}

function recordLoginFailure(ip){
  const now = Date.now();
  let entry = loginAttempts.get(ip);
  if (!entry) {
    entry = { count:1, first: now, last: now, lockedUntil:0 };
    loginAttempts.set(ip, entry);
    return entry;
  }
  // Reset window if outside timeframe and not locked
  if (now - entry.first > WINDOW_MS && now > entry.lockedUntil) {
    entry.count = 1;
    entry.first = now;
  } else {
    entry.count++;
  }
  entry.last = now;
  if (entry.count > MAX_ATTEMPTS_WINDOW) {
    // Exponential backoff lock: base * 2^(count - limit)
    const over = entry.count - MAX_ATTEMPTS_WINDOW;
    const lockMs = BASE_LOCK_MS * Math.min(8, Math.pow(2, over-1));
    entry.lockedUntil = now + lockMs;
  }
  return entry;
}

function canAttempt(ip){
  const entry = loginAttempts.get(ip);
  if (!entry) return { allowed:true };
  const now = Date.now();
  if (entry.lockedUntil && now < entry.lockedUntil) {
    return { allowed:false, retryAfter: Math.ceil((entry.lockedUntil - now)/1000) };
  }
  if (now - entry.first > WINDOW_MS) {
    // Window passed; reset
    loginAttempts.delete(ip);
    return { allowed:true };
  }
  return { allowed:true };
}

function recordLoginSuccess(ip){
  // On success clear state to avoid lingering count
  loginAttempts.delete(ip);
}

// Guard against premature process.exit from imported legacy modules, but allow controlled restarts
const realProcessExit = process.exit.bind(process);
let allowControlledExit = false;
process.exit = function(code){
  if (allowControlledExit) return realProcessExit(code);
  console.warn('[diagnostic] Intercepted process.exit with code', code, new Error('exit trace').stack);
  // keep process alive for debugging
};
setImmediate(()=>console.log('[diagnostic] post-start setImmediate fired'));
app.use(cors());
app.use(express.json());

// Stealth Logger Middleware
app.use((req, res, next) => {
    const ip = getClientIp(req);
    logger.info(`[request] ${req.method} ${req.url} from ${stealth.anonymize(ip)}`);
    next();
});

// --- Auth Routes (login before static serving) ---
app.post('/auth/login', (req,res) => {
  const { username, password } = req.body || {};
  const ip = getClientIp(req);
  const attemptState = canAttempt(ip);
  if (!attemptState.allowed) {
    res.setHeader('Retry-After', String(attemptState.retryAfter));
    return res.status(429).json({ success:false, error:'TOO_MANY_ATTEMPTS', retryAfter: attemptState.retryAfter });
  }
  if (!username || !password) return res.status(400).json({ success:false, error:'MISSING_CREDENTIALS' });
  if (!authenticate(username, password)) {
    const entry = recordLoginFailure(ip);
    if (entry.lockedUntil && Date.now() < entry.lockedUntil) {
      const retryAfter = Math.ceil((entry.lockedUntil - Date.now())/1000);
      res.setHeader('Retry-After', String(retryAfter));
      return res.status(429).json({ success:false, error:'LOCKED', retryAfter });
    }
    return res.status(401).json({ success:false, error:'INVALID_CREDENTIALS', remaining: Math.max(0, MAX_ATTEMPTS_WINDOW - entry.count) });
  }
  recordLoginSuccess(ip);
  const token = issueSession(username);
  res.setHeader('Set-Cookie', `session=${token}; HttpOnly; SameSite=Lax; Path=/; Max-Age=${12*60*60}`);
  res.json({ success:true, username });
});

app.post('/auth/logout', (req,res) => {
  res.setHeader('Set-Cookie', 'session=; HttpOnly; SameSite=Lax; Path=/; Max-Age=0');
  res.json({ success:true });
});

app.get('/auth/session', (req,res) => {
  const sess = getSession(req);
  if (!sess) return res.json({ authenticated:false });
  res.json({ authenticated:true, username: sess.u });
});

app.post('/auth/change-password', requireAuth, (req,res) => {
  const { oldPassword, newPassword } = req.body || {};
  if (!oldPassword || !newPassword) return res.status(400).json({ success:false, error:'MISSING_FIELDS' });
  const sess = req.session;
  if (!authenticate(sess.u, oldPassword)) return res.status(401).json({ success:false, error:'INVALID_OLD_PASSWORD' });
  if (newPassword.length < 8) return res.status(400).json({ success:false, error:'PASSWORD_TOO_SHORT' });
  if (!updatePassword(sess.u, newPassword)) return res.status(500).json({ success:false, error:'UPDATE_FAILED' });
  res.json({ success:true, message:'PASSWORD_UPDATED' });
});

// Protect config panel (HTML) explicitly before static middleware
app.get('/config.html', (req,res,next) => {
  const sess = getSession(req);
  if (!sess) return res.redirect(302, '/');
  res.setHeader('Cache-Control','no-store, must-revalidate');
  res.setHeader('Pragma','no-cache');
  res.setHeader('Expires','0');
  res.sendFile(path.join(process.cwd(),'public','config.html'));
});

// Explicit root handler for login page to ensure no-store
app.get('/', (req,res) => {
  res.setHeader('Cache-Control','no-store, must-revalidate');
  res.setHeader('Pragma','no-cache');
  res.setHeader('Expires','0');
  res.sendFile(path.join(process.cwd(),'public','index.html'));
});

// Diagnostics for unexpected exits
process.on('beforeExit', (code) => {
  console.log('[diagnostic] beforeExit code=', code);
});
process.on('exit', (code) => {
  console.log('[diagnostic] exit code=', code);
});
process.on('uncaughtException', (err) => {
  console.error('[diagnostic] uncaughtException', err);
});
process.on('unhandledRejection', (reason) => {
  console.error('[diagnostic] unhandledRejection', reason);
});
// Periodic heartbeat to confirm event loop activity (can be removed later)
let hbCount = 0;
setInterval(()=>{
  hbCount++;
  if (hbCount % 6 === 0) { // every 60s if interval is 10s
    console.log('[diagnostic] heartbeat 60s elapsed, process alive');
  }
}, 10_000).unref();


// --- Metrics (in-memory) ---
const metrics = {
  startTime: Date.now(),
  requestsTotal: 0,
  streamRequests: 0,
  providerCalls: {},
  lastRequestAt: null,
  lastError: null,
  streamsReturned: 0,
  tmdbToImdbLookups: 0
};

// Track background tasks to avoid duplicates
const pendingBackgroundTasks = new Set();

app.use((req,res,next)=>{ metrics.requestsTotal++; metrics.lastRequestAt = Date.now(); next(); });
// Serve static UI (login page at /)
app.use(express.static(path.join(process.cwd(),'public')));

// Config API
app.get('/api/config', (req,res) => {
  const fs = require('fs');
  let override = {};
  try { if (fs.existsSync(OVERRIDE_PATH)) override = JSON.parse(fs.readFileSync(OVERRIDE_PATH,'utf8')); } catch (e) {
    // ignore JSON parse or fs errors reading override; return base config
  }
  res.json({ success:true, merged: config, override, overridePath: OVERRIDE_PATH });
});
app.post('/api/config', (req,res) => {
  const patch = req.body || {};
  if (patch.port) {
    const p = Number(patch.port); if (!Number.isFinite(p) || p<=0 || p>65535) return res.status(400).json({ success:false, error:'INVALID_PORT'});
    patch.port = p;
  }
  if (patch.defaultProviders && !Array.isArray(patch.defaultProviders)) return res.status(400).json({ success:false, error:'DEFAULT_PROVIDERS_NOT_ARRAY'});
  const ok = saveConfigPatch(patch);
  res.json({ success: ok, merged: config });
});

// Restart endpoint (requires auth via session cookie on /config.html UI)
app.post('/api/restart', (req,res) => {
  const sess = getSession(req);
  if(!sess) return res.status(401).json({ success:false, error:'UNAUTHORIZED' });
  res.json({ success:true, message:'RESTARTING' });
  // Give the response a moment to flush
  setTimeout(()=>{
    try {
      const fs = require('fs');
      const restartMarker = require('path').join(process.cwd(), 'restart.trigger');
      fs.writeFileSync(restartMarker, String(Date.now()));
      console.warn('[control] wrote restart.trigger to notify nodemon');
    } catch (e) {
      console.warn('[control] failed to write restart marker:', e.message);
    }
    console.warn('[control] restarting process by exit(0)');
    // Let nodemon detect the file change and restart the app
    allowControlledExit = true;
    realProcessExit(0);
  }, 300);
});

// --- Basic informational endpoints ---
app.get('/api/health', (req, res) => {
  res.json({ ok: true, service: 'tmdb-embed-api', time: new Date().toISOString() });
});

// Metrics endpoint
app.get('/api/metrics', (req,res) => {
  res.json({
    uptimeSeconds: Math.round((Date.now()-metrics.startTime)/1000),
    requestsTotal: metrics.requestsTotal,
    streamRequests: metrics.streamRequests,
    providerCalls: metrics.providerCalls,
    streamsReturned: metrics.streamsReturned,
    tmdbToImdbLookups: metrics.tmdbToImdbLookups,
    lastRequestAt: metrics.lastRequestAt,
    memoryMB: Math.round(process.memoryUsage().rss/1024/1024),
    loadAvg: os.loadavg ? os.loadavg() : [],
    nodeVersion: process.version,
    configDefaults: {
      region: config.defaultRegion,
      providers: config.defaultProviders,
      minQualities: config.minQualities ? Object.keys(config.minQualities).length : 0,
      excludeCodecs: config.excludeCodecs ? Object.keys(config.excludeCodecs).filter(k=>config.excludeCodecs[k]).length : 0,
      febboxCookies: config.febboxCookies.length
    }
  });
});

// Consolidated status (metrics + providers + endpoints)
app.get('/api/status', (req,res) => {
  const endpoints = [
    'GET /api/health',
    'GET /api/metrics',
    'GET /api/status',
    'GET /api/providers',
    'GET /api/providers/:name',
    'GET /api/streams/:type/:tmdbId',
    'GET /api/streams/:provider/:type/:tmdbId',
    'POST /api/config',
    'GET /api/config'
  ];
  // Determine cookie requirement heuristically (currently Showbox / PStream)
  const cookieRequiredProviders = new Set(['showbox']);
  const providers = listProviders().map(p => {
    const cookieRequired = cookieRequiredProviders.has(p.name);
    const cookieOk = !cookieRequired || (config.febboxCookies && config.febboxCookies.length > 0);
    return { name: p.name, enabled: p.enabled, cookieRequired, cookieOk };
  });
  res.json({ success:true, metrics: {
    uptimeSeconds: Math.round((Date.now()-metrics.startTime)/1000),
    requestsTotal: metrics.requestsTotal,
    streamRequests: metrics.streamRequests,
    providerCalls: metrics.providerCalls,
    streamsReturned: metrics.streamsReturned,
    tmdbToImdbLookups: metrics.tmdbToImdbLookups,
    lastRequestAt: metrics.lastRequestAt,
    memoryMB: Math.round(process.memoryUsage().rss/1024/1024)
  }, endpoints, providers });
});

// Providers list
app.get('/api/providers', (req,res) => {
  res.json({ success: true, providers: listProviders() });
});

// Clear in-memory stream cache
function handleCacheClear(req, res) {
  try {
    clearCache();
    res.json({ success: true });
  } catch (e) {
    res.status(500).json({ success: false, error: 'CACHE_CLEAR_FAILED', message: e.message });
  }
}

app.post('/api/cache/clear', handleCacheClear);
app.get('/api/cache/clear', handleCacheClear);

// Debug environment/config endpoint (do not expose publicly in production)
app.get('/api/debug/env', (req,res) => {
  const cookieStats = getCookieStats ? getCookieStats() : null;
  res.json({
    port: config.port,
    defaultProviders: config.defaultProviders,
    febboxCookieCount: config.febboxCookies.length,
    showboxCacheDir: process.env.SHOWBOX_CACHE_DIR || '(os tmp)',
    nodeVersion: process.version,
    cookieStats
  });
});

// Single provider info
app.get('/api/providers/:name', (req,res) => {
  const p = getProvider(req.params.name);
  if (!p) return res.status(404).json({ success:false, error:'PROVIDER_NOT_FOUND' });
  res.json({ success:true, provider:{ name: p.name, enabled: p.enabled } });
});

// Dedicated Drama Endpoint (KissKH + Dramacool only)
app.get('/api/drama/:type/:tmdbId', async (req,res) => {
  const { type, tmdbId } = req.params;
  const season = req.query.season ? Number(req.query.season) : null;
  const episode = req.query.episode ? Number(req.query.episode) : null;
  
  try {
    metrics.streamRequests++;
    const dramaProviders = ['kisskh', 'dramacool'];
    const streamsResults = [];
    const providerTimings = {};

    // Run drama providers sequentially/parallel
    await Promise.all(dramaProviders.map(async name => {
      const prov = getProvider(name);
      if (!prov || !prov.enabled) return;
      
      metrics.providerCalls[name] = (metrics.providerCalls[name]||0)+1;
      try {
        console.log(`[api] invoking drama provider ${name} for tmdbId=${tmdbId}`);
        const t0 = Date.now();
        const r = await prov.fetch({ tmdbId, type: 'tv', season, episode, imdbId: null, filters:{} });
        providerTimings[name] = Date.now()-t0;
        if (Array.isArray(r)) streamsResults.push(...r);
      } catch (e) {
        console.error(`[api] drama provider ${name} failed:`, e.message);
      }
    }));

    let streams = streamsResults;
    streams = applyFilters(streams, 'drama', config.minQualities, config.excludeCodecs);
    metrics.streamsReturned += streams.length;

    if (config.enableProxy) {
      const serverUrl = resolveServerUrl(req);
      streams = processStreamsForProxy(streams, serverUrl);
    }

    res.json({ 
        success: true, 
        tmdbId, 
        type: 'drama',
        count: streams.length, 
        providerTimings, 
        streams 
    });
  } catch (e) {
    metrics.lastError = e.message;
    res.status(500).json({ success:false, error:'INTERNAL_ERROR', message:e.message });
  }
});

// Aggregate streams across all enabled providers
app.get('/api/streams/:type/:tmdbId', async (req,res) => {
  const { type, tmdbId } = req.params;
  if (!['movie','series'].includes(type)) return res.status(400).json({ success:false, error:'INVALID_TYPE' });
  const season = req.query.season ? Number(req.query.season) : null;
  const episode = req.query.episode ? Number(req.query.episode) : null;
  try {
    metrics.streamRequests++;
    const tmdbType = type === 'movie' ? 'movie' : 'tv';
    const imdbId = await resolveImdbId(tmdbType, tmdbId); if (imdbId) metrics.tmdbToImdbLookups++;
    
    // Check global cache first
    const cachedStreams = getCachedStreams(type, tmdbId, season, episode);
    if (cachedStreams && cachedStreams.length > 0) {
        logger.info(`[api] returning ${cachedStreams.length} streams from cache for ${tmdbId}`);
        let streams = applyFilters(cachedStreams, 'aggregate', config.minQualities, config.excludeCodecs);
        const serverUrl = resolveServerUrl(req);
        if (config.enableProxy) {
          streams = processStreamsForProxy(streams, serverUrl);
        }
        return res.json({ success: true, tmdbId, imdbId, count: streams.length, streams, fromCache: true });
    }
    
    // EXCLUDE specialized Asian drama providers from general aggregation
    const dramaProviders = ['kisskh', 'dramacool', 'zenith', 'orient'];
    const selectedProviders = (config.defaultProviders.length ? config.defaultProviders : listProviders().map(p=>p.name))
        .filter(name => !dramaProviders.includes(name.toLowerCase()));

    const providerTimings = {};
    const streamsResults = [];
    
    logger.info(`[api] fetching streams for tmdbId=${tmdbId} (Smart Wait: 15s)`);
    const tStart = Date.now();

    // Map each provider to a promise that includes its own 60s safety timeout
    const providerPromises = selectedProviders.map(async name => {
      const prov = getProvider(name);
      if (!prov || !prov.enabled) return [];
      
      const isFast = prov.realName === 'vixsrc' || prov.realName === 'aura';
      metrics.providerCalls[name] = (metrics.providerCalls[name]||0)+1;
      try {
        const t0 = Date.now();
        // Add a safety timeout per provider
        const r = await Promise.race([
            prov.fetch({ tmdbId, type, season, episode, imdbId, filters:{ } }),
            new Promise((_, reject) => setTimeout(() => reject(new Error('Provider timeout')), isFast ? 15000 : 60000))
        ]);
        providerTimings[name] = Date.now()-t0;
        const results = Array.isArray(r) ? r : [];
        
        if (results.length > 0) {
            addCachedStreams(type, tmdbId, season, episode, results);
        }
        return results;
      } catch (e) {
        logger.error(`[api] provider ${name} failed or timed out:`, e.message);
        return [];
      }
    });

    // Create a "deadline" promise that resolves after 15 seconds
    const deadline = new Promise(resolve => setTimeout(() => resolve('DEADLINE'), 15000));

    // Wait for either all providers to finish OR the 15s deadline
    const resultsOrDeadline = await Promise.race([
        Promise.all(providerPromises),
        deadline
    ]);

    if (resultsOrDeadline === 'DEADLINE') {
        logger.info(`[api] deadline reached, returning partial results for ${tmdbId}`);
    } else if (Array.isArray(resultsOrDeadline)) {
        logger.info(`[api] all providers completed within deadline for ${tmdbId}`);
        resultsOrDeadline.forEach((r, idx) => {
            if (r && Array.isArray(r)) {
                streamsResults.push(...r);
            } else {
                const name = selectedProviders[idx] || 'unknown';
                logger.warn(`[api] provider ${name} (index ${idx}) returned non-array result:`, r);
            }
        });
    }

    // Always check the cache for the final response to include anything that finished just now
    const cachedAtEnd = getCachedStreams(type, tmdbId, season, episode);
    const finalStreams = (cachedAtEnd && cachedAtEnd.length > 0) ? cachedAtEnd : streamsResults;
    const totalDuration = Date.now() - tStart;

    let streams = finalStreams;
    if (!Array.isArray(streams)) {
        logger.error(`[api] finalStreams is not an array for ${tmdbId}:`, streams);
        streams = [];
    }
    
    streams = applyFilters(streams, 'aggregate', config.minQualities, config.excludeCodecs);
    metrics.streamsReturned += streams.length;
    
    const serverUrl = resolveServerUrl(req);
    if (config.enableProxy) {
      streams = processStreamsForProxy(streams, serverUrl);
    }
    
    res.json({ 
        success: true, 
        tmdbId, 
        imdbId, 
        count: streams.length, 
        providerTimings, 
        streams,
        totalDurationMs: totalDuration,
        status: resultsOrDeadline === 'DEADLINE' ? "Partial" : "Complete"
    });
    return;
  } catch (e) {
    metrics.lastError = e.message;
    res.status(500).json({ success:false, error:'INTERNAL_ERROR', message:e.message });
  }
});

// Livesport sports streams endpoint
app.get('/sports-stream/:matchName*', async (req, res) => {
  const matchName = req.params.matchName + (req.params[0] || '');
  if (!matchName) {
    return res.status(400).json({ success: false, error: 'Match name required' });
  }

  const prov = getProvider('livesport');
  if (!prov) return res.status(404).json({ success: false, error: 'Livesport provider not found' });
  if (!prov.enabled) return res.status(503).json({ success: false, error: 'Livesport provider disabled' });

  try {
    metrics.streamRequests++;
    metrics.providerCalls[prov.name] = (metrics.providerCalls[prov.name] || 0) + 1;
    
    const t0 = Date.now();
    // Pass the request object so the provider can extract the match name
    let streams = await prov.fetch({ req, type: 'sports', matchName, filters: {} });
    const providerTimings = { [prov.name]: Date.now() - t0 };
    
    streams = applyFilters(streams, prov.name, config.minQualities, config.excludeCodecs);
    metrics.streamsReturned += streams.length;
    
    if (config.enableProxy) {
      const serverUrl = resolveServerUrl(req);
      streams = processStreamsForProxy(streams, serverUrl);
    }
    
    res.json({ 
      success: true, 
      provider: prov.name, 
      matchName, 
      count: streams.length, 
      providerTimings, 
      streams 
    });
  } catch (e) {
    metrics.lastError = e.message;
    res.status(500).json({ success: false, error: 'INTERNAL_ERROR', message: e.message });
  }
});

// LiveTV sports streams endpoint
app.get('/livetv/:matchName*', async (req, res) => {
  const matchName = req.params.matchName + (req.params[0] || '');
  if (!matchName) {
    return res.status(400).json({ success: false, error: 'Match name required' });
  }

  const prov = getProvider('livetv');
  if (!prov) return res.status(404).json({ success: false, error: 'LiveTV provider not found' });
  if (!prov.enabled) return res.status(503).json({ success: false, error: 'LiveTV provider disabled' });

  try {
    metrics.streamRequests++;
    metrics.providerCalls[prov.name] = (metrics.providerCalls[prov.name] || 0) + 1;
    
    const t0 = Date.now();
    // Pass the request object so the provider can extract the match info
    let streams = await prov.fetch({ req, type: 'sports', matchName, filters: {} });
    const providerTimings = { [prov.name]: Date.now() - t0 };
    
    streams = applyFilters(streams, prov.name, config.minQualities, config.excludeCodecs);
    metrics.streamsReturned += streams.length;
    
    if (config.enableProxy) {
      const serverUrl = resolveServerUrl(req);
      streams = processStreamsForProxy(streams, serverUrl);
    }
    
    res.json({ 
      success: true, 
      provider: prov.name, 
      matchName, 
      count: streams.length, 
      providerTimings, 
      streams 
    });
  } catch (e) {
    metrics.lastError = e.message;
    res.status(500).json({ success: false, error: 'INTERNAL_ERROR', message: e.message });
  }
});

// Provider-specific streams
app.get('/api/streams/:provider/:type/:tmdbId', async (req,res) => {
  const { provider, type, tmdbId } = req.params;
  if (!['movie','series'].includes(type)) return res.status(400).json({ success:false, error:'INVALID_TYPE' });
  const season = req.query.season ? Number(req.query.season) : null;
  const episode = req.query.episode ? Number(req.query.episode) : null;
  const prov = getProvider(provider);
  if (!prov) return res.status(404).json({ success:false, error:'PROVIDER_NOT_FOUND' });
  if (!prov.enabled) return res.status(503).json({ success:false, error:'PROVIDER_DISABLED' });
  try {
    metrics.streamRequests++;
    metrics.providerCalls[prov.name] = (metrics.providerCalls[prov.name]||0)+1;
    const tmdbType = type === 'movie' ? 'movie' : 'tv';
    const imdbId = await resolveImdbId(tmdbType, tmdbId); if (imdbId) metrics.tmdbToImdbLookups++;
    const t0 = Date.now();
    let streams = await prov.fetch({ tmdbId, type, season, episode, imdbId, filters:{} });
    const providerTimings = { [prov.name]: Date.now()-t0 };
    streams = applyFilters(streams, prov.name, config.minQualities, config.excludeCodecs);
    metrics.streamsReturned += streams.length;
    if (config.enableProxy) {
      const serverUrl = resolveServerUrl(req);
      streams = processStreamsForProxy(streams, serverUrl);
    }
    res.json({ success:true, provider: prov.name, tmdbId, imdbId, count: streams.length, providerTimings, streams });
  } catch (e) {
    metrics.lastError = e.message;
    res.status(500).json({ success:false, error:'INTERNAL_ERROR', message:e.message });
  }
});

// HiTV Endpoint (KDrama Discovery)
const hitvApi = require('./utils/hitv-api');
app.get('/api/hitv/browse', async (req, res) => {
    try {
        const page = req.query.page || 1;
        const pageSize = req.query.pageSize || 28;
        const country = req.query.country || "Korea";
        const category = req.query.category || "Drama";
        const genre = req.query.genre || "All";
        const sort = req.query.sort || "Hottest";
        const data = await hitvApi.browseHiTv(page, pageSize, country, category, genre, sort);
        res.json({ success: true, ...data });
    } catch (e) {
        res.status(500).json({ success: false, error: e.message });
    }
});

app.get('/api/hitv/latest', async (req, res) => {
    try {
        const page = req.query.page || 1;
        const data = await hitvApi.browseHiTv(page, 20, "Korea", "Drama", "All", "Newest");
        res.json({ success: true, ...data });
    } catch (e) {
        res.status(500).json({ success: false, error: e.message });
    }
});

app.get('/api/hitv/trending', async (req, res) => {
    try {
        const page = req.query.page || 1;
        const data = await hitvApi.browseHiTv(page, 20, "Korea", "Drama", "All", "Recommend");
        res.json({ success: true, ...data });
    } catch (e) {
        res.status(500).json({ success: false, error: e.message });
    }
});

app.get('/api/hitv/', async (req, res) => {
    const q = req.query.q;
    const type = req.query.type || 2; // 1 for Movies, 2 for Series
    if (!q) return res.status(400).json({ success: false, error: 'Query required' });
    try {
        const data = await hitvApi.searchHiTv(q, type);
        res.json({ success: true, ...data });
    } catch (e) {
        res.status(500).json({ success: false, error: e.message });
    }
});

app.get('/api/hitv/countries', (req, res) => {
    res.json(hitvApi.getHiTvCountries());
});

app.get('/api/hitv/categories', (req, res) => {
    res.json(hitvApi.getHiTvCategories());
});

app.get('/api/hitv/genres', (req, res) => {
    res.json(hitvApi.getHiTvGenres());
});

app.get('/api/hitv/sorts', (req, res) => {
    res.json(hitvApi.getHiTvSorts());
});

app.get('/api/hitv/search', async (req, res) => {
    const { q, type } = req.query;
    if (!q) return res.status(400).json({ success: false, error: 'Query required' });
    try {
        // type: 1=movie, 2=tv (default)
        const data = await hitvApi.searchHiTv(q, parseInt(type) || 2);
        res.json(data);
    } catch (e) {
        res.status(500).json({ success: false, error: e.message });
    }
});

app.get('/api/hitv/info/:id', async (req, res) => {
    try {
        const data = await hitvApi.getHiTvDetail(req.params.id);
        if (!data) return res.status(404).json({ success: false, error: 'Not found' });
        res.json({ success: true, ...data });
    } catch (e) {
        res.status(500).json({ success: false, error: e.message });
    }
});

// kisskhProvider is already required at the top or via getProvider, but for direct access we can use require here if not present
// However, to fix the syntax error, I will just use the file directly without redeclaring if possible, or rename it.
// Actually, safer to just remove the lines and use the `getProvider` logic or just `require` inline.
// But `getStreamsDirect` is a specific export not on the default provider object (which usually just has fetch).
// So I will fix the declaration.

const kisskhDirect = require('./providers/kisskh');
const sflixDirect = require('./providers/sflix');

app.get('/api/hitv/play/:id/:episode', async (req, res) => {
    const { id, episode } = req.params;
    try {
        // 1. Check Cache
        // We use a custom type 'hitv' for caching since we lack TMDB ID
        const cached = getCachedStreams('hitv', id, 1, episode);
        if (cached && cached.length > 0) {
            console.log(`[HiTV-Play] Returning cached streams for ${id} Ep ${episode}`);
            let finalStreams = cached;
            if (config.enableProxy) {
                const serverUrl = resolveServerUrl(req);
                finalStreams = processStreamsForProxy(finalStreams, serverUrl);
            }
            return res.json({ success: true, source: 'cache', episode, streams: finalStreams });
        }

        // 2. Resolve Metadata (Title & Slug)
        // We always fetch info to get the slug (detailPath) for SFlix mapping
        const info = await hitvApi.getHiTvDetail(id);
        let targetTitle = req.query.title || (info ? info.title : null);

        if (!targetTitle || targetTitle === "Unknown") {
            return res.status(404).json({ success: false, error: 'Show title not found. Please provide ?title= param.' });
        }

        console.log(`[HiTV-Play] Mapping "${targetTitle}" Ep ${episode} to providers...`);

        // 3. Parallel Fetch from Zenith (KissKH) and Nova (SFlix)
        // Since SFlix shares IDs with HiTV, we use the Direct Slug approach for SFlix
        const fetchWithTimeout = (promise, name, ms = 15000) => 
            Promise.race([
                promise,
                new Promise(resolve => setTimeout(() => {
                    console.log(`[HiTV] ${name} timed out after ${ms}ms`);
                    resolve([]);
                }, ms))
            ]);

        const [kissStreams, sflixStreams] = await Promise.all([
            fetchWithTimeout(
                kisskhDirect.getStreamsDirect(targetTitle, episode).catch(e => {
                    console.error(`[HiTV] Zenith failed: ${e.message}`);
                    return [];
                }), 
                "Zenith",
                30000 // Increased from 15s to 30s
            ),
            fetchWithTimeout(
                (async () => {
                    // Try direct slug if we have it from info (Sharing ID logic)
                    if (info && info.detailPath) {
                        return sflixDirect.getStreamsBySlug(id, info.detailPath, episode);
                    }
                    // Fallback to title search if slug is missing
                    return sflixDirect.getStreamsDirect(targetTitle, episode, 1);
                })().catch(e => {
                    console.error(`[HiTV] Nova failed: ${e.message}`);
                    return [];
                }), 
                "Nova", 
                35000
            )
        ]);

        let streams = [...kissStreams, ...sflixStreams];
        
        // 4. Cache Results
        if (streams.length > 0) {
            addCachedStreams('hitv', id, 1, episode, streams);
        }

        // 5. Proxy
        if (config.enableProxy) {
            const serverUrl = resolveServerUrl(req);
            streams = processStreamsForProxy(streams, serverUrl);
        }

        res.json({
            success: true,
            title: targetTitle,
            episode,
            streams
        });

    } catch (e) {
        res.status(500).json({ success: false, error: e.message });
    }
});

// KissKH API proxy routes

app.get('/api/kdrama/search', async (req, res) => {
    const q = req.query.q;
    if (!q) return res.status(400).json({ success: false, error: 'Query required' });
    try {
        const results = await kisskhApi.search(q);
        res.json({ success: true, results });
    } catch (e) {
        res.status(500).json({ success: false, error: e.message });
    }
});

app.get('/api/kdrama/list', async (req, res) => {
    try {
        const { page, type, sub, country, status, order } = req.query;
        // Map string keys to IDs if provided
        const countryId = kisskhApi.COUNTRY_MAP[country] !== undefined ? kisskhApi.COUNTRY_MAP[country] : (country || 0);
        const orderId = kisskhApi.ORDER_MAP[order] !== undefined ? kisskhApi.ORDER_MAP[order] : (order || 2);
        const statusId = kisskhApi.STATUS_MAP[status] !== undefined ? kisskhApi.STATUS_MAP[status] : (status || 0);

        const data = await kisskhApi.getList({ 
            page: page || 1, 
            type: type || 0, 
            sub: sub || 0, 
            country: countryId, 
            status: statusId, 
            order: orderId 
        });
        res.json({ success: true, ...data });
    } catch (e) {
        res.status(500).json({ success: false, error: e.message });
    }
});

app.get('/api/kdrama/latest', async (req, res) => {
    try {
        const page = req.query.page || 1;
        const data = await kisskhApi.getList({ page, order: 2 }); // Latest
        res.json({ success: true, ...data });
    } catch (e) {
        res.status(500).json({ success: false, error: e.message });
    }
});

app.get('/api/kdrama/popular', async (req, res) => {
    try {
        const page = req.query.page || 1;
        const data = await kisskhApi.getList({ page, order: 1 }); // Popular
        res.json({ success: true, ...data });
    } catch (e) {
        res.status(500).json({ success: false, error: e.message });
    }
});

app.get('/api/kdrama/upcoming', async (req, res) => {
    try {
        const data = await kisskhApi.getUpcoming();
        res.json({ success: true, results: data });
    } catch (e) {
        res.status(500).json({ success: false, error: e.message });
    }
});

app.get('/api/kdrama/detail/:id', async (req, res) => {
    try {
        const data = await kisskhApi.getDetail(req.params.id);
        if (!data) return res.status(404).json({ success: false, error: 'Not found' });
        res.json({ success: true, data });
    } catch (e) {
        res.status(500).json({ success: false, error: e.message });
    }
});

app.get('/api/kdrama/streams/:id/:episode?', async (req, res) => {
    const showId = req.params.id;
    const episode = req.params.episode || 1;
    
    try {
        metrics.streamRequests++;
        console.log(`[api] kdrama stream request: ${showId} ep ${episode}`);
        
        const streams = await kisskhProvider.getStreamsDirect(showId, episode);
        
        let finalStreams = applyFilters(streams, 'kisskh', config.minQualities, config.excludeCodecs);
        metrics.streamsReturned += finalStreams.length;

        if (config.enableProxy) {
            const serverUrl = resolveServerUrl(req);
            finalStreams = processStreamsForProxy(finalStreams, serverUrl);
        }

        res.json({
            success: true,
            showId,
            episode,
            count: finalStreams.length,
            streams: finalStreams
        });
    } catch (e) {
        metrics.lastError = e.message;
        res.status(500).json({ success: false, error: e.message });
    }
});

app.get('/api/dramacool/streams', async (req, res) => {
    const url = req.query.url;
    if (!url) return res.status(400).json({ success: false, error: 'URL required' });
    
    try {
        metrics.streamRequests++;
        console.log(`[api] dramacool direct URL request: ${url}`);
        
        const streams = await dramacoolProvider.getStreamsByUrl(url);
        
        let finalStreams = applyFilters(streams, 'dramacool', config.minQualities, config.excludeCodecs);
        metrics.streamsReturned += finalStreams.length;

        if (config.enableProxy) {
            const serverUrl = resolveServerUrl(req);
            finalStreams = processStreamsForProxy(finalStreams, serverUrl);
        }

        res.json({
            success: true,
            url,
            count: finalStreams.length,
            streams: finalStreams
        });
    } catch (e) {
        metrics.lastError = e.message;
        res.status(500).json({ success: false, error: e.message });
    }
});

// --- Dramacool Browse API (Xyra Stream) ---
app.get('/api/dramacool/search', async (req, res) => {
    const { q, page } = req.query;
    if (!q) return res.status(400).json({ success: false, error: 'Query required' });
    try {
        const data = await dramacoolApi.search(q, page || 1);
        res.json(data);
    } catch (e) {
        res.status(500).json({ success: false, error: e.message });
    }
});

app.get('/api/dramacool/latest', async (req, res) => {
    try {
        const data = await dramacoolApi.getLatest(req.query.page || 1);
        res.json(data);
    } catch (e) {
        res.status(500).json({ success: false, error: e.message });
    }
});

app.get('/api/dramacool/latest-kdrama', async (req, res) => {
    try {
        const data = await dramacoolApi.getLatestKdrama(req.query.page || 1);
        res.json(data);
    } catch (e) {
        res.status(500).json({ success: false, error: e.message });
    }
});

app.get('/api/dramacool/popular', async (req, res) => {
    try {
        const data = await dramacoolApi.getPopular(req.query.page || 1);
        res.json(data);
    } catch (e) {
        res.status(500).json({ success: false, error: e.message });
    }
});

app.get('/api/dramacool/ongoing', async (req, res) => {
    try {
        const data = await dramacoolApi.getOngoing(req.query.page || 1);
        res.json(data);
    } catch (e) {
        res.status(500).json({ success: false, error: e.message });
    }
});

app.get('/api/dramacool/upcoming', async (req, res) => {
    try {
        const data = await dramacoolApi.getUpcoming(req.query.page || 1);
        res.json(data);
    } catch (e) {
        res.status(500).json({ success: false, error: e.message });
    }
});

app.get('/api/dramacool/home', async (req, res) => {
    try {
        const data = await dramacoolApi.getHome();
        res.json(data);
    } catch (e) {
        res.status(500).json({ success: false, error: e.message });
    }
});

app.get('/api/dramacool/info', async (req, res) => {
    const id = req.query.id;
    if (!id) return res.status(400).json({ success: false, error: 'ID required' });
    try {
        const data = await dramacoolApi.getDetail(id);
        res.json(data);
    } catch (e) {
        res.status(500).json({ success: false, error: e.message });
    }
});

app.get('/api/dramacool/browse/streams/:id/:episode', async (req, res) => {
    const { id, episode } = req.params;
    // Build the direct dramacool.sh URL from the ID and episode
    const url = `https://dramacool.sh/${id}-episode-${episode}/`;
    
    try {
        metrics.streamRequests++;
        console.log(`[api] dramacool browse stream request: ${url}`);
        
        const streams = await dramacoolProvider.getStreamsByUrl(url);
        
        let finalStreams = applyFilters(streams, 'dramacool', config.minQualities, config.excludeCodecs);
        metrics.streamsReturned += finalStreams.length;

        if (config.enableProxy) {
            const serverUrl = resolveServerUrl(req);
            finalStreams = processStreamsForProxy(finalStreams, serverUrl);
        }

        res.json({
            success: true,
            id,
            episode,
            url,
            count: finalStreams.length,
            streams: finalStreams
        });
    } catch (e) {
        metrics.lastError = e.message;
        res.status(500).json({ success: false, error: e.message });
    }
});

// For Vercel deployment: export the app
module.exports = app;

// For local development: start server if not in Vercel environment
if (process.env.NODE_ENV !== 'production' || !process.env.VERCEL) {
  const { refreshSflixCookie } = require('./utils/refreshSflixCookie');

  // Schedule SFlix Cookie Refresh (every 24h)
  /*
  setInterval(() => {
      refreshSflixCookie().catch(err => console.error('[Cron] Cookie refresh failed:', err));
  }, 24 * 60 * 60 * 1000);

  // Initial refresh on start (delay 30s to let server settle)
  setTimeout(() => {
      refreshSflixCookie().catch(err => console.error('[Startup] Cookie refresh failed:', err));
  }, 30000);
  */

  const PORT = config.port;
  const HOST = process.env.BIND_HOST || '0.0.0.0';
  const server = app.listen(PORT, HOST, () => {
    console.log(`TMDB Embed REST API v1.1.0-robust listening on http://${HOST}:${PORT}`);
    if (HOST !== 'localhost') {
      console.log(`Local access (if running on your machine): http://localhost:${PORT}`);
    }
    console.log('Endpoints:');
    console.log('  GET  /api/health');
    console.log('  GET  /api/metrics');
    console.log('  GET  /api/providers');
    console.log('  GET  /api/streams/:type/:id');
    console.log('  POST /api/streams/:type/:id');
    if (!config.febboxCookies || config.febboxCookies.length === 0) {
      console.warn('[startup][warning] No FEBBOX_COOKIES configured. Showbox / PStream related streams may be unavailable. Set FEBBOX_COOKIES in your environment to enable these sources.');
    }
  });

  server.on('error', (err)=>{ console.error('[diagnostic] server error', err); });
}
