const TTLCache = require('ttl-cache');

// Cache results for 1 hour
const cache = new TTLCache({
    ttl: 60 * 60 * 1000,
    max: 100 // Store last 100 movie/series lookups
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

module.exports = {
    getCachedStreams,
    setCachedStreams
};