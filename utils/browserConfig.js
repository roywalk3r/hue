/**
 * Hue Stream API
 * Professional Streaming Aggregator
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */

const fs = require('fs');
const path = require('path');
const os = require('os');

const DEFAULT_ARGS = [
    '--no-sandbox',
    '--disable-setuid-sandbox',
    '--disable-dev-shm-usage',
    '--disable-accelerated-2d-canvas',
    '--no-first-run',
    '--no-zygote',
    '--disable-gpu',
    '--disable-blink-features=AutomationControlled'
];

function findPlaywrightChromium() {
    if (process.env.BROWSER_EXECUTABLE_PATH || process.env.PUPPETEER_EXECUTABLE_PATH) {
        return null;
    }
    const baseDir = process.env.PLAYWRIGHT_BROWSERS_PATH
        ? path.join(process.env.PLAYWRIGHT_BROWSERS_PATH)
        : path.join(os.homedir(), '.cache', 'ms-playwright');
    if (!fs.existsSync(baseDir)) {
        return null;
    }
    const dirs = fs.readdirSync(baseDir)
        .filter(name => /^chromium-\d+$/.test(name))
        .map(name => path.join(baseDir, name))
        .sort((a, b) => {
            const va = parseInt(path.basename(a).match(/chromium-(\d+)$/)[1], 10);
            const vb = parseInt(path.basename(b).match(/chromium-(\d+)$/)[1], 10);
            return vb - va;
        });
    for (const dir of dirs) {
        const chrome = path.join(dir, 'chrome-linux64', 'chrome');
        if (fs.existsSync(chrome)) {
            return chrome;
        }
    }
    return null;
}

function parseEnvArgs(raw) {
    if (!raw || typeof raw !== 'string') return [];
    // Simple split on whitespace; trim empty tokens
    return raw.split(/\s+/).filter(Boolean);
}

function buildLaunchOptions(overrides = {}) {
    const envArgs = parseEnvArgs(process.env.BROWSER_ARGS || process.env.PUPPETEER_ARGS);
    const mergedArgs = Array.from(new Set([...DEFAULT_ARGS, ...(overrides.args || [])]));
    const executablePath =
        process.env.BROWSER_EXECUTABLE_PATH ||
        process.env.PUPPETEER_EXECUTABLE_PATH ||
        overrides.executablePath ||
        findPlaywrightChromium();
    const headlessFlag = process.env.BROWSER_HEADLESS === 'false' || process.env.PUPPETEER_HEADLESS === 'false' ? false : true;

    const opts = {
        headless: headlessFlag,
        args: [
            ...mergedArgs,
            '--log-level=3', // Fatal only
            '--silent-launch',
            '--disable-logging',
            '--no-report-upload'
        ],
        dumpio: false
    };

    if (executablePath) {
        opts.executablePath = executablePath;
    }

    return opts;
}

module.exports = { buildLaunchOptions };