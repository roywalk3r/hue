const { chromium } = require("playwright-extra");
const StealthPlugin = require("puppeteer-extra-plugin-stealth");
const { buildLaunchOptions } = require("./browserConfig");

// Use stealth plugin
chromium.use(StealthPlugin());

let browserInstance = null;
let activeRequests = 0;
const MAX_CONCURRENT_PAGES = 2; // Strict limit for low-resource VPS

async function getBrowser() {
    if (!browserInstance) {
        console.log("[BrowserManager] Launching new browser instance...");
        browserInstance = await chromium.launch(buildLaunchOptions());
        
        browserInstance.on('disconnected', () => {
            console.log("[BrowserManager] Browser disconnected.");
            browserInstance = null;
        });
    }
    return browserInstance;
}

async function getNewContext(options = {}) {
    const browser = await getBrowser();
    
    // Wait if too many pages are active
    while (activeRequests >= MAX_CONCURRENT_PAGES) {
        await new Promise(r => setTimeout(r, 1000));
    }
    
    activeRequests++;
    
    const defaultOptions = {
        userAgent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
        viewport: { width: 1280, height: 720 }
    };
    
    const context = await browser.newContext({ ...defaultOptions, ...options });
    
    // Wrap close to decrement active count
    const originalClose = context.close.bind(context);
    context.close = async () => {
        activeRequests--;
        return await originalClose();
    };
    
    return context;
}

async function getNewPage(options = {}) {
    const context = await getNewContext(options);
    const page = await context.newPage();
    
    // CPU Optimization: Block heavy/unnecessary resources
    await page.route('**/*', (route) => {
        const type = route.request().resourceType();
        const url = route.request().url();
        
        // Block images, fonts, and styles unless explicitly needed
        if (['image', 'font', 'stylesheet', 'media'].includes(type)) {
            // Allow media only if it's a manifest/stream we need to intercept
            if (url.includes('.m3u8') || url.includes('.mp4')) {
                return route.continue();
            }
            return route.abort();
        }
        
        // Block common ad/analytics domains
        if (url.includes('google-analytics') || url.includes('doubleclick') || url.includes('adsystem') || url.includes('analytics')) {
            return route.abort();
        }
        
        route.continue();
    });
    
    return page;
}

async function closeBrowser() {
    if (browserInstance) {
        await browserInstance.close();
        browserInstance = null;
        activeRequests = 0;
    }
}

module.exports = {
    getBrowser,
    getNewContext,
    getNewPage,
    closeBrowser
};