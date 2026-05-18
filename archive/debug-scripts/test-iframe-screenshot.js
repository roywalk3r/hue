const { chromium } = require('playwright');
const fs = require('fs');

(async () => {
  const browser = await chromium.launch({ headless: true, executablePath: '/opt/google/chrome/chrome' });
  const context = await browser.newContext({ viewport: { width: 1280, height: 720 } });
  const page = await context.newPage();
  
  console.log('Navigating to episode 1...');
  await page.goto('https://hianimes.se/watch/one-piece-episode-1-a7b7hv-141', { waitUntil: 'domcontentloaded', timeout: 30000 });
  await page.waitForTimeout(5000);
  
  // Take screenshot of the full page
  await page.screenshot({ path: 'hianimes-ep1.png', fullPage: false });
  console.log('Screenshot saved to hianimes-ep1.png');
  
  // Get iframe element and take screenshot of it
  const iframe = await page.$('iframe');
  if (iframe) {
    const box = await iframe.boundingBox();
    console.log('Iframe bounds:', box);
    await page.screenshot({ path: 'hianimes-ep1-iframe.png', clip: box });
    console.log('Iframe screenshot saved');
    
    // Get iframe frame object
    const frames = page.frames();
    const playerFrame = frames.find(f => f.url().includes('megaplay'));
    if (playerFrame) {
      console.log('Found player frame:', playerFrame.url());
      const frameTitle = await playerFrame.title();
      console.log('Frame title:', frameTitle);
      const frameHtml = await playerFrame.content();
      console.log('Frame HTML length:', frameHtml.length);
      console.log('Frame HTML first 300 chars:', frameHtml.substring(0, 300));
    }
  }
  
  await browser.close();
})();
