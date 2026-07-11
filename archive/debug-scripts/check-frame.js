const { chromium } = require('playwright');

(async () => {
  const browser = await chromium.launch({ headless: true, executablePath: '/opt/google/chrome/chrome' });
  const context = await browser.newContext();
  const page = await context.newPage();
  
  await page.goto('https://hianimes.se/watch/one-piece-episode-1-a7b7hv-141', { waitUntil: 'domcontentloaded', timeout: 30000 });
  await page.waitForTimeout(5000);
  
  const frames = page.frames();
  for (const frame of frames) {
    const url = frame.url();
    if (url.includes('megaplay')) {
      console.log('Megaplay frame URL:', url);
      const title = await frame.title().catch(() => 'no title');
      console.log('Frame title:', title);
      const html = await frame.content();
      console.log('HTML length:', html.length);
      console.log('First 500 chars:', html.substring(0, 500));
      
      // Check for error message
      const error = await frame.$eval('body', b => b.innerText).catch(() => '');
      console.log('Body text:', error.substring(0, 200));
    }
  }
  
  await browser.close();
})();
