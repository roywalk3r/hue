/**
 * Stealth & Obfuscation Module
 * Provides utilities for header randomization, user-agent rotation, 
 * and request jitter to evade bot detection.
 * 
 * @module utils/stealth
 */

const crypto = require('crypto');

// Common User Agents for rotation
const USER_AGENTS = [
    'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36',
    'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36',
    'Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:109.0) Gecko/20100101 Firefox/121.0',
    'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36'
];

// Mobile User Agents (good for bypassing Turnstile sometimes)
const MOBILE_AGENTS = [
    'Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1',
    'Mozilla/5.0 (Linux; Android 10; K) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Mobile Safari/537.36'
];

/**
 * Returns a random User-Agent string.
 * @param {boolean} mobile - Whether to request a mobile UA.
 * @returns {string} Random User-Agent.
 */
function getRandomUserAgent(mobile = false) {
    const list = mobile ? MOBILE_AGENTS : USER_AGENTS;
    return list[Math.floor(Math.random() * list.length)];
}

/**
 * Generates a realistic set of browser headers.
 * @param {object} options - Options for header generation.
 * @returns {object} Headers object.
 */
function getHeaders({ referer, mobile = false } = {}) {
    const ua = getRandomUserAgent(mobile);
    const headers = {
        'User-Agent': ua,
        'Accept': 'text/html,application/xhtml+xml,application/xml;q=0.9,image/webp,image/apng,*/*;q=0.8',
        'Accept-Language': 'en-US,en;q=0.9',
        'Connection': 'keep-alive',
        'Upgrade-Insecure-Requests': '1',
        'Sec-Fetch-Dest': 'document',
        'Sec-Fetch-Mode': 'navigate',
        'Sec-Fetch-Site': 'none',
        'Sec-Fetch-User': '?1'
    };

    if (referer) {
        headers['Referer'] = referer;
        headers['Sec-Fetch-Site'] = 'same-origin';
    }

    return headers;
}

/**
 * Introduces a random delay (jitter) to humanize request timing.
 * @param {number} min - Minimum delay in ms.
 * @param {number} max - Maximum delay in ms.
 * @returns {Promise<void>}
 */
function jitter(min = 500, max = 1500) {
    const delay = Math.floor(Math.random() * (max - min + 1)) + min;
    return new Promise(resolve => setTimeout(resolve, delay));
}

/**
 * Anonymizes an identifier (e.g., user ID or IP) using a one-way hash.
 * Useful for logging unique users without storing PII.
 * @param {string} input - The string to anonymize.
 * @returns {string} - Short hash.
 */
function anonymize(input) {
    if (!input) return 'unknown';
    return crypto.createHash('sha256').update(input).digest('hex').substring(0, 8);
}

module.exports = {
    getRandomUserAgent,
    getHeaders,
    jitter,
    anonymize
};
