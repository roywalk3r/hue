const { chromium } = require('playwright');

(async () => {
  const browser = await chromium.launch({ headless: true, executablePath: '/opt/google/chrome/chrome' });
  const context = await browser.newContext();
  const page = await context.newPage();
  
  console.log('Navigating to vidmoly embed...');
  await page.goto('https://vidmoly.biz/embed-fqyz11yb8t0f.html', { waitUntil: 'load', timeout: 30000 });
  await page.waitForTimeout(8000);
  
  // Check for video
  const videos = await page.$$eval('video', vids => 
    vids.map(v => ({ src: v.src, class: v.className, id: v.id }))
  );
  console.log('Videos:', JSON.stringify(videos, null, 2));
  
  // Check for iframe inside iframe (some embeds nest)
  const iframes = await page.$$eval('iframe', frames => 
    frames.map(f => ({ src: f.src }))
  );
  console.log('Iframes:', JSON.stringify(iframes, null, 2));
  
  // Look for any script that sets video source
  const scripts = await page.$$eval('script', scripts => 
    scripts.filter(s => s.innerText.includes('src') || s.innerText.includes('video') || s.innerText.includes('m3u8') || s.innerText.includes('mp4'))
      .map(s => s.innerText.substring(0, 200))
  );
  console.log('Relevant scripts:', scripts.slice(0, 3));
  
  // Try to get page source
  const html = await page.content();
  const m3u8Match = html.match(/https?:\/\/[^\s\"'\)]+\.m3u8/);
  const mp4Match = html.match(/https?:\/\/[^\s\"'\)]+\.mp4/);
  console.log('M3U8 in source:', m3u8Match?.[0]);
  console.log('MP4 in source:', mp4Match?.[0]);
  
  // Check for jwplayer or other player config
  const playerMatch = html.match(/jwplayer\("[^"]+"\)\.setup\(\{([^}]+)\}/);
  if (playerMatch) {
    console.log('JWPlayer config:', playerMatch[0].substring(0, 300));
  }
  
  await browser.close();
})();
