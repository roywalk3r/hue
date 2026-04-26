const { chromium } = require('playwright');

(async () => {
    const browser = await chromium.launch({ headless: true, executablePath: '/opt/google/chrome/chrome' });
    const domains = ['https://hianimes.se', 'https://hianime.ph', 'https://hianime.lc'];
    
    for (const domain of domains) {
        const context = await browser.newContext();
        const page = await context.newPage();
        try {
            console.log(`Checking ${domain}...`);
            await page.goto(domain, { waitUntil: 'load', timeout: 30000 });
            const title = await page.title();
            const url = page.url();
            console.log(`- Title: ${title}`);
            console.log(`- Final URL: ${url}`);
            
            // Check for search input to confirm it's a real site
            const hasSearch = await page.$('input[name="keyword"]') !== null;
            console.log(`- Functional: ${hasSearch}`);
            
        } catch (e) {
            console.log(`- Error: ${e.message}`);
        } finally {
            await context.close();
        }
    }
    await browser.close();
})();