const { chromium } = require('playwright');

(async () => {
  const browser = await chromium.launch({ headless: true, executablePath: '/opt/google/chrome/chrome' });
  const context = await browser.newContext();
  const page = await context.newPage();
  
  console.log('Navigating to watch page...');
  await page.goto('https://hianimes.se/watch/one-piece-episode-1161-hef5mk', { waitUntil: 'load', timeout: 30000 });
  await page.waitForTimeout(5000);
  
  // Get all iframes
  const iframes = await page.$$eval('iframe', frames => 
    frames.map(f => ({ src: f.src, id: f.id, class: f.className, name: f.name }))
  );
  console.log('Iframes:', JSON.stringify(iframes, null, 2));
  
  // Look for video element
  const videos = await page.$$eval('video', vids => 
    vids.map(v => ({ src: v.src, poster: v.poster, class: v.className }))
  );
  console.log('Videos:', JSON.stringify(videos, null, 2));
  
  // Look for any element with data-src or src containing embed/stream
  const embeds = await page.$$eval('[src*="embed"], [src*="stream"], [data-src*="embed"], [data-src*="stream"]', 
    els => els.map(e => ({ src: e.src || e.getAttribute('data-src'), tag: e.tagName }))
  );
  console.log('Embeds:', JSON.stringify(embeds, null, 2));
  
  // Check for player container
  const player = await page.$eval('#player, .player, .video-player, [class*="player"]', 
    el => ({ class: el.className, id: el.id, html: el.innerHTML.substring(0, 500) })
  ).catch(() => null);
  console.log('Player container:', player ? JSON.stringify(player, null, 2).substring(0, 500) : 'not found');
  
  await browser.close();
})();
