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

  console.log('--- DEEP API CAPTURE ---');
  
  context.on('response', async res => {
    const url = res.url();
    if (url.includes('myani.cfd/api/')) {
        console.log(`[API] ${url}`);
        if (url.includes('episodes')) {
            try {
                const json = await res.json();
                console.log('--- EPISODES FOUND ---');
                // console.log(JSON.stringify(json, null, 2));
            } catch (e) {}
        }
    }
  });

  try {
    await page.goto('https://hianimes.se/home', { waitUntil: 'load' });
    await page.waitForTimeout(5000);

    // Find and click One Piece
    await page.evaluate(() => {
        const links = Array.from(document.querySelectorAll('a'));
        const op = links.find(a => a.innerText.includes('One Piece'));
        if (op) op.click();
    });

    await page.waitForTimeout(15000);

  } catch (err) {
    console.error(err);
  } finally {
    await browser.close();
  }
})();
