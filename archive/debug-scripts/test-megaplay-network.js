const { chromium } = require('playwright');

(async () => {
  const browser = await chromium.launch({ headless: true, executablePath: '/opt/google/chrome/chrome' });
  const context = await browser.newContext();
  const page = await context.newPage();
  
  const requests = [];
  page.on('request', req => {
    const url = req.url();
    if (url.includes('getSources') || url.includes('m3u8') || url.includes('.ts') || url.includes('mp4') || url.includes('megaplay') || url.includes('statlytic') || url.includes('mewstream')) {
      requests.push({ url, method: req.method(), headers: req.headers() });
    }
  });
  
  console.log('Loading hianimes.se episode 1...');
  await page.goto('https://hianimes.se/watch/one-piece-episode-1-a7b7hv-141', { waitUntil: 'domcontentloaded', timeout: 30000 });
  await page.waitForTimeout(8000);
  
  console.log('\n=== Network requests ===');
  requests.forEach((r, i) => {
    console.log(i, r.method, r.url.substring(0, 120));
  });
  
  await browser.close();
})();
