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
    if (url.includes('api/anime/')) {
        console.log(`
[SUCCESS] Captured Anime Info: ${url}`);
        try {
            const json = await res.json();
            console.log('--- JSON BODY ---');
            console.log(JSON.stringify(json, null, 2));
        } catch (e) {}
    }
  });

  try {
    const url = 'https://hianimes.se/anime/one-piece-9t5su0';
    await page.goto(url, { waitUntil: 'load' });
    await page.waitForTimeout(10000);
  } catch (err) {
    console.error(err);
  } finally {
    await browser.close();
  }
})();
