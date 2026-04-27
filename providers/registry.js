/**
 * Provider Registry
 * Dynamic loading and management of streaming sources.
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @license MIT
 */

const { config } = require('../utils/config');
const fs = require('fs');
const path = require('path');
const { getCachedStreams, setCachedStreams } = require('../utils/cache');

// Map real provider names to "special" names for the API response
const PROVIDER_RENAME_MAP = {
  'huerust': 'Huerust',
  'sflix': 'Nova',
  'vixsrc': 'Aura',
  'vidrock': 'Luna',
  'vidsrc-xyz': 'Solar',
  'vidsrc-icu': 'Stellar',
  'vidsrc-embed': 'Nebula',
  'kisskh': 'Zenith',
  'dramacool': 'Orient',
  'moviesmod': 'Cine',
  'uhdmovies': 'Ultra',
  '4khdhub': 'Prime',
  'mp4hydra': 'Hydra',
  'showbox': 'Echo',
  'showbox-playwright': 'Echo Pro',
  'livesport': 'Stadium',
  'aisports': 'Field',
  'streamed': 'Relay',
  'livetv': 'Arena',
  'ppv': 'Event',
  'vidsrccc-playwright': 'Vertex',
  'moviebox': 'Cinema',
  '9anime': 'Saber',
  'autoembed': 'Blaze'
};

// Keywords to sanitize from titles/names (maps to public names)
const SANITIZE_KEYWORDS = {
  'vidsrc': 'Aura',
  'vidrock': 'Luna',
  'sflix': 'Nova',
  'kisskh': 'Zenith',
  'dramacool': 'Orient',
  'moviesmod': 'Cine',
  'uhdmovies': 'Ultra',
  '4khdhub': 'Prime',
  'mp4hydra': 'Hydra',
  'showbox': 'Echo',
  'livesport': 'Stadium',
  'aisports': 'Field',
  'streamed': 'Relay',
  'livetv': 'Arena',
  'ppv': 'Event',
  'moviebox': 'Cinema',
  '9anime': 'Saber',
  'autoembed': 'Blaze'
};

function getPublicName(realName) {
  return PROVIDER_RENAME_MAP[realName.toLowerCase()] || realName;
}

function isFastProvider(name) {
  const n = name.toLowerCase();
  return n === 'vixsrc' || n === 'aura' || n === 'huerust';
}

function sanitizeText(text) {
  if (!text) return text;
  let sanitized = text;
  for (const [key, replacement] of Object.entries(SANITIZE_KEYWORDS)) {
    const regex = new RegExp(key, 'gi');
    sanitized = sanitized.replace(regex, replacement);
  }
  return sanitized;
}

// Lazy load cache for providers
const providerCache = new Map();

// Function name mappings for existing providers
const providerFunctionMap = {
  'Showbox.js': 'getStreamsFromTmdbId',
  'showbox-playwright.js': 'getStreamsFromTmdbId',
  '4khdhub.js': 'get4KHDHubStreams',
  'moviesmod.js': 'getMoviesModStreams',
  'MP4Hydra.js': 'getMP4HydraStreams',
  'VidZee.js': 'getVidZeeStreams',
  'vixsrc.js': 'getVixsrcStreams',
  'uhdmovies.js': 'getUHDMoviesStreams',
  'vidsrccc-playwright.js': 'getVidsrcStreams',
  'livesport.js': 'getLivesportStreams',
  'aisports.js': 'getAiSportsStreams',
  'streamed.js': 'getStreamedStreams',
  'livetv.js': 'getLivetvStreams',
  'sflix.js': 'getStreams',
  'kisskh.js': 'getStreams',
  'vidrock.js': 'getStreams',
  'vidsrc-xyz.js': 'getStreams',
  'vidsrc-icu.js': 'getStreams',
  'dramacool.js': 'getStreams',
  'vidsrc-embed.js': 'getStreams',
  'moviebox.js': 'getStreams',
  '9anime.js': 'getStreams',
  'ppv.js': 'getPPVStreams',
  'autoembed.js': 'getStreams',
  'huerust.js': 'getStreams',
};

// Stats for debug endpoint
let lastCookieStats = { selected: null, index: null, total: 0, remainingMB: null, timestamp: null };

async function getEffectiveCookies() {
  return Array.isArray(config.febboxCookies) ? config.febboxCookies : [];
}

// Get all provider files
function getProviderFiles() {
  const providersDir = path.join(__dirname);
  return fs.readdirSync(providersDir)
    .filter(file => file.endsWith('.js') && 
                    file !== 'registry.js' && 
                    file !== 'vidsrcextractor.js' && 
                    file !== 'Showbox.js' && 
                    file !== 'showbox-playwright.js' && 
                    file !== 'VidZee.js' &&
                    file !== 'vidsrccc-puppeteer.js.bak')
    .map(file => ({
      name: path.parse(file).name.toLowerCase(),
      file: file,
      functionName: providerFunctionMap[file]
    }));
}

// Load a provider module
function loadProvider(providerFile) {
  if (!providerCache.has(providerFile)) {
    try {
      providerCache.set(providerFile, require(path.join(__dirname, providerFile)));
    } catch (e) {
      console.error(`Failed to load provider ${providerFile}:`, e.message);
      console.error(e.stack); // Added stack trace
      return null;
    }
  }
  return providerCache.get(providerFile);
}

// Create fetch function for a provider
function createFetchFunction(providerInfo) {
  return async function(ctx) {
    const mediaType = ctx.type === 'movie' ? 'movie' : 'tv';
    
    // Check cache first (skip for complex providers like livesport/livetv/ppv that rely on 'req')
    if (providerInfo.name !== 'livesport' && providerInfo.name !== 'aisports' && providerInfo.name !== 'streamed' && providerInfo.name !== 'livetv' && providerInfo.name !== 'ppv') {
        const cacheKeyType = `${providerInfo.name}:${mediaType}`;
        const cached = getCachedStreams(cacheKeyType, ctx.tmdbId, ctx.season, ctx.episode);
        if (cached) {
            console.log(`[registry] ${providerInfo.name} returned from cache`);
            return cached;
        }
    }

    const module = loadProvider(providerInfo.file);
    if (!module) return [];

    const funcName = providerInfo.functionName;
    if (!module[funcName]) {
      console.error(`Provider ${providerInfo.name} does not export ${funcName}`);
      return [];
    }

    try {
      const t0 = Date.now();
      
      console.log(`[registry] Calling ${providerInfo.name} with tmdbId=${ctx.tmdbId}, mediaType=${mediaType}`);

      let result;
      if (providerInfo.name === 'showbox' || providerInfo.name === 'showbox-playwright') {
        // Special case for Showbox with TMDB key and cookies
        const { getTmdbApiKey } = require('../utils/tmdbKey');
        const tmdbApiKey = getTmdbApiKey();
        if (!tmdbApiKey) {
          console.warn('[registry] showbox skipped: TMDB API key missing');
          return [];
        }
        const cookies = await getEffectiveCookies();
        const previousConfig = global.currentRequestConfig;
        global.currentRequestConfig = { ...(previousConfig || {}) };
        let selected = null;
        if (cookies.length > 0) {
          const index = Math.floor(Math.random() * cookies.length);
          selected = cookies[index];
          global.currentRequestConfig.cookie = selected.startsWith('ui=') ? selected : `ui=${selected}`;
          global.currentRequestConfig.cookies = cookies.map(c => c.startsWith('ui=') ? c : `ui=${c}`);
          lastCookieStats = { selected: selected.slice(0, 16) + '...', index, total: cookies.length, remainingMB: null, timestamp: Date.now() };
          console.log(`[registry] Cookie random pick index=${index} total=${cookies.length}`);
        }
        result = await module[funcName](mediaType, ctx.tmdbId, ctx.season || null, ctx.episode || null, null, selected);
        if (global.currentRequestUserCookieRemainingMB != null) {
          lastCookieStats.remainingMB = global.currentRequestUserCookieRemainingMB;
        }
        global.currentRequestConfig = previousConfig || {};
      } else {
        // Standard provider call
        if (providerInfo.name === 'vidsrccc-playwright') {
          // VidsrcCC needs IMDB ID for TV series
          result = await module[funcName](ctx.tmdbId, mediaType, ctx.season || null, ctx.episode || null, ctx.imdbId || null);
        } else if (providerInfo.name === 'livesport' || providerInfo.name === 'aisports' || providerInfo.name === 'streamed' || providerInfo.name === 'livetv' || providerInfo.name === 'ppv') {
          // Livesport/AiSports/Streamed/LiveTV/PPV needs the request object to extract the match name
          result = await module[funcName]({ ...ctx, req: ctx.req });
        } else if (providerInfo.name === 'moviebox') {
          // Moviebox can use pre-fetched tmdbData to avoid failing requests
          result = await module[funcName](ctx.tmdbId, mediaType, ctx.season || null, ctx.episode || null, ctx.tmdbData);
        } else {
          result = await module[funcName](ctx.tmdbId, mediaType, ctx.season || null, ctx.episode || null);
        }
      }

      const durationMs = Date.now() - t0;
      console.log(`[registry] ${providerInfo.name} fetch duration ${durationMs}ms`);

      if (!Array.isArray(result)) return [];

      // Add special provider name and sanitize titles
      const publicName = getPublicName(providerInfo.realName);
      const finalResult = result.map(s => {
          let sanitizedTitle = s.title || s.name || 'Stream';
          sanitizedTitle = sanitizeText(sanitizedTitle);
          
          return {
              ...s,
              title: sanitizedTitle,
              name: s.name ? sanitizeText(s.name) : undefined,
              provider: s.provider || publicName
          };
      });
      
      // Cache success result (skip livesport/aisports/livetv/ppv)
      if (providerInfo.realName !== 'livesport' && providerInfo.realName !== 'aisports' && providerInfo.realName !== 'livetv' && providerInfo.realName !== 'ppv' && finalResult.length > 0) {
          const cacheKeyType = `${providerInfo.realName}:${mediaType}`;
          setCachedStreams(cacheKeyType, ctx.tmdbId, ctx.season, ctx.episode, finalResult);
      }

      return finalResult;

    } catch (e) {
      console.error(`[registry] ${providerInfo.name} fetch error:`, e.message);
      return [];
    }
  };
}

// Initialize providers
const providerFiles = getProviderFiles();
const providers = [];

for (const providerInfo of providerFiles) {
  // Unify flag naming: enableVidsrcxyzProvider
  const safeName = providerInfo.name.replace(/[^a-z0-9]/gi, '');
  const enableFlag = `enable${safeName.charAt(0).toUpperCase() + safeName.slice(1)}Provider`;
  
  // Only enable when explicitly allowed in config
  let enabled = config[enableFlag] === true;
  if (providerInfo.name === 'vidsrc-xyz' || providerInfo.name === 'vidsrc-icu' || providerInfo.name === 'vidsrccc-playwright') {
      enabled = false;
  }

  // When Huerust is enabled, skip providers it already handles to avoid duplicates
  const HUERUST_HANDLES = ['moviebox', 'sflix', 'vixsrc', 'kisskh', 'dramacool'];
  if (enabled && config.enableHuerustProvider && HUERUST_HANDLES.includes(providerInfo.name)) {
      console.log(`[registry] ${providerInfo.name} skipped — handled by Huerust engine`);
      enabled = false;
  }

  if (enabled) {
    const publicName = getPublicName(providerInfo.name);
    const isFast = isFastProvider(providerInfo.name) || 
                   providerInfo.name === 'moviebox' || 
                   providerInfo.name === 'autoembed' || 
                   providerInfo.name === '4khdhub' ||
                   providerInfo.name === 'moviesmod' ||
                   providerInfo.name === 'uhdmovies';

    providers.push({
      name: publicName,
      realName: providerInfo.name,
      enabled: true,
      isFast: isFast,
      fetch: createFetchFunction({ ...providerInfo, realName: providerInfo.name, publicName })
    });
    console.log(`[registry] ${providerInfo.name} provider enabled as "${publicName}"`);
  } else {
    console.log(`[registry] ${providerInfo.name} disabled via config`);
  }
}

function listProviders() { return providers.map(p => ({ name: p.name, enabled: p.enabled })); }
function getProvider(name) { 
  const lowName = name.toLowerCase();
  return providers.find(p => p.name.toLowerCase() === lowName || p.realName.toLowerCase() === lowName) || null; 
}

function getCookieStats() { return lastCookieStats; }

module.exports = { listProviders, getProvider, getCookieStats };
