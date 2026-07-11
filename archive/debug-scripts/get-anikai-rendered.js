/**
 * Hue Stream API
 * Professional Streaming Aggregator
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */

const { chromium } = require('playwright');

(async () => {
  const browser = await chromium.launch({ 
    headless: true,
    executablePath: '/opt/google/chrome/chrome'
  });
  const context = await browser.newContext();
  const page = await context.newPage();

  try {
    await page.goto('https://anikai.to/watch/naruto-shippuden-100/episode-1', { waitUntil: 'networkidle' });
    const html = await page.content();
    console.log(html.substring(0, 5000)); // Print first 5000 chars of rendered HTML
  } catch (error) {
    console.error('Error:', error);
  } finally {
    await browser.close();
  }
})();
