/**
 * HUE ENGINE
 * KissKH KKey Cache — Pre-fetch and cache episode kkeys to avoid browser overhead
 *
 * KKeys are deterministic per episode ID. Once captured via Playwright interception,
 * they can be reused indefinitely for direct API calls without a browser.
 */

const fs = require('fs');
const path = require('path');
const { getNewPage, closeBrowser } = require('./browserManager');

const BASE_URL = "https://kisskh.ovh";

const CACHE_DIR = path.join(process.cwd(), 'providers', '.cache');
const CACHE_FILE = path.join(CACHE_DIR, 'kisskh-kkeys.json');
const CACHE_TTL_MS = 7 * 24 * 60 * 60 * 1000; // 7 days

let memoryCache = null;

function loadCache() {
    if (memoryCache) return memoryCache;
    try {
        if (fs.existsSync(CACHE_FILE)) {
            const raw = fs.readFileSync(CACHE_FILE, 'utf8');
            memoryCache = JSON.parse(raw);
            return memoryCache;
        }
    } catch (e) {
        console.error('[KissKHKKeyCache] Failed to load cache:', e.message);
    }
    memoryCache = {};
    return memoryCache;
}

function saveCache() {
    try {
        if (!fs.existsSync(CACHE_DIR)) fs.mkdirSync(CACHE_DIR, { recursive: true });
        fs.writeFileSync(CACHE_FILE, JSON.stringify(memoryCache, null, 2));
    } catch (e) {
        console.error('[KissKHKKeyCache] Failed to save cache:', e.message);
    }
}

function isExpired(entry) {
    if (!entry || !entry.ts) return true;
    return Date.now() - entry.ts > CACHE_TTL_MS;
}

/**
 * Get cached kkey for an episode. Returns null if not cached or expired.
 */
function getCachedKkey(episodeId) {
    const cache = loadCache();
    const entry = cache[String(episodeId)];
    if (!entry || isExpired(entry)) return null;
    return entry.kkey;
}

/**
 * Set kkey in cache.
 */
function setCachedKkey(episodeId, kkey) {
    const cache = loadCache();
    cache[String(episodeId)] = { kkey, ts: Date.now() };
    saveCache();
}

/**
 * Capture kkey for a single episode using Playwright network interception.
 *
 * @param {string|number} episodeId — KissKH episode ID (e.g. 207851)
 * @param {string|number} showId — KissKH drama/show ID (e.g. 12358)
 * @param {number} epNum — episode number (e.g. 14)
 * @returns {Promise<string|null>} — the captured kkey or null
 */
async function captureKkey(episodeId, showId, epNum = 1) {
    const epId = String(episodeId);
    const sId = String(showId);
    const eNum = String(epNum);

    console.log(`[KissKHKKeyCache] Capturing kkey for episode ${epId}...`);

    const page = await getNewPage();
    let capturedKkey = null;

    const requestHandler = (req) => {
        const url = req.url();
        if (url.includes(`/api/DramaList/Episode/${epId}`) && url.includes('kkey=')) {
            const u = new URL(url);
            capturedKkey = u.searchParams.get('kkey');
        }
    };

    page.on('request', requestHandler);

    try {
        const directUrl = `${BASE_URL}/Drama/Direct/Episode-${eNum}?id=${sId}&ep=${epId}&page=0&pageSize=100`;
        await page.goto(directUrl, { waitUntil: 'networkidle', timeout: 30000 });
        await page.waitForTimeout(3000);
    } catch (e) {
        console.error(`[KissKHKKeyCache] Browser navigation error: ${e.message}`);
    } finally {
        page.off('request', requestHandler);
        await page.context().close();
    }

    if (capturedKkey) {
        setCachedKkey(epId, capturedKkey);
        console.log(`[KissKHKKeyCache] Captured and cached kkey for episode ${epId}`);
        return capturedKkey;
    }

    console.warn(`[KissKHKKeyCache] Failed to capture kkey for episode ${epId}`);
    return null;
}

/**
 * Get kkey — returns cached value immediately, or captures via browser if missing.
 *
 * @param {string|number} episodeId
 * @param {string|number} showId
 * @param {number} epNum
 * @returns {Promise<string|null>}
 */
async function getKkey(episodeId, showId, epNum = 1) {
    const cached = getCachedKkey(episodeId);
    if (cached) {
        console.log(`[KissKHKKeyCache] Cache hit for episode ${episodeId}`);
        return cached;
    }
    return captureKkey(episodeId, showId, epNum);
}

/**
 * Warm the cache by batch-capturing kkeys for multiple episodes.
 * Episodes are processed sequentially to avoid browser overload.
 *
 * @param {Array<{episodeId, showId, epNum}>} episodes
 * @returns {Promise<Object>} — map of episodeId -> kkey
 */
async function warmCache(episodes) {
    const results = {};
    for (const { episodeId, showId, epNum } of episodes) {
        const kkey = await getKkey(episodeId, showId, epNum);
        if (kkey) results[episodeId] = kkey;
    }
    // Close the shared browser instance when done batching
    await closeBrowser();
    return results;
}

module.exports = {
    getKkey,
    captureKkey,
    getCachedKkey,
    setCachedKkey,
    warmCache
};
