/**
 * Hue Stream API
 * Professional Streaming Aggregator
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */

const TTLCache = require('ttl-cache');

// L1: in-memory, 60-min TTL — fast sync reads, survives no restart
const L1_TTL = 60 * 60 * 1000;              // 60 min (half of Redis 4h TTL)
const l1 = new TTLCache({ ttl: L1_TTL, max: 5000 });

// L2: Redis — shared between Node.js and Rust engine, survives restarts
const REDIS_TTL_SEC = 4 * 60 * 60; // 4 hours
let redis = null;

if (process.env.REDIS_URL) {
    const Redis = require('ioredis');
    redis = new Redis(process.env.REDIS_URL, {
        lazyConnect: true,
        maxRetriesPerRequest: 1,
        enableOfflineQueue: false,
    });
    redis.connect().catch(e => {
        console.warn('[cache] Redis connect failed, using memory-only cache:', e.message);
        redis = null;
    });
    redis.on('error', e => console.warn('[cache] Redis error:', e.message));
}

function cacheKey(type, tmdbId, season, episode) {
    return `hue:streams:${type}:${tmdbId}:${season || 0}:${episode || 0}`;
}

async function getCachedStreams(type, tmdbId, season, episode) {
    const key = cacheKey(type, tmdbId, season, episode);

    // L1 hit — no serialization cost
    const hot = l1.get(key);
    if (hot) return hot;

    // L2 hit — deserialize and backfill L1
    if (redis) {
        try {
            const raw = await redis.get(key);
            if (raw) {
                const streams = JSON.parse(raw);
                l1.set(key, streams);
                return streams;
            }
        } catch (_) {}
    }
    return null;
}

async function setCachedStreams(type, tmdbId, season, episode, streams) {
    const key = cacheKey(type, tmdbId, season, episode);
    l1.set(key, streams);
    if (redis) {
        try { await redis.set(key, JSON.stringify(streams), 'EX', REDIS_TTL_SEC); } catch (_) {}
    }
}

async function addCachedStreams(type, tmdbId, season, episode, newStreams) {
    const key = cacheKey(type, tmdbId, season, episode);

    // Read existing from L1 first (avoids Redis round-trip on hot paths)
    let existing = l1.get(key);
    if (!existing && redis) {
        try {
            const raw = await redis.get(key);
            if (raw) existing = JSON.parse(raw);
        } catch (_) {}
    }

    const urlMap = new Map();
    (existing || []).forEach(s => { if (s.url) urlMap.set(s.url, s); });
    newStreams.forEach(s => { if (s.url) urlMap.set(s.url, s); });
    const merged = Array.from(urlMap.values());

    l1.set(key, merged);
    if (redis) {
        try { await redis.set(key, JSON.stringify(merged), 'EX', REDIS_TTL_SEC); } catch (_) {}
    }
}

function clearCache() {
    l1.flush();
    if (redis) redis.flushdb().catch(() => {});
}

module.exports = {
    getCachedStreams,
    setCachedStreams,
    addCachedStreams,
    clearCache,
};
