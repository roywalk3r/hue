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
  const context = await browser.newContext({
    userAgent: 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36'
  });
  const page = await context.newPage();

  console.log('--- HIANIME RECOVERY ---');
  
  context.on('response', async res => {
    const url = res.url();
    if (url.includes('getSources')) {
        console.log(`
[SUCCESS] Captured Source: ${url}`);
        try {
            console.log(await res.text());
        } catch (e) {}
    }
  });

  try {
    const url = 'https://hianimes.se/watch/one-piece-episode-1158-xra5nl';
    console.log(`Navigating to: ${url}`);
    await page.goto(url, { waitUntil: 'domcontentloaded', timeout: 60000 });
    
    console.log('Waiting for elements to load...');
    await page.waitForTimeout(10000);

    const iframes = await page.evaluate(() => Array.from(document.querySelectorAll('iframe')).map(i => i.src));
    console.log('Iframes:', iframes);

    // If no iframes, the page might be blank or protected
    if (iframes.length === 0) {
        console.log('No iframes found. Content snippet:');
        console.log((await page.innerText('body')).substring(0, 500));
    } else {
        console.log('Iframe detected, waiting for network...');
        await page.waitForTimeout(10000);
    }

  } catch (err) {
    console.error(err);
  } finally {
    await browser.close();
  }
})();
