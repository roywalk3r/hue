const { chromium } = require('playwright');

(async () => {
  const browser = await chromium.launch({
    headless: true,
    executablePath: '/opt/google/chrome/chrome'
  });
  const context = await browser.newContext();
  const page = await context.newPage();

  console.log('--- CAPTURING ANIME ID ---');
  
  context.on('response', async res => {
    const url = res.url();
    if (url.includes('api/episodes/')) {
        console.log(`
[SUCCESS] Captured Episode List URL: ${url}`);
        const animeId = url.split('/').pop().split('?')[0];
        console.log(`ANIME_ID FOUND: ${animeId}`);
    }
  });

  try {
    const infoUrl = 'https://hianimes.se/anime/one-piece-gahocg';
    console.log(`Navigating to: ${infoUrl}`);
    await page.goto(infoUrl, { waitUntil: 'load', timeout: 60000 });
    await page.waitForTimeout(10000);

  } catch (err) {
    console.error(err);
  } finally {
    await browser.close();
  }
})();
