/**
 * Provider Registry
 * Dynamic loading and management of streaming sources.
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @license MIT
 */

const fs = require('fs');
const path = require('path');
const { config } = require('../utils/config');

// Only remaining providers are Playwright-based or live
const providerFunctionMap = {
  'Showbox.js': 'getStreamsFromTmdbId',
  'showbox-playwright.js': 'getStreamsFromTmdbId',
  'MP4Hydra.js': 'getMP4HydraStreams',
  'VidZee.js': 'getVidZeeStreams',
  'vidsrccc-playwright.js': 'getVidsrcStreams',
  'moviesmod.js': 'getMoviesModStreams',
  'uhdmovies.js': 'getUHDMoviesStreams',
  '4khdhub.js': 'get4KHDHubStreams',
  'livesport.js': 'getLivesportStreams',
  'aisports.js': 'getAiSportsStreams',
  'livetv.js': 'getLivetvStreams',
  'ppv.js': 'getPPVStreams',
};

const providerCache = new Map();

function getProviderFiles() {
  const providersDir = path.join(__dirname);
  return fs.readdirSync(providersDir)
    .filter(file => file.endsWith('.js') &&
                    file !== 'registry.js' &&
                    providerFunctionMap[file])
    .map(file => ({
      name: path.parse(file).name.toLowerCase(),
      file: file,
      functionName: providerFunctionMap[file]
    }));
}

function loadProvider(providerFile) {
  if (!providerCache.has(providerFile)) {
    try {
      providerCache.set(providerFile, require(path.join(__dirname, providerFile)));
    } catch (e) {
      console.error(`Failed to load provider ${providerFile}:`, e.message);
      return null;
    }
  }
  return providerCache.get(providerFile);
}

function createFetchFunction(providerInfo) {
  return async function(ctx) {
    const mediaType = ctx.type === 'movie' ? 'movie' : 'tv';

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
        const { getTmdbApiKey } = require('../utils/tmdbKey');
        const tmdbApiKey = getTmdbApiKey();
        if (!tmdbApiKey) {
          console.warn('[registry] showbox skipped: TMDB API key missing');
          return [];
        }
        result = await module[funcName](mediaType, ctx.tmdbId, ctx.season || null, ctx.episode || null);
      } else if (['livesport', 'aisports', 'streamed', 'livetv', 'ppv'].includes(providerInfo.name)) {
        result = await module[funcName]({ ...ctx, req: ctx.req });
      } else {
        result = await module[funcName](ctx.tmdbId, mediaType, ctx.season || null, ctx.episode || null);
      }

      const durationMs = Date.now() - t0;
      console.log(`[registry] ${providerInfo.name} fetch duration ${durationMs}ms`);

      if (!Array.isArray(result)) return [];
      return result;
    } catch (e) {
      console.error(`[registry] ${providerInfo.name} fetch error:`, e.message);
      return [];
    }
  };
}

const providerFiles = getProviderFiles();
const providers = [];

for (const providerInfo of providerFiles) {
  const safeName = providerInfo.name.replace(/[^a-z0-9]/gi, '');
  const enableFlag = `enable${safeName.charAt(0).toUpperCase() + safeName.slice(1)}Provider`;

  let enabled = config[enableFlag] === true;

  if (enabled) {
    providers.push({
      name: providerInfo.name,
      realName: providerInfo.name,
      enabled: true,
      isFast: false,
      fetch: createFetchFunction(providerInfo)
    });
    console.log(`[registry] ${providerInfo.name} provider enabled`);
  } else {
    console.log(`[registry] ${providerInfo.name} disabled via config`);
  }
}

function listProviders() { return providers.map(p => ({ name: p.name, enabled: p.enabled })); }
function getProvider(name) {
  const lowName = name.toLowerCase();
  return providers.find(p => p.name.toLowerCase() === lowName || p.realName.toLowerCase() === lowName) || null;
}

module.exports = { listProviders, getProvider };
