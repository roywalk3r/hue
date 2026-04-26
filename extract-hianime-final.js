const { chromium } = require('playwright');

(async () => {
  const browser = await chromium.launch({
    headless: true,
    executablePath: '/opt/google/chrome/chrome'
  });
  const context = await browser.newContext();
  const page = await context.newPage();

  console.log('--- HIANIME SOURCE EXTRACTION ---');
  
  context.on('response', async res => {
    const url = res.url();
    if (url.includes('getSources')) {
        console.log(`\n[CAPTURED] getSources URL: ${url}`);
        try {
            const json = await res.json();
            console.log('--- SOURCES JSON ---');
            console.log(JSON.stringify(json, null, 2));
            console.log('---------------------\n');
        } catch (e) {
            console.log('[ERROR] Could not parse JSON from getSources');
        }
    }
  });

  try {
    // 1. We must navigate through the site to get the correct cookies and referer state
    console.log('1. Navigating to hianimes.se...');
    await page.goto('https://hianimes.se/home', { waitUntil: 'networkidle' });

    console.log('2. Opening watch page...');
    const watchUrl = 'https://hianimes.se/watch/one-piece-episode-1158-xra5nl';
    await page.goto(watchUrl, { waitUntil: 'load' });
    
    console.log('3. Waiting for iframe and player...');
    await page.waitForTimeout(10000);

    // 4. Look for the iframe and try to interact with it
    const frame = page.frames().find(f => f.url().includes('stream/s-2'));
    if (frame) {
        console.log(`Found player frame: ${frame.url()}`);
        // Interaction might be needed to trigger getSources
        await page.evaluate(() => {
            const playBtn = document.querySelector('.play-btn, #player');
            if (playBtn) playBtn.click();
        });
    } else {
        console.log('Player frame not found in context.');
    }

    await page.waitForTimeout(15000);

  } catch (err) {
    console.error(err);
  } finally {
    await browser.close();
  }
})();
