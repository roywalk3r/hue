/**
 * Hue Stream API
 * Professional Streaming Aggregator
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */

const logger = require('./logger');

const FAILURE_THRESHOLD = 2; // Number of consecutive failures before tripping

const stats = {};

function getStats(name) {
    if (!stats[name]) {
        stats[name] = {
            consecutiveFailures: 0,
            totalFailures: 0,
            totalSuccesses: 0,
            disabledUntil: 0
        };
    }
    return stats[name];
}

function recordSuccess(name) {
    const s = getStats(name);
    s.consecutiveFailures = 0;
    s.totalSuccesses++;
    s.disabledUntil = 0;
}

function recordFailure(name) {
    const s = getStats(name);
    s.consecutiveFailures++;
    s.totalFailures++;

    if (s.consecutiveFailures >= FAILURE_THRESHOLD) {
        // Exponential backoff: 3 min → 6 min → 12 min, capped at 15 min
        const tripCount = Math.max(0, s.consecutiveFailures - FAILURE_THRESHOLD);
        const backoffMs = Math.min(3 * 60 * 1000 * Math.pow(2, tripCount), 15 * 60 * 1000);
        s.disabledUntil = Date.now() + backoffMs;
        logger.warn(`[CircuitBreaker] Provider ${name} tripped! Cooldown ${Math.round(backoffMs / 60000)}min`);
    }
}

function isProviderHealthy(name) {
    const s = getStats(name);
    if (s.disabledUntil > Date.now()) {
        return false;
    }
    return true;
}

function getHealthSummary() {
    const summary = {};
    for (const name in stats) {
        summary[name] = {
            ...stats[name],
            isHealthy: isProviderHealthy(name),
            remainingCooldown: Math.max(0, stats[name].disabledUntil - Date.now())
        };
    }
    return summary;
}

module.exports = {
    recordSuccess,
    recordFailure,
    isProviderHealthy,
    getHealthSummary
};
