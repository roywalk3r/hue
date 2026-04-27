/**
 * Hue Stream API
 * Professional Streaming Aggregator
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */

const axios = require('axios');
const cheerio = require('cheerio');
const { CookieJar } = require('tough-cookie');
const { wrapper } = require('axios-cookiejar-support');

async function testDriveleech() {
  const jar = new CookieJar();
  const client = wrapper(axios.create({ jar, maxRedirects: 10 }));
  
  const driveseedUrl = 'https://driveseed.org/r?key=QWNZTTJuWUtPV2w3SU0yT1pIbk9GOGxKM0g1dUFLOEN6UWs3TWZ6S1J3QT0=&id=OHo4a3AwdWx6Yjl6aldHVko5ZzVIMzB3ejdlZlRGV0NNck1TT29nK1Q5Ty9NbmJZdHJpaSs3d01VVjVaY09ZWg==';
  
  console.log('Step 1: Requesting driveseed URL...');
  const response1 = await client.get(driveseedUrl);
  console.log(`Response status: ${response1.status}`);
  console.log(`Response URL: ${response1.request.res.responseUrl}`);
  console.log(`HTML length: ${response1.data.length}`);
  
  // Check for JavaScript redirect
  const $ = cheerio.load(response1.data);
  const scriptContent = $('script').html();
  const redirectMatch = scriptContent && scriptContent.match(/window\.location\.replace\("([^"]+)"\)/);
  
  if (redirectMatch && redirectMatch[1]) {
    const relativeUrl = redirectMatch[1];
    const finalUrl = new URL(relativeUrl, 'https://driveleech.net/').href;
    console.log(`\nStep 2: Found JS redirect to: ${finalUrl}`);
    
    // Check cookies before following redirect
    const cookies = await jar.getCookies('https://driveleech.net');
    console.log(`Cookies for driveleech.net:`, cookies.map(c => `${c.key}=${c.value}`));
    
    console.log('\nStep 3: Following redirect...');
    const response2 = await client.get(finalUrl);
    console.log(`Response status: ${response2.status}`);
    console.log(`Response URL: ${response2.request.res.responseUrl}`);
    console.log(`HTML length: ${response2.data.length}`);
    
    const $2 = cheerio.load(response2.data);
    const links = $2('a').map((i, el) => $2(el).text().trim()).get().filter(t => t);
    console.log(`Links on page: ${links.slice(0, 20).join(', ')}`);
    
    const buttons = $2('button, a.btn').map((i, el) => $2(el).text().trim()).get().filter(t => t);
    console.log(`Buttons on page: ${buttons.join(', ')}`);
    
    // Check all cookies after final request
    const allCookies = await jar.getCookies('https://driveleech.net');
    console.log(`\nAll driveleech.net cookies:`, allCookies.map(c => `${c.key}=${c.value}`));
    
    // Look for specific content
    console.log('\n=== Page Analysis ===');
    const title = $2('title').text();
    console.log(`Page title: ${title}`);
    
    const h1 = $2('h1').text().trim();
    console.log(`H1: ${h1}`);
    
    // Check for forms
    const forms = $2('form').length;
    console.log(`Forms on page: ${forms}`);
    
    // Check for login indicators
    const hasLogin = response2.data.toLowerCase().includes('login') || response2.data.toLowerCase().includes('sign in');
    console.log(`Contains login text: ${hasLogin}`);
    
    // Look for download-related buttons with different selectors
    const downloadButtons = $2('a[href*="download"], a[href*="workers.dev"], a:contains("Download"), a:contains("Resume")').map((i, el) => ({
      text: $2(el).text().trim(),
      href: $2(el).attr('href')
    })).get();
    console.log(`\nDownload-related links found: ${downloadButtons.length}`);
    downloadButtons.forEach(btn => console.log(`  - "${btn.text}" -> ${btn.href}`));
  }
}

testDriveleech().catch(console.error);
