/**
 * Hue Stream API
 * Professional Streaming Aggregator
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */

const axios = require('axios');
const { config } = require('./config');
const { getTmdbApiKey } = require('./tmdbKey');

const cache = {
  imdbByTmdb: new Map(),
  details: new Map(),
  seasons: new Map(),
  search: new Map()
};

const TTL_MS = 6 * 60 * 60 * 1000;
function isFresh(entry) { return entry && (Date.now() - entry.ts) < TTL_MS; }

async function tmdbFetch(endpoint, params = {}) {
  const apiKey = getTmdbApiKey();
  if (!apiKey) throw new Error('TMDB_API_KEY missing');

  const url = `https://api.themoviedb.org/3${endpoint}`;
  const response = await axios.get(url, {
    params: { ...params, api_key: apiKey },
    timeout: 15000
  });
  return response.data;
}

/**
 * Get basic info for a TMDB ID
 * Returns { title, year, originalTitle, type }
 */
async function getTmdbInfo(tmdbId, mediaType) {
  const type = mediaType === 'tv' || mediaType === 'series' ? 'tv' : 'movie';
  const key = `info:${type}:${tmdbId}`;
  
  const cached = cache.details.get(key);
  if (isFresh(cached)) return cached.data;

  try {
    const data = await getDetails(type, tmdbId);
    const releaseDate = data.release_date || data.first_air_date;
    const year = releaseDate ? parseInt(releaseDate.split("-")[0], 10) : null;
    const title = data.name || data.title;
    
    const info = {
      title,
      year: isNaN(year) ? null : year,
      originalTitle: data.original_name || data.original_title,
      type
    };

    cache.details.set(key, { data: info, ts: Date.now() });
    return info;
  } catch (e) {
    console.error(`[TMDB] Error fetching info for ${tmdbId}:`, e.message);
    return { title: null, year: null, type };
  }
}

async function getExternalIds(type, tmdbId) {
  const normalizedType = type === 'series' ? 'tv' : type;
  const key = `ext:${normalizedType}:${tmdbId}`;
  const cached = cache.imdbByTmdb.get(key);
  if (isFresh(cached)) return cached.data;

  const data = await tmdbFetch(`/${normalizedType}/${tmdbId}/external_ids`);
  cache.imdbByTmdb.set(key, { data, ts: Date.now() });
  return data;
}

async function getDetails(type, tmdbId) {
  const normalizedType = type === 'series' ? 'tv' : type;
  const key = `details:${normalizedType}:${tmdbId}`;
  const cached = cache.details.get(key);
  if (isFresh(cached)) return cached.data;

  const data = await tmdbFetch(`/${normalizedType}/${tmdbId}`);
  cache.details.set(key, { data, ts: Date.now() });
  return data;
}

async function getSeasonDetails(tmdbId, season) {
  const key = `season:${tmdbId}:${season}`;
  const cached = cache.seasons.get(key);
  if (isFresh(cached)) return cached.data;

  const data = await tmdbFetch(`/tv/${tmdbId}/season/${season}`);
  cache.seasons.set(key, { data, ts: Date.now() });
  return data;
}

async function search(query, type = 'multi', year = null) {
  const key = `search:${type}:${query}:${year || 'all'}`;
  const cached = cache.search.get(key);
  if (isFresh(cached)) return cached.data;

  let endpoint = `/search/${type}`;
  const params = { query };
  
  if (year) {
    if (type === 'tv') params.first_air_date_year = year;
    else if (type === 'movie') params.primary_release_year = year;
    else params.year = year;
  }
  
  try {
    const data = await tmdbFetch(endpoint, params);
    const results = data.results || [];
    cache.search.set(key, { data: results, ts: Date.now() });
    return results;
  } catch (e) {
    return [];
  }
}

async function getPopular(type = 'movie', page = 1) {
  try {
    const data = await tmdbFetch(`/${type}/popular`, { page });
    return data.results || [];
  } catch (e) {
    return [];
  }
}

async function getNowPlaying(page = 1) {
  try {
    const data = await tmdbFetch('/movie/now_playing', { page });
    return data.results || [];
  } catch (e) {
    return [];
  }
}

async function getTrending(mediaType = 'all', timeWindow = 'day', page = 1) {
  try {
    const data = await tmdbFetch(`/trending/${mediaType}/${timeWindow}`, { page });
    return data.results || [];
  } catch (e) {
    return [];
  }
}

async function resolveImdbId(type, tmdbId) {
  try {
    const ext = await getExternalIds(type, tmdbId);
    return ext.imdb_id || null;
  } catch {
    return null;
  }
}

module.exports = { 
  getTmdbInfo,
  getDetails, 
  getExternalIds, 
  getSeasonDetails,
  resolveImdbId, 
  search, 
  getPopular,
  getNowPlaying,
  getTrending
};
