/**
 * Hue Stream API
 * Professional Streaming Aggregator
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */

const { chromium, firefox } = require("playwright-extra");
const StealthPlugin = require("puppeteer-extra-plugin-stealth");
const { buildLaunchOptions } = require("./browserConfig");
const { AD_DOMAINS } = require("./interceptor");

// Use stealth plugin for Chromium only
chromium.use(StealthPlugin());

let browserInstance = null;
let firefoxInstance = null;
let launchMutex = Promise.resolve();

// Semaphore for concurrency control
const MAX_GLOBAL_CONCURRENT_PAGES = process.env.MAX_CONCURRENT_PAGES ? parseInt(process.env.MAX_CONCURRENT_PAGES) : 10;
const queue = [];
let activePages = 0;

async function acquirePageSlot() {
    if (activePages < MAX_GLOBAL_CONCURRENT_PAGES) {
        activePages++;
        return;
    }
    await new Promise(resolve => queue.push(resolve));
    activePages++;
}

function releasePageSlot() {
    activePages--;
    if (queue.length > 0) {
        const next = queue.shift();
        next();
    }
}

async function getBrowser(type = 'chromium') {
    // Lock the launch process so only one browser spawns at a time
    return new Promise((resolve, reject) => {
        launchMutex = launchMutex.then(async () => {
            try {
                const browser = await _getBrowserInternal(type);
                resolve(browser);
            } catch (e) {
                reject(e);
            }
        });
    });
}

async function _getBrowserInternal(type = 'chromium') {
    const headless = process.env.BROWSER_HEADLESS !== 'false';

    if (type === 'firefox') {
        if (!firefoxInstance) {
            console.log(`[BrowserManager] Launching Firefox (Headless: ${headless})...`);
            firefoxInstance = await firefox.launch({ headless });
            firefoxInstance.on('disconnected', () => { firefoxInstance = null; });
        }
        return firefoxInstance;
    }

    if (!browserInstance) {
        console.log(`[BrowserManager] Launching Optimized Chromium (Headless: ${headless})...`);
        const baseOptions = buildLaunchOptions();
        // Force headless based on env if not already set in baseOptions
        if (baseOptions.headless === undefined) {
            baseOptions.headless = headless;
        }

        const extraArgs = [
            '--disable-canvas-aa',
            '--disable-2d-canvas-clip-aa',
            '--disable-gl-drawing-for-tests',
            '--disable-dev-shm-usage',
            '--no-zygote',
            '--use-gl=swiftshader',
            '--disable-software-rasterizer',
            '--mute-audio',
            '--disable-extensions',
            '--disable-background-networking',
            '--disable-background-timer-throttling',
            '--disable-backgrounding-occluded-windows',
            '--disable-breakpad',
            '--disable-component-extensions-with-background-pages',
            '--disable-ipc-flooding-protection',
            '--disable-renderer-backgrounding',
            '--disable-notifications',
            '--disable-popup-blocking',
            '--no-first-run',
            '--no-default-browser-check',
            '--font-render-hinting=none'
        ];
        const mergedArgs = Array.from(new Set([...(baseOptions.args || []), ...extraArgs]));
        browserInstance = await chromium.launch({ ...baseOptions, args: mergedArgs });
        browserInstance.on('disconnected', () => { browserInstance = null; });
    }
    return browserInstance;
}

function pickRotatedProxy(config) {
    if (!config.proxyList || config.proxyList.length === 0) return undefined;
    const url = config.proxyList[Math.floor(Math.random() * config.proxyList.length)];
    console.log(`[BrowserManager] Using rotated proxy: ${url.split('@').pop()}`);
    // Playwright needs { server, username, password } — the user:pass@host:port
    // form fails with ERR_INVALID_AUTH_CREDENTIALS in current versions.
    try {
        const u = new URL(url);
        const out = { server: `${u.protocol}//${u.host}` };
        if (u.username) out.username = decodeURIComponent(u.username);
        if (u.password) out.password = decodeURIComponent(u.password);
        return out;
    } catch (_) {
        return { server: url };
    }
}

async function getNewPage(options = {}) {
    const browserType = options.browserType || 'chromium';
    const { config } = require('./config');

    // Acquire lock before launching context
    await acquirePageSlot();

    try {
        const browser = await getBrowser(browserType);

        const defaultOptions = {
            userAgent: browserType === 'firefox'
                ? 'Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:121.0) Gecko/20100101 Firefox/121.0'
                : "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
            viewport: { width: 1280, height: 720 },
            proxy: options.proxy || pickRotatedProxy(config)
        };
        
        const context = await browser.newContext({ ...defaultOptions, ...options });
        const page = await context.newPage();
        
        // Resource blocking logic
        if (browserType === 'chromium') {
            await page.route('**/*', (route) => {
                const type = route.request().resourceType();
                const url = route.request().url();
                
                // Allow essential scripts for Turnstile/Cloudflare
                if (url.includes('challenges.cloudflare.com') || url.includes('turnstile')) {
                    return route.continue();
                }

                // Allow media
                if (url.includes('.m3u8') || url.includes('.mp4') || url.includes('.ts')) return route.continue();
                
                // Block heavy types: images, fonts, and stylesheets
                // (Stylesheet blocking was previously removed but caused log spam)
                if (['image', 'font', 'stylesheet'].includes(type)) return route.abort();
                
                // Block ads
                if (AD_DOMAINS.some(domain => url.includes(domain))) return route.abort();
                
                route.continue();
            });
        }
        
        // Override close to release slot
        const originalClose = context.close.bind(context);
        let closed = false;
        context.close = async () => {
            if (closed) return;
            closed = true;
            try {
                await originalClose();
            } finally {
                releasePageSlot();
            }
        };
        
        return page;
    } catch (e) {
        releasePageSlot();
        throw e;
    }
}

async function closeBrowser() {
    if (browserInstance) { await browserInstance.close(); browserInstance = null; }
    if (firefoxInstance) { await firefoxInstance.close(); firefoxInstance = null; }
}

async function getNewContext(options = {}) {
    const browserType = options.browserType || 'chromium';
    const { config } = require('./config');

    // Auto-proxy rotation
    const proxy = options.proxy || pickRotatedProxy(config);

    await acquirePageSlot(); 
    try {
        const browser = await getBrowser(browserType);
        const defaultOptions = {
            userAgent: browserType === 'firefox' 
                ? 'Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:121.0) Gecko/20100101 Firefox/121.0'
                : "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
            viewport: { width: 1280, height: 720 },
            proxy: proxy
        };
        const context = await browser.newContext({ ...defaultOptions, ...options });
        
        const originalClose = context.close.bind(context);
        let closed = false;
        context.close = async () => {
            if (closed) return;
            closed = true;
            try {
                await originalClose();
            } finally {
                releasePageSlot();
            }
        };
        return context;
    } catch(e) {
        releasePageSlot();
        throw e;
    }
}

module.exports = { getBrowser, getNewPage, getNewContext, closeBrowser };
