/**
 * Hue Stream API
 * Professional Streaming Aggregator
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */

const { getPopular, resolveImdbId } = require('../utils/tmdb');
const { getProvider, listProviders } = require('../providers/registry');
const { setCachedStreams, getCachedStreams } = require('../utils/cache');
const logger = require('../utils/logger');
const { config } = require('../utils/config');

const WARMUP_INTERVAL = 1 * 60 * 60 * 1000; // 1 hour
const MAX_CONCURRENT_WARMUP = 2; // Keep it low to not impact users

async function warmupMedia(item, type) {
    const tmdbId = item.id;
    const mediaType = type === 'movie' ? 'movie' : 'series';
    
    // Skip if already cached
    const existing = getCachedStreams(mediaType, tmdbId, 1, 1);
    if (existing && existing.length > 0) return;

    logger.info(`[Warmup] Pre-scraping ${type}: ${item.title || item.name} (${tmdbId})`);

    const providers = listProviders()
        .map(p => getProvider(p.name))
        .filter(p => p.enabled && p.isFast); // Only use fast providers for warmup

    const imdbId = await resolveImdbId(type, tmdbId);

    // Run providers for this item
    for (const prov of providers) {
        try {
            await prov.fetch({ 
                tmdbId, 
                type: mediaType, 
                season: 1, 
                episode: 1, 
                imdbId, 
                filters: {} 
            });
        } catch (e) {}
    }
}

async function runWarmup() {
    logger.info('[Warmup] Starting background warmup...');
    
    try {
        const movies = await getPopular('movie', 1);
        const shows = await getPopular('tv', 1);
        
        const currentYear = new Date().getFullYear() - 1;

        // Stricter filter: Must have a release date AND it must be in the past
        const topItems = [
            ...movies.map(i => ({ ...i, type: 'movie' })),
            ...shows.map(i => ({ ...i, type: 'tv' }))
        ].filter(item => {
            const releaseStr = item.release_date || item.first_air_date;

            // 1. Metadata Date Check
            if (!releaseStr) return false;
            const releaseDate = new Date(releaseStr);
            if (releaseDate.getFullYear() > currentYear) return false;

            // 3. One-day old check
            const yesterday = new Date();
            yesterday.setDate(yesterday.getDate() - 1);
            return releaseDate < yesterday;
        }).slice(0, 20);

        logger.info(`[Warmup] Filtered ${topItems.length} valid items out of 40 popular titles.`);

        // Process sequentially to be gentle on CPU
        for (const item of topItems) {
            await warmupMedia(item, item.type);
            // Small gap between items
            await new Promise(r => setTimeout(r, 5000));
        }

        logger.info('[Warmup] Background warmup completed.');
    } catch (err) {
        logger.error(`[Warmup] Error: ${err.message}`);
    }
}

function startWarmupWorker() {
    if (process.env.DISABLE_WARMUP === 'true') return;
    
    logger.info('[Warmup] Initialized.');
    
    // Initial delay to let the server start up fully
    setTimeout(runWarmup, 60000);

    // Schedule periodic runs
    setInterval(runWarmup, WARMUP_INTERVAL);
}

module.exports = { startWarmupWorker };
