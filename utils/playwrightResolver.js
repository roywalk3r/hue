const { chromium } = require('playwright');

/**
 * Resolves the best m3u8 URL by surfing the page.
 * Uses standard Playwright with manual stealth for maximum stability and speed.
 */
async function resolveWithPlaywright(targetUrl, referer = null) {
    let browser = null;
    try {
        browser = await chromium.launch({
            headless: true,
            args: [
                '--no-sandbox', 
                '--disable-setuid-sandbox',
                '--disable-blink-features=AutomationControlled',
                '--disable-dev-shm-usage',
                '--no-zygote',
                '--mute-audio',
                '--disable-extensions',
                '--disable-gpu'
            ]
        });
        
        const context = await browser.newContext({
            userAgent: 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36',
            viewport: { width: 1280, height: 720 },
            locale: 'en-US',
            timezoneId: 'America/New_York'
        });

        // Manual Stealth: Hide webdriver
        await context.addInitScript(() => {
            Object.defineProperty(navigator, 'webdriver', {
                get: () => undefined
            });
        });

        const page = await context.newPage();
        
        // Optimized Resource Blocking
        await page.route('**/*', (route) => {
            const type = route.request().resourceType();
            const url = route.request().url();
            
            // Allow essential types
            if (['document', 'script', 'xhr', 'fetch', 'stylesheet', 'image'].includes(type)) {
                return route.continue();
            }
            
            // Allow media manifests
            if (url.includes('.m3u8') || url.includes('.mp4') || url.includes('.ts')) return route.continue();
            
            // Block fonts and other heavy/useless stuff
            if (['font', 'media', 'texttrack', 'object', 'beacon', 'csp_report', 'imageset'].includes(type)) return route.abort();
            
            route.continue();
        });

        const foundStreams = [];
        let resolveStreamPromise; 
        const streamFoundPromise = new Promise(resolve => { resolveStreamPromise = resolve; });

        // Passive listener
        page.on('request', request => {
            const url = request.url();
            if (url.includes('.m3u8')) {
                foundStreams.push({
                    url: url,
                    headers: request.headers(),
                    priority: url.includes('4k') ? 4 : url.includes('1080') ? 3 : url.includes('720') ? 2 : 1
                });
                
                // Early exit on master/index
                if (url.includes('index.m3u8') || url.includes('master.m3u8') || url.includes('playlist.m3u8')) {
                    resolveStreamPromise(true);
                }
            }
        });

        console.log(`Playwright surfing: ${targetUrl}`);
        
        // Navigation
        await page.goto(targetUrl, { 
            waitUntil: 'domcontentloaded', 
            timeout: 20000,
            referer: referer || 'https://fbstreams.pm/'
        });

        // Fast interaction loop
        const interactionSelectors = ['#player', 'video', '.play-button', 'button', 'canvas', '.ratio-16x9', 'body', 'iframe'];
        const startTime = Date.now();
        const MAX_WAIT = 10000; // 10 seconds max

        const interactionLoop = async () => {
            for (let i = 0; i < 4; i++) {
                if (foundStreams.length > 0) return;
                if (Date.now() - startTime > MAX_WAIT) return;

                // console.log(`Surfing attempt ${i + 1}...`);
                for (const selector of interactionSelectors) {
                    if (foundStreams.length > 0) return;
                    try {
                        // Locate frames too
                        const frames = page.frames();
                        for (const frame of frames) {
                             const el = await frame.$(selector);
                             if (el) await el.click({ timeout: 500, force: true }).catch(() => {});
                        }
                    } catch (e) {}
                }
                await page.waitForTimeout(1000);
            }
        };

        // Wait for either the loop to finish OR a stream to be found
        await Promise.race([
            interactionLoop(),
            streamFoundPromise,
            new Promise(r => setTimeout(r, MAX_WAIT + 2000))
        ]);

        if (foundStreams.length > 0) {
            // Sort
            foundStreams.sort((a, b) => b.priority - a.priority);
            
            // Unique
            const uniqueStreams = [];
            const seenUrls = new Set();
            for (const s of foundStreams) {
                if (!seenUrls.has(s.url)) {
                    seenUrls.add(s.url);
                    uniqueStreams.push({
                        url: s.url,
                        headers: s.headers,
                        quality: s.url.includes('1080') ? '1080p' : s.url.includes('720') ? '720p' : 'Auto'
                    });
                }
            }
            
            console.log(`Successfully captured ${uniqueStreams.length} streams`);
            return { type: 'stream_list', streams: uniqueStreams };
        }

        return { type: 'page', error: 'No m3u8 found after surfing' };

    } catch (error) {
        console.error('Playwright Error:', error.message);
        return { error: error.message };
    } finally {
        if (browser) {
            try { await browser.close(); } catch(e) {}
        }
    }
}

module.exports = { resolveWithPlaywright };