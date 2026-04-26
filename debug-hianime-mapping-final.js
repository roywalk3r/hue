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
        try {
            const json = await res.json();
            console.log(`\n[API] ${url}`);
            if (url.includes('search')) {
                console.log('--- SEARCH DATA ---');
                console.log(JSON.stringify(json[0]).substring(0, 500));
            }
            if (url.includes('anime/')) {
                 console.log('--- DETAIL DATA ---');
                 console.log(JSON.stringify(json).substring(0, 500));
            }
        } catch(e) {}
    }
  });

  try {
    const searchUrl = 'https://hianimes.se/browser?keyword=One+Piece';
    await page.goto(searchUrl, { waitUntil: 'load' });
    await page.waitForTimeout(5000);

    const firstResult = await page.evaluate(() => {
        const item = document.querySelector('.anime-item a') || document.querySelector('a[href*="/anime/"]');
        return item ? item.href : null;
    });

    if (firstResult) {
        console.log(`Found result: ${firstResult}`);
        await page.goto(firstResult, { waitUntil: 'load' });
        await page.waitForTimeout(10000);
    }

  } catch (err) {
    console.error(err);
  } finally {
    await browser.close();
  }
})();
