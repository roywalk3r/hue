/**
 * Secure Logger Module
 * Provides configurable verbosity, colorized output, and automatic PII/Secret masking.
 * 
 * @module utils/logger
 */

const fs = require('fs');
const path = require('path');

// Configuration
const LOG_LEVEL = process.env.LOG_LEVEL || 'INFO'; // DEBUG, INFO, WARN, ERROR
const LOG_FILE = path.join(process.cwd(), 'app.log');

const LEVELS = {
    DEBUG: 0,
    INFO: 1,
    WARN: 2,
    ERROR: 3,
    NONE: 4
};

const COLORS = {
    DEBUG: '\x1b[36m', // Cyan
    INFO: '\x1b[32m',  // Green
    WARN: '\x1b[33m',  // Yellow
    ERROR: '\x1b[31m', // Red
    RESET: '\x1b[0m',
    GRAY: '\x1b[90m'
};

// Secrets to mask (loaded dynamically)
const SECRETS = new Set();

/**
 * Register a secret string to be automatically masked in logs.
 * @param {string} secret - The secret string to hide.
 */
function registerSecret(secret) {
    if (secret && typeof secret === 'string' && secret.length > 5) {
        SECRETS.add(secret);
    }
}

/**
 * Masks IP addresses and registered secrets in a string.
 * @param {string} str - The raw log string.
 * @returns {string} - The sanitized string.
 */
function mask(str) {
    if (typeof str !== 'string') return str;
    
    let sanitized = str;

    // Mask registered secrets
    for (const secret of SECRETS) {
        sanitized = sanitized.split(secret).join('***REDACTED***');
    }

    // Mask IPv4 Addresses (Simple Anonymization)
    // Matches x.x.x.x and replaces last octet with ***
    sanitized = sanitized.replace(/(\d{1,3}\.\d{1,3}\.\d{1,3})\.\d{1,3}/g, '$1.***');

    // Mask API Keys in URLs (common pattern)
    sanitized = sanitized.replace(/(api_key=)[^&]+/, '$1***KEY***');
    sanitized = sanitized.replace(/(token=)[^&]+/, '$1***TOKEN***');

    return sanitized;
}

/**
 * Internal log handler.
 * @param {string} level - Log level key.
 * @param {any[]} args - Arguments to log.
 */
function log(level, ...args) {
    if (LEVELS[level] < LEVELS[LOG_LEVEL]) return;

    const timestamp = new Date().toISOString();
    const message = args.map(arg => 
        typeof arg === 'object' ? JSON.stringify(arg) : String(arg)
    ).join(' ');

    const maskedMessage = mask(message);
    const logLine = `[${timestamp}] [${level}] ${maskedMessage}`;

    // Console output
    console.log(`${COLORS[level] || ''}${logLine}${COLORS.RESET}`);

    // File output (strip colors)
    try {
        fs.appendFileSync(LOG_FILE, logLine + '\n');
    } catch (e) {
        // Fail silently if FS breaks
    }
}

module.exports = {
    debug: (...args) => log('DEBUG', ...args),
    info: (...args) => log('INFO', ...args),
    warn: (...args) => log('WARN', ...args),
    error: (...args) => log('ERROR', ...args),
    registerSecret
};
