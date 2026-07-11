/**
 * Hue Stream API
 * Professional Streaming Aggregator
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */

const { refreshSflixCookie } = require('../utils/refreshSflixCookie');
const { refreshMovieboxCookie } = require('../utils/refreshMovieboxCookie');
const logger = require('../utils/logger');

// Interval in milliseconds (15 minutes)
const REFRESH_INTERVAL = 15 * 60 * 1000;

let isRefreshing = false;

async function runRefresh() {
    if (isRefreshing) return;
    isRefreshing = true;

    logger.info('[TokenWorker] Starting background token refresh...');

    try {
        // Run refreshes in sequence to avoid resource spikes
        const sflixSuccess = await refreshSflixCookie();
        logger.info(`[TokenWorker] SFlix Refresh: ${sflixSuccess ? 'SUCCESS' : 'FAILED'}`);

        const movieboxSuccess = await refreshMovieboxCookie();
        logger.info(`[TokenWorker] MovieBox Refresh: ${movieboxSuccess ? 'SUCCESS' : 'FAILED'}`);

    } catch (err) {
        logger.error(`[TokenWorker] Unexpected error: ${err.message}`);
    } finally {
        isRefreshing = false;
        logger.info('[TokenWorker] Background refresh completed.');
    }
}

function startTokenWorker() {
    logger.info(`[TokenWorker] Initialized. Interval: ${REFRESH_INTERVAL / 60000} mins`);
    
    // Run once immediately on start
    runRefresh();

    // Schedule periodic runs
    setInterval(runRefresh, REFRESH_INTERVAL);
}

module.exports = { startTokenWorker };
