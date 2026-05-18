const { chromium } = require('playwright');

(async () => {
  const browser = await chromium.launch({ headless: true, executablePath: '/opt/google/chrome/chrome' });
  const context = await browser.newContext();
  const page = await context.newPage();
  
  // Test ep 1161 first
  console.log('Navigating to episode 1161...');
  await page.goto('https://hianimes.se/watch/one-piece-episode-1161-hef5mk', { waitUntil: 'domcontentloaded', timeout: 30000 });
  await page.waitForTimeout(3000);
  let iframes = await page.$$eval('iframe', frames => frames.map(f => ({ src: f.src })));
  console.log('Ep 1161 iframes:', JSON.stringify(iframes, null, 2));
  
  // Test ep 1
  console.log('\nNavigating to episode 1...');
  await page.goto('https://hianimes.se/watch/one-piece-episode-1-a7b7hv-141', { waitUntil: 'domcontentloaded', timeout: 30000 });
  await page.waitForTimeout(3000);
  iframes = await page.$$eval('iframe', frames => frames.map(f => ({ src: f.src })));
  console.log('Ep 1 iframes:', JSON.stringify(iframes, null, 2));
  
  await browser.close();
})();
