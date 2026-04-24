const logger = require('./logger');

const FAILURE_THRESHOLD = 3; // Number of consecutive failures before tripping
const COOLDOWN_DURATION = 10 * 60 * 1000; // 10 minutes cooldown

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
        s.disabledUntil = Date.now() + COOLDOWN_DURATION;
        logger.warn(`[CircuitBreaker] Provider ${name} tripped! Disabled for ${COOLDOWN_DURATION / 60000} mins`);
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
