/**
 * HUE ENGINE ❄
 * System Designation: The Architecture of Aggregation
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */

/**
 * HiTV Library — Unified client for HiTV streaming platform
 *
 * Encapsulates:
 *   • Direct API (home.hitv.vip) via Playwright — gallery, episodes, streams
 *   • Legacy API (aoneroom) — browse, search, detail, library build
 *   • AES-128-CBC decryption, taxonomy, caching, TMDB enrichment
 *
 * Usage:
 *   const hitv = require('./utils/hitv');
 *   const series = await hitv.gallery({ location: 5, genre: 3, page: 1 });
 *   const episodes = await hitv.episodes(seriesId);
 *   const streams = await hitv.streams(episodeId);
 */

const { chromium } = require('playwright');
const axios = require('axios');
const crypto = require('crypto');
const fs = require('fs').promises;
const path = require('path');
const tmdb = require('./tmdb');

// ─── Constants ───────────────────────────────────────────────────────────────

const DIRECT_BASE = 'https://home.hitv.vip';
const LEGACY_BASE = 'https://h5-api.aoneroom.com/wefeed-h5api-bff';
const SALT = 'Wcb26arWkvkcAZc378eR';

const TAXONOMY = require('../hitv_taxonomy.json');

// ─── Direct API: Playwright + Decryption ────────────────────────────────────

let _browser = null;
let _context = null;
let _contextReady = false;

async function _ensureContext() {
  if (!_browser) _browser = await chromium.launch({ headless: true });
  if (!_context) _context = await _browser.newContext();
  if (!_contextReady) {
    const pg = await _context.newPage();
    await pg.goto(DIRECT_BASE, { waitUntil: 'domcontentloaded', timeout: 15000 });
    await pg.waitForTimeout(2000);
    await pg.close();
    _contextReady = true;
  }
  return _context;
}

function _md5(s) {
  return crypto.createHash('md5').update(String(s)).digest('hex');
}

/**
 * Generate sign for rslv endpoint.
 * Algorithm reverse-engineered from HiTV Nuxt chunk 65:
 *   sign = md5("eid=<eid>&scid=<scid>&sid=<sid>&sq=<sq>&appkey=<appkey>")
 *
 * Set HITV_APPKEY env var to override the default if HiTV rotates it.
 * The appkey lives in HiTV's webpack chunk 55, module 1680:
 *   function(){return Object(n.b)("<value>")}
 */
function _generateSign(eid, sid, scid, sq, appkey = null) {
  const key = appkey || process.env.HITV_APPKEY || 'bywebabcd1234';
  const raw = `eid=${eid}&scid=${scid}&sid=${sid}&sq=${sq}&appkey=${key}`;
  return _md5(raw);
}

let _cachedAppkey = null;

/**
 * Dynamically extract the current appkey from HiTV's CDN.
 * Loads a series page via Playwright, intercepts chunk requests, and
 * extracts the appkey from module 1680 (base64-encoded constant).
 * Result is cached in-memory so subsequent calls are instant.
 *
 * Set HITV_APPKEY env var to skip the fetch entirely.
 */
async function _fetchAppkeyFromCDN() {
  if (_cachedAppkey) return _cachedAppkey;
  try {
    const ctx = await _ensureContext();
    const pg = await ctx.newPage();

    let appkey = null;

    // Intercept chunk responses before navigation
    pg.route(/hi_p_res\/5[0-9]\..*\.js$/, async (route) => {
      const resp = await route.fetch();
      const js = await resp.text();
      // Extract module 1680: find "1680:function" then "Object(n.b)("..." )"
      const modHead = js.indexOf('1680:function');
      if (modHead >= 0) {
        // Search for Object(n.b)("...") within the next 300 chars
        const chunk = js.substring(modHead, modHead + 500);
        const m = chunk.match(/Object\(n\.b\)\("([^"]+)"\)/);
        if (m) appkey = m[1];
      }
      route.fulfill({ body: js, headers: resp.headers() });
    });

    // Navigate to a series page to trigger chunk loads
    await pg.goto(DIRECT_BASE + '/series/perfect-crown', {
      waitUntil: 'domcontentloaded', timeout: 15000,
    });
    await pg.waitForTimeout(8000);
    await pg.close();

    if (appkey) _cachedAppkey = appkey;
    return appkey;
  } catch {
    return null;
  }
}

function _decrypt(data, ts, did) {
  const inner = _md5(did + ts);
  const outer = _md5(inner + SALT);
  const key = outer.slice(0, 16);
  const iv = outer.slice(16);
  try {
    const decipher = crypto.createDecipheriv('aes-128-cbc', key, iv);
    let decrypted = decipher.update(Buffer.from(data, 'base64'));
    decrypted = Buffer.concat([decrypted, decipher.final()]);
    return decrypted.toString('utf8');
  } catch {
    return null;
  }
}

function _parseEncrypted(body) {
  try {
    const json = typeof body === 'string' ? JSON.parse(body) : body;
    if (!json.ts || !json.data) return null;
    return { ts: json.ts, data: json.data };
  } catch {
    return null;
  }
}

/**
 * Fetch and decrypt any HiTV direct API endpoint.
 */
async function fetchDirect(path, did = null, extraHeaders = {}) {
  const context = await _ensureContext();
  const url = path.startsWith('http') ? path : DIRECT_BASE + path;
  const didValue = did || 'hue-visitor-' + Date.now();

  const resp = await context.request.get(url, {
    headers: {
      'User-Agent': 'Mozilla/5.0 (Linux; Android 10; K) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/134.0.0.0 Mobile Safari/537.36',
      'aid': 'hitv.pc',
      'platform': 'pc',
      'lth': 'en-US',
      'referer': DIRECT_BASE + '/gallery',
      'did': didValue,
      ...extraHeaders,
    },
  });

  const body = await resp.text();
  const enc = _parseEncrypted(body);
  if (!enc) return null;

  const decrypted = _decrypt(enc.data, String(enc.ts), didValue);
  if (!decrypted) return null;
  try { return JSON.parse(decrypted); }
  catch { return decrypted; }
}

/**
 * Browse gallery with full taxonomy support.
 */
async function gallery({
  cate = 1,
  location = 1,
  sort = 'hot',
  genre = -1,
  page = 1,
  status = -1,
  year = -1,
  did = null,
} = {}) {
  let path = `/s1/w/series/api/series/gallery?cate=${cate}&location=${location}&sort=${sort}&genre=${genre}&page=${page}`;
  if (status !== -1) path += `&status=${status}`;
  if (year !== -1) path += `&year=${year}`;
  return fetchDirect(path, did);
}

/**
 * Get episode list for a series.
 */
async function episodes(seriesId, did = null) {
  return fetchDirect(`/s1/w/series/api/series/episodes?sid=${seriesId}`, did);
}

/**
 * Get stream URLs for an episode.
 * Pass sid + scid to auto-generate the sign; omit for manual sign.
 */
async function streams(eid, sid = null, scid = null, quality = 1, did = null, sign = null, sq = '1') {
  let path = `/s1/w/series/api/series/rslv?eid=${eid}&sid=${sid || ''}&scid=${scid || ''}&usageType=11&quality=${quality}&sq=${sq}`;
  const s = sign || (sid && scid ? _generateSign(eid, sid, scid, sq) : null);
  if (s) path += `&sign=${s}`;
  return fetchDirect(path, did);
}

/**
 * Get episode detail (includes stream sources).
 */
async function episodeDetail(eid, did = null) {
  return fetchDirect(`/s1/w/series/api/episode/detail?eid=${eid}`, did, {
    referer: DIRECT_BASE + '/',
  });
}

/**
 * Search HiTV directly (aggregate endpoint — results richer than `/search/search`).
 * scope: 101 = series, 102 = stars, 100 = all
 */
async function searchDirect(query, page = 1, did = null, scope = 101) {
  return fetchDirect(`/s1/w/search/api/aggregate/search?keyword=${encodeURIComponent(query)}&page=${page}&scope=${scope}`, did);
}

/**
 * Get album detail (trending collections, genre collections, etc.)
 * Returns { album: {...}, seriesList: [...], more: boolean }
 */
async function albumDetail(aid, did = null) {
  return fetchDirect(`/s1/w/routine/api/seriesAlbum/detail?aid=${aid}`, did);
}

/**
 * Get trending search hotwords.
 */
async function hotwords(did = null) {
  const data = await fetchDirect('/s1/w/routine/api/search/config', did);
  return data?.hotwords || [];
}

/**
 * Get navigation items (discovery entries, menu structure).
 */
async function navigations(did = null) {
  const data = await fetchDirect('/s1/w/routine/api/navigation/get', did);
  return data?.navigations || [];
}

/**
 * Close Playwright browser (free resources).
 */
async function closeDirect() {
  if (_context) { _context = null; _contextReady = false; }
  if (_browser) { await _browser.close(); _browser = null; }
}

// ─── Legacy API: aoneroom ───────────────────────────────────────────────────

const LEGACY_HEADERS = {
  'Content-Type': 'application/json',
  'User-Agent': 'Mozilla/5.0 (Linux; Android 10; K) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/114.0.0.0 Mobile Safari/537.36',
  'Origin': 'https://www.gohitv.com',
  'Referer': 'https://www.gohitv.com/',
};

const LEGACY_CATEGORIES = [
  { name: 'Drama', id: 2 },
  { name: 'Movie', id: 1 },
  { name: 'Variety', id: 3 },
  { name: 'Anime', id: 4 },
];

const LEGACY_COUNTRIES = [
  'All', 'United States', 'United Kingdom', 'Korea', 'Japan', 'Bangladesh', 'China',
  'Egypt', 'France', 'Germany', 'India', 'Indonesia', 'Iraq', 'Italy', 'Ivory Coast',
  'Kenya', 'Lebanon', 'Mexico', 'Morocco', 'Nigeria', 'Pakistan', 'Philippines',
  'Russia', 'Saudi Arabia', 'South Africa', 'Spain', 'Syria', 'Thailand', 'Malaysia',
  'Turkey', 'Vietnam', 'Others',
];

const LEGACY_GENRES = [
  'All', 'Action', 'Adventure', 'Animation', 'Biography', 'Comedy', 'Crime',
  'Documentary', 'Drama', 'Family', 'Fantasy', 'Film-Noir', 'Game-Show', 'History',
  'Horror', 'Music', 'Musical', 'Mystery', 'News', 'Reality-TV', 'Romance',
  'Sci-Fi', 'Short', 'Sport', 'Talk-Show', 'Thriller', 'War', 'Western', 'Wuxia', 'Xianxia',
];

const LEGACY_SORTS = ['ForYou', 'Hottest', 'Newest', 'Latest', 'Recommend', 'General'];

async function legacyBrowse({ page = 1, pageSize = 28, country = 'Korea', category = 'Drama', genre = 'All', sort = 'Hottest', year = null }) {
  const catObj = LEGACY_CATEGORIES.find(c => c.name.toLowerCase() === category.toLowerCase()) || LEGACY_CATEGORIES[0];
  const payload = {
    page: parseInt(page),
    perPage: parseInt(pageSize),
    channelId: catObj.id,
    country: (country === 'All' || !country) ? undefined : country,
    sort,
  };
  if (genre && genre.toLowerCase() !== 'all') payload.genre = genre;
  if (year) payload.year = String(year);

  try {
    const { data } = await axios.post(`${LEGACY_BASE}/subject/filter`, payload, { headers: LEGACY_HEADERS });
    if (data?.code === 0 && data.data) {
      return {
        page: data.data.pager.page,
        hasNext: data.data.pager.hasMore,
        items: data.data.items.map(item => ({
          id: item.subjectId,
          title: item.title,
          cover: item.cover?.url || null,
          rating: item.imdbRatingValue,
          releaseDate: item.releaseDate,
          genre: item.genre,
          year: item.releaseDate ? item.releaseDate.split('-')[0] : null,
          type: item.subjectType === 1 ? 'movie' : 'tv',
        })),
      };
    }
    return { items: [] };
  } catch (e) {
    return { items: [] };
  }
}

async function legacySearch(query, type = 2, page = 1) {
  const payload = { keyword: query, page: parseInt(page), perPage: 24, subjectType: type };
  try {
    const { data } = await axios.post(`${LEGACY_BASE}/subject/search`, payload, { headers: LEGACY_HEADERS });
    if (data?.code === 0 && data.data) {
      const list = data.data.items || [];
      return {
        page: data.data.pager?.page || page,
        hasNext: data.data.pager?.hasMore || false,
        items: list.map(item => ({
          id: item.subjectId,
          title: item.title,
          cover: item.cover?.url || null,
          rating: item.imdbRatingValue,
          releaseDate: item.releaseDate,
          genre: item.genre,
          type: item.subjectType === 1 ? 'movie' : 'tv',
        })),
      };
    }
    return { items: [], page, hasNext: false };
  } catch (e) {
    return { items: [], page, hasNext: false };
  }
}

async function legacyDetail(id) {
  if (!/^\d+$/.test(id)) {
    const searchRes = await legacySearch(id, 2, 1);
    if (searchRes.items?.length > 0) {
      return legacyDetail(searchRes.items[0].id);
    }
    return { id, title: 'Unknown', episodes: [] };
  }
  try {
    const { data } = await axios.get(`${LEGACY_BASE}/detail?subjectId=${id}&lang=en`, { headers: LEGACY_HEADERS });
    if (data?.code === 0 && data.data?.subject) {
      const d = data.data.subject;
      return {
        id: d.subjectId,
        title: d.title,
        description: d.description,
        cover: d.cover?.url || null,
        detailPath: d.detailPath,
        episodes: (data.data.resource?.seasons || []).flatMap(s => {
          const eps = [];
          for (let i = 1; i <= (s.maxEp || 0); i++) {
            eps.push({ number: i, title: `Episode ${i}`, id: `${d.subjectId}-${i}` });
          }
          return eps;
        }),
      };
    }
    return null;
  } catch {
    return { id, title: 'Unknown', episodes: [] };
  }
}

// ─── TMDB Enrichment ─────────────────────────────────────────────────────────

const MAPPING_CACHE_FILE = path.join(__dirname, '../hitv_mapping_cache.json');
let mappingCache = {};

(async () => {
  try {
    const raw = await fs.readFile(MAPPING_CACHE_FILE, 'utf8');
    mappingCache = JSON.parse(raw);
  } catch {}
})();

async function _saveMappingCache() {
  try { await fs.writeFile(MAPPING_CACHE_FILE, JSON.stringify(mappingCache)); } catch {}
}

function _cleanTitle(title) {
  return title
    ?.replace(/\s*\(\d{4}\)/g, '')
    ?.replace(/\s*Season\s*\d+/gi, '')
    ?.replace(/\s*S\d+/gi, '')
    ?.replace(/\s*-\s*Ep.*/gi, '')
    ?.trim() || '';
}

async function enrich(items, type = 'tv') {
  const enriched = [];
  let hasNewMappings = false;

  for (const item of items) {
    const cached = mappingCache[item.id];
    if (cached && cached.enriched !== false) {
      enriched.push({ ...item, ...cached });
      continue;
    }

    try {
      const results = await tmdb.search(_cleanTitle(item.title), type, item.year);
      if (results?.length > 0) {
        const m = results[0];
        const data = {
          tmdbId: m.id,
          overview: m.overview,
          rating: m.vote_average ? m.vote_average.toString() : item.rating,
          enriched: true,
        };
        if (m.backdrop_path) {
          data.backdrop = `https://image.tmdb.org/t/p/original${m.backdrop_path}`;
          data.cover = `https://image.tmdb.org/t/p/w780${m.poster_path}`;
        } else if (m.poster_path) {
          data.cover = `https://image.tmdb.org/t/p/w780${m.poster_path}`;
        }
        mappingCache[item.id] = data;
        hasNewMappings = true;
        enriched.push({ ...item, ...data });
      } else {
        mappingCache[item.id] = { enriched: false };
        hasNewMappings = true;
        enriched.push(item);
      }
    } catch {
      enriched.push(item);
    }
  }

  if (hasNewMappings) _saveMappingCache();
  return enriched;
}

// ─── Unified Exports ─────────────────────────────────────────────────────────

module.exports = {
  // Taxonomy
  taxonomy: TAXONOMY,

  // Direct API
  gallery,
  episodes,
  streams,
  episodeDetail,
  searchDirect,
  albumDetail,
  hotwords,
  navigations,
  fetchDirect,
  closeDirect,

  // Legacy API
  browse: legacyBrowse,
  search: legacySearch,
  detail: legacyDetail,

  // Enrichment
  enrich,

  // Sign generation (cracked from Nuxt chunk 65)
  generateSign: _generateSign,
  fetchAppkey: _fetchAppkeyFromCDN,

  // Constants
  DIRECT_BASE,
  LEGACY_BASE,
  LEGACY_CATEGORIES,
  LEGACY_COUNTRIES,
  LEGACY_GENRES,
  LEGACY_SORTS,
};
