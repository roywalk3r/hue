/**
 * HUE ENGINE ❄
 * System Designation: The Architecture of Aggregation
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */

/**
 * HiTV Direct API Client
 *
 * Uses Playwright to fetch and decrypt HiTV API responses.
 * Navigates to the homepage first to establish session cookies,
 * then makes direct API calls with full browser context.
 */

const { chromium } = require('playwright');
const { decrypt, parseEncryptedResponse } = require('./hitv-crypto');

const BASE_URL = 'https://home.hitv.vip';

let _browser = null;
let _context = null;
let _contextReady = false;

async function getContext() {
  if (!_browser) {
    _browser = await chromium.launch({ headless: true });
  }
  if (!_context) {
    _context = await _browser.newContext();
  }
  // Prime the context: visit homepage once to accumulate session cookies
  if (!_contextReady) {
    const pg = await _context.newPage();
    await pg.goto(BASE_URL, { waitUntil: 'domcontentloaded', timeout: 15000 });
    await pg.waitForTimeout(2000);
    await pg.close();
    _contextReady = true;
  }
  return _context;
}

async function close() {
  if (_context) { _context = null; _contextReady = false; }
  if (_browser) { await _browser.close(); _browser = null; }
}

/**
 * Fetch and decrypt a HiTV API endpoint.
 * Context is primed with session cookies from the homepage.
 */
async function fetchApi(path, did, opts = {}) {
  const context = await getContext();
  const url = path.startsWith('http') ? path : BASE_URL + path;

  const didValue = did || 'hue-visitor-' + Date.now();

  const resp = await context.request.get(url, {
    headers: {
      'User-Agent': 'Mozilla/5.0 (Linux; Android 10; K) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/134.0.0.0 Mobile Safari/537.36',
      'aid': 'hitv.pc',
      'platform': 'pc',
      'lth': 'en-US',
      'referer': BASE_URL + '/gallery',
      'did': didValue,
      ...opts.headers,
    },
  });

  const body = await resp.text();
  const encrypted = parseEncryptedResponse(body);
  if (!encrypted) return null;

  const decrypted = decrypt(encrypted.data, String(encrypted.ts), didValue);
  if (!decrypted) return null;

  try { return JSON.parse(decrypted); }
  catch { return decrypted; }
}

async function fetchGallery({ cate = 1, location = 1, sort = 'hot', genre = -1, page = 1, did } = {}) {
  const path = `/s1/w/series/api/series/gallery?cate=${cate}&location=${location}&sort=${sort}&genre=${genre}&page=${page}`;
  return fetchApi(path, did);
}

async function fetchEpisode(sid, serialNo, did) {
  return fetchApi(`/s1/w/routine/api/episode/detail?sid=${sid}&serialNo=${serialNo}`, did, {
    headers: { referer: `${BASE_URL}/series/${sid}` },
  });
}

async function fetchStreams(eid, did) {
  return fetchApi(`/s1/w/routine/api/series/rslv?eid=${eid}&usageType=11&quality=1`, did);
}

module.exports = { fetchApi, fetchGallery, fetchEpisode, fetchStreams, close, BASE_URL };
