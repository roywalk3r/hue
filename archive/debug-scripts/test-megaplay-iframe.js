const { chromium } = require('playwright');

(async () => {
  const browser = await chromium.launch({ headless: true, executablePath: '/opt/google/chrome/chrome' });
  const context = await browser.newContext();
  const page = await context.newPage();
  
  const requests = [];
  page.on('request', req => {
    const url = req.url();
    if (url.includes('getSources') || url.includes('m3u8') || url.includes('.ts') || url.includes('mp4')) {
      requests.push({ url, method: req.method() });
    }
  });
  
  console.log('Loading megaplay iframe...');
  await page.goto('https://megaplay.buzz/stream/s-2/2142/sub', { waitUntil: 'networkidle', timeout: 30000 });
  await page.waitForTimeout(5000);
  
  console.log('\nNetwork requests:');
  requests.forEach(r => console.log(r.method, r.url.substring(0, 120)));
  
  // Check for video element
  const videos = await page.$$eval('video', vids => 
    vids.map(v => ({ src: v.src, poster: v.poster }))
  );
  console.log('\nVideos:', JSON.stringify(videos, null, 2));
  
  // Check page title
  const title = await page.title();
  console.log('\nPage title:', title);
  
  await browser.close();
})();
