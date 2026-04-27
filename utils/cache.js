/**
 * Hue Stream API
 * Professional Streaming Aggregator
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */

const TTLCache = require('ttl-cache');

// Cache results for 4 hours
const cache = new TTLCache({
    ttl: 4 * 60 * 60 * 1000,
    max: 1000 // Store last 1000 movie/series lookups
});

function getCacheKey(type, tmdbId, season, episode) {
    return `${type}_${tmdbId}_${season || 0}_${episode || 0}`;
}

function getCachedStreams(type, tmdbId, season, episode) {
    const key = getCacheKey(type, tmdbId, season, episode);
    return cache.get(key) || null;
}

function setCachedStreams(type, tmdbId, season, episode, streams) {
    const key = getCacheKey(type, tmdbId, season, episode);
    cache.set(key, streams);
}

function addCachedStreams(type, tmdbId, season, episode, newStreams) {
    const key = getCacheKey(type, tmdbId, season, episode);
    const existing = cache.get(key) || [];
    const urlMap = new Map();
    
    // Index existing
    existing.forEach(s => { if(s.url) urlMap.set(s.url, s); });
    // Merge new (overwrite existing with same URL if any, or just add)
    newStreams.forEach(s => { if(s.url) urlMap.set(s.url, s); });
    
    cache.set(key, Array.from(urlMap.values()));
}

module.exports = {
    getCachedStreams,
    setCachedStreams,
    addCachedStreams,
    clearCache: () => cache.flush()
};
