const { chromium } = require('playwright');

/**
 * Resolves the best m3u8 URL by surfing the page in a headless browser.
 */
async function resolveWithPlaywright(targetUrl, referer = null) {
    let browser;
    try {
        browser = await chromium.launch({
            headless: true,
            args: [
                '--no-sandbox', 
                '--disable-setuid-sandbox',
                '--disable-blink-features=AutomationControlled',
            ]
        });
        
        const context = await browser.newContext({
            userAgent: 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36',
            viewport: { width: 1280, height: 720 }
        });

        const page = await context.newPage();
        
        const foundStreams = [];

        // Monitor all network requests
        page.on('request', request => {
            const url = request.url();
            if (url.includes('.m3u8')) {
                foundStreams.push({
                    url: url,
                    headers: request.headers(),
                    priority: url.includes('4k') ? 4 : url.includes('1080') ? 3 : url.includes('720') ? 2 : 1
                });
            }
        });

        console.log(`Playwright surfing: ${targetUrl}`);
        
        // Navigate and wait for some network activity
        // User requested no timeouts, so we set timeout to 0 (infinite)
        // We also use 'domcontentloaded' as it's faster than 'load' and usually sufficient for scripts to start
        await page.goto(targetUrl, { 
            waitUntil: 'domcontentloaded', 
            timeout: 0,
            referer: referer || 'https://fbstreams.pm/'
        });

        // Aggressive surfing: Try to trigger the player
        const interactionSelectors = ['#player', 'video', '.play-button', 'button', 'canvas', '.ratio-16x9', 'body'];
        
        for (let i = 0; i < 3; i++) { // 3 attempts at interaction
            if (foundStreams.length > 0 && i > 0) break; // Stop if we found something
            
            console.log(`Surfing attempt ${i + 1}...`);
            for (const selector of interactionSelectors) {
                try {
                    const el = await page.$(selector);
                    if (el) {
                        await el.click({ timeout: 1000, force: true });
                        await page.waitForTimeout(1000);
                    }
                } catch (e) {}
            }
            await page.waitForTimeout(2000);
        }

        // Final wait to capture any delayed requests
        if (foundStreams.length === 0) {
            await page.waitForTimeout(5000);
        }

        if (foundStreams.length > 0) {
            // Sort by priority (highest first)
            foundStreams.sort((a, b) => b.priority - a.priority);
            
            // Return unique streams
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

            return {
                type: 'stream_list',
                streams: uniqueStreams
            };
        }

        return {
            type: 'page',
            error: 'No m3u8 found after surfing'
        };

    } catch (error) {
        console.error('Playwright Error:', error.message);
        return { error: error.message };
    } finally {
        if (browser) await browser.close();
    }
}

module.exports = { resolveWithPlaywright };