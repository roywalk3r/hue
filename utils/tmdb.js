const fetch = (...args) => import('node-fetch').then(m => m.default(...args));
const { config } = require('./config');
const { getTmdbApiKey } = require('./tmdbKey');

const cache = {
  imdbByTmdb: new Map(),
  details: new Map(),
};
const TTL_MS = 6 * 60 * 60 * 1000;
function isFresh(entry) { return entry && (Date.now() - entry.ts) < TTL_MS; }

async function tmdbFetchJson(url) {
  const apiKey = config.tmdbApiKey || getTmdbApiKey();
  if (!apiKey) throw new Error('TMDB_API_KEY missing');
  const sep = url.includes('?') ? '&' : '?';
  const full = `${url}${sep}api_key=${apiKey}`;
  const res = await fetch(full, { timeout: 15000 });
  if (!res.ok) throw new Error(`TMDB request failed ${res.status}`);
  return res.json();
}
async function getExternalIds(type, tmdbId) {
  const key = `${type}:${tmdbId}`;
  const cached = cache.imdbByTmdb.get(key);
  if (isFresh(cached)) return cached.data;
  const json = await tmdbFetchJson(`https://api.themoviedb.org/3/${type}/${tmdbId}/external_ids`);
  cache.imdbByTmdb.set(key, { data: json, ts: Date.now() });
  return json;
}
async function getDetails(type, tmdbId) {
  const key = `${type}:${tmdbId}:details`;
  const cached = cache.details.get(key);
  if (isFresh(cached)) return cached.data;
  const json = await tmdbFetchJson(`https://api.themoviedb.org/3/${type}/${tmdbId}`);
  cache.details.set(key, { data: json, ts: Date.now() });
  return json;
}
async function search(query, type = 'multi', year = null) {
    const key = `search:${type}:${query}:${year || ''}`;
    const cached = cache.details.get(key);
    if (isFresh(cached)) return cached.data;
    
    let url = `https://api.themoviedb.org/3/search/${type}?query=${encodeURIComponent(query)}`;
    if (year) {
        if (type === 'tv') {
            url += `&first_air_date_year=${year}`;
        } else if (type === 'movie') {
            url += `&primary_release_year=${year}`;
        } else {
            url += `&year=${year}`;
        }
    }
    
    try {
        const json = await tmdbFetchJson(url);
        const results = json.results || [];
        cache.details.set(key, { data: results, ts: Date.now() });
        return results;
    } catch (e) {
        return [];
    }
}
async function resolveImdbId(type, tmdbId) {
  try { const ext = await getExternalIds(type, tmdbId); return ext.imdb_id || null; } catch { return null; }
}
module.exports = { getExternalIds, getDetails, resolveImdbId, search };