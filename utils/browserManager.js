const { chromium } = require("playwright-extra");
const StealthPlugin = require("puppeteer-extra-plugin-stealth");
const { buildLaunchOptions } = require("./browserConfig");

// Use stealth plugin
chromium.use(StealthPlugin());

let browserInstance = null;

async function getBrowser() {
    if (!browserInstance) {
        console.log("[BrowserManager] Launching new browser instance...");
        browserInstance = await chromium.launch(buildLaunchOptions());
        
        // Handle browser disconnect
        browserInstance.on('disconnected', () => {
            console.log("[BrowserManager] Browser disconnected.");
            browserInstance = null;
        });
    }
    return browserInstance;
}

async function getNewContext(options = {}) {
    const browser = await getBrowser();
    const defaultOptions = {
        userAgent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
        viewport: { width: 1280, height: 720 }
    };
    return await browser.newContext({ ...defaultOptions, ...options });
}

async function getNewPage(options = {}) {
    const context = await getNewContext(options);
    return await context.newPage();
}

async function closeBrowser() {
    if (browserInstance) {
        await browserInstance.close();
        browserInstance = null;
    }
}

module.exports = {
    getBrowser,
    getNewContext,
    getNewPage,
    closeBrowser
};