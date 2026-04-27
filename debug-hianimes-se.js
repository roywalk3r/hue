/**
 * Hue Stream API
 * Professional Streaming Aggregator
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */

const { chromium } = require('playwright');

(async () => {
    const browser = await chromium.launch({ headless: true, executablePath: '/opt/google/chrome/chrome' });
    const context = await browser.newContext();
    const page = await context.newPage();
    
    console.log('Intercepting network requests on https://hianimes.se/');
    
    page.on('request', req => {
        if (req.url().includes('ajax') || req.url().includes('getSources')) {
            console.log(`[AJAX] ${req.url()}`);
        }
    });

    try {
        await page.goto('https://hianimes.se/home', { waitUntil: 'networkidle' });
        await page.waitForTimeout(5000);
        
        // Find first watch link
        const watchLink = await page.evaluate(() => {
            return document.querySelector('a[href*="/watch/"]')?.href;
        });
        
        if (watchLink) {
            console.log(`Navigating to watch link: ${watchLink}`);
            await page.goto(watchLink, { waitUntil: 'networkidle' });
            await page.waitForTimeout(10000);
            
            // Check for servers
            const servers = await page.evaluate(() => {
                return Array.from(document.querySelectorAll('.server-item, .server')).map(s => s.innerText.trim());
            });
            console.log('Servers found:', servers);
        } else {
            console.log('No watch link found.');
            const body = await page.innerText('body');
            console.log('Body snippet:', body.substring(0, 500));
        }

    } catch (e) {
        console.log('Error:', e.message);
    } finally {
        await browser.close();
    }
})();