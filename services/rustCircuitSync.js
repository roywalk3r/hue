/**
 * Hue Stream API
 * Professional Streaming Aggregator
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */

const { recordFailure, recordSuccess } = require('../utils/providerCircuitBreaker');
const logger = require('../utils/logger');

const RUST_URL = process.env.RUST_ENGINE_URL || 'http://127.0.0.1:24609';
const SYNC_INTERVAL = 60 * 1000;

// Map Rust provider real_name → Node.js circuit breaker name
const RUST_TO_NODE_NAME = {
    'moviebox': 'moviebox',
    'sflix':    'sflix',
    'vixsrc':   'vixsrc',
    'kisskh':   'kisskh',
    'dramacool':'dramacool',
};

async function syncOnce() {
    try {
        const resp = await fetch(`${RUST_URL}/api/providers`, {
            signal: AbortSignal.timeout(4000),
        });
        if (!resp.ok) return;

        const providers = await resp.json();
        if (!Array.isArray(providers)) return;

        for (const p of providers) {
            const nodeName = RUST_TO_NODE_NAME[p.name] || p.name;
            if (p.is_healthy === false || p.isHealthy === false) {
                // Trip Node.js breaker to match Rust's view
                recordFailure(nodeName);
                recordFailure(nodeName);
                recordFailure(nodeName);
                logger.info(`[CircuitSync] Provider ${nodeName} marked unhealthy via Rust`);
            } else if (p.is_healthy === true || p.isHealthy === true) {
                recordSuccess(nodeName);
            }
        }
    } catch (_) {
        // Rust engine unreachable — silently skip
    }
}

function startCircuitSync() {
    if (!process.env.RUST_ENGINE_URL && !process.env.ENABLE_HUERUST_PROVIDER) return;
    logger.info('[CircuitSync] Started — syncing Rust circuit state every 60s');
    syncOnce();
    setInterval(syncOnce, SYNC_INTERVAL);
}

module.exports = { startCircuitSync };
