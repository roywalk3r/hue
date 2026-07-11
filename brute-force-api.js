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
            console.log(`  Length: ${text.length}`);
            if (text.includes('One Piece') || text.includes('episodes')) {
                console.log('  --- START BODY ---');
                console.log(text.substring(0, 500));
                console.log('  --- END BODY ---');
            }
        } catch (e) {}
    }
  });

  try {
    await page.goto('https://hianimes.se/home', { waitUntil: 'load' });
    await page.waitForTimeout(5000);
    
    console.log('Clicking One Piece link...');
    await page.click('a:has-text("One Piece")');
    await page.waitForTimeout(10000);

  } catch (err) {
    console.error(err);
  } finally {
    await browser.close();
  }
})();
