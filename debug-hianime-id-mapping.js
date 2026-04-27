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
  const browser = await chromium.launch({ 
    headless: true,
    executablePath: '/opt/google/chrome/chrome'
  });
  const context = await browser.newContext();
  const page = await context.newPage();

  context.on('response', async res => {
    const url = res.url();
    if (url.includes('myani.cfd/api/')) {
        console.log(`[API] ${url}`);
        try {
            const text = await res.text();
            if (url.includes('search')) {
                 console.log('--- SEARCH RESPONSE ---');
                 console.log(text.substring(0, 1000));
            }
            if (url.includes('info')) {
                 console.log('--- INFO RESPONSE ---');
                 console.log(text.substring(0, 1000));
            }
        } catch (e) {}
    }
  });

  try {
    const searchUrl = 'https://hianimes.se/search?keyword=Naruto';
    console.log(`Searching: ${searchUrl}`);
    await page.goto(searchUrl, { waitUntil: 'load' });
    await page.waitForTimeout(5000);

    const firstResult = await page.evaluate(() => document.querySelector('a[href*="/anime/"]')?.href);
    if (firstResult) {
        console.log(`Navigating to result: ${firstResult}`);
        await page.goto(firstResult, { waitUntil: 'load' });
        await page.waitForTimeout(10000);
    }

  } catch (err) {
    console.error(err);
  } finally {
    await browser.close();
  }
})();
