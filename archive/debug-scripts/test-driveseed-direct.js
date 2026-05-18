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

async function testDriveseed() {
  const jar = new CookieJar();
  const client = wrapper(axios.create({ jar, maxRedirects: 0 })); // Don't follow redirects
  
  const driveseedUrl = 'https://driveseed.org/r?key=QWNZTTJuWUtPV2w3SU0yT1pIbk9GOGxKM0g1dUFLOEN6UWs3TWZ6S1J3QT0=&id=OHo4a3AwdWx6Yjl6aldHVko5ZzVIMzB3ejdlZlRGV0NNck1TT29nK1Q5Ty9NbmJZdHJpaSs3d01VVjVaY09ZWg==';
  
  console.log('Step 1: Requesting driveseed redirect URL (no auto-redirect)...');
  try {
    const response1 = await client.get(driveseedUrl);
    console.log(`Response status: ${response1.status}`);
  } catch (error) {
    if (error.response && error.response.status === 302) {
      console.log('Got 302 redirect');
      console.log(`Location: ${error.response.headers.location}`);
    }
  }
  
  // Now follow manually with redirects enabled
  const clientWithRedirects = wrapper(axios.create({ jar, maxRedirects: 10 }));
  
  console.log('\nStep 2: Following with redirects enabled...');
  const response = await clientWithRedirects.get(driveseedUrl);
  console.log(`Final URL: ${response.request.res.responseUrl}`);
  console.log(`HTML length: ${response.data.length}`);
  
  // Check for JS redirect
  const $ = cheerio.load(response.data);
  const scriptContent = $('script').html();
  const jsRedirect = scriptContent && scriptContent.match(/window\.location\.replace\("([^"]+)"\)/);
  
  if (jsRedirect) {
    console.log(`\nFound JS redirect to: ${jsRedirect[1]}`);
    console.log('This would take us to driveleech.net which is under maintenance');
  }
  
  // Check what's on the driveseed page itself
  console.log('\n=== Analyzing driveseed.org page ===');
  const title = $('title').text();
  console.log(`Page title: ${title}`);
  
  const h3 = $('h3').text().trim();
  console.log(`H3: ${h3}`);
  
  // Look for file info
  const fileInfo = $('li.list-group-item').map((i, el) => $(el).text().trim()).get();
  console.log(`\nFile info items: ${fileInfo.length}`);
  fileInfo.forEach(info => console.log(`  - ${info}`));
  
  // Look for download buttons
  const buttons = $('a.btn').map((i, el) => ({
    text: $(el).text().trim(),
    href: $(el).attr('href'),
    classes: $(el).attr('class')
  })).get();
  console.log(`\nButtons found: ${buttons.length}`);
  buttons.forEach(btn => console.log(`  - "${btn.text}" [${btn.classes}] -> ${btn.href}`));
}

testDriveseed().catch(console.error);
