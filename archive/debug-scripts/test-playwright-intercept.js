const { chromium } = require('playwright');

(async () => {
  const browser = await chromium.launch({ headless: true, executablePath: '/opt/google/chrome/chrome' });
  const context = await browser.newContext();
  const page = await context.newPage();
  
  const requests = [];
  page.on('request', req => {
    const url = req.url();
    if (url.includes('api') || url.includes('ajax') || url.includes('stream') || url.includes('source') || url.includes('embed') || url.includes('m3u8') || url.includes('vidmoly') || url.includes('megaplay') || url.includes('animedata')) {
      requests.push({ url, method: req.method(), headers: req.headers() });
    }
  });
  
  page.on('response', async res => {
    const url = res.url();
    if (url.includes('api') || url.includes('ajax') || url.includes('stream') || url.includes('source') || url.includes('animedata')) {
      try {
        const text = await res.text();
        console.log('\n[RESPONSE]', url, 'status:', res.status(), 'len:', text.length);
        if (text.length < 1000) console.log(text.substring(0, 500));
        else if (text.includes('sources') || text.includes('m3u8') || text.includes('file')) {
          console.log(text.substring(0, 500));
        }
      } catch {}
    }
  });
  
  console.log('Navigating to hianimes.se...');
  await page.goto('https://hianimes.se/watch/one-piece-episode-1161-hef5mk', { waitUntil: 'networkidle' });
  await page.waitForTimeout(5000);
  
  console.log('\n=== All captured requests ===');
  requests.forEach((r, i) => console.log(i, r.method, r.url.substring(0, 120)));
  
  await browser.close();
})();
