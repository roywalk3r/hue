const { chromium } = require('playwright');

(async () => {
  const browser = await chromium.launch({ headless: true, executablePath: '/opt/google/chrome/chrome' });
  const context = await browser.newContext();
  const page = await context.newPage();
  
  // First visit hianimes.se to set cookies/referer context
  console.log('Visiting hianimes.se first...');
  await page.goto('https://hianimes.se/watch/one-piece-episode-1-a7b7hv-141', { waitUntil: 'domcontentloaded', timeout: 30000 });
  await page.waitForTimeout(3000);
  
  // Get iframe src
  const iframeSrc = await page.$eval('iframe', f => f.src);
  console.log('Iframe src:', iframeSrc);
  
  // Now navigate to the iframe directly
  console.log('\nNavigating to iframe...');
  await page.goto(iframeSrc, { waitUntil: 'networkidle', timeout: 30000 });
  await page.waitForTimeout(5000);
  
  const title = await page.title();
  console.log('Iframe title:', title);
  
  // Check for video
  const videos = await page.$$eval('video', vids => 
    vids.map(v => ({ src: v.src, poster: v.poster }))
  );
  console.log('Videos:', JSON.stringify(videos, null, 2));
  
  // Get page content
  const html = await page.content();
  const m3u8Match = html.match(/https?:\/\/[^\s\"'\)]+\.m3u8/);
  console.log('M3U8 in page:', m3u8Match?.[0]);
  
  await browser.close();
})();
