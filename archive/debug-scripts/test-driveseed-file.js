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

async function testDriveseedFile() {
  const jar = new CookieJar();
  const client = wrapper(axios.create({ jar, maxRedirects: 10 }));
  
  // The file ID from the redirect
  const fileUrl = 'https://driveseed.org/file/NBt4Vx8bJgdzFBSkbIJx';
  
  console.log(`Testing file page on driveseed.org: ${fileUrl}\n`);
  
  const response = await client.get(fileUrl);
  console.log(`Response status: ${response.status}`);
  console.log(`Final URL: ${response.request.res.responseUrl}`);
  console.log(`HTML length: ${response.data.length}`);
  
  const $ = cheerio.load(response.data);
  
  console.log('\n=== Page Analysis ===');
  const title = $('title').text();
  console.log(`Page title: ${title}`);
  
  const h3 = $('h3').text().trim();
  console.log(`H3: ${h3}`);
  
  // Look for file info
  const fileInfo = $('li.list-group-item').map((i, el) => $(el).text().trim()).get();
  console.log(`\nFile info items: ${fileInfo.length}`);
  fileInfo.slice(0, 5).forEach(info => console.log(`  - ${info.substring(0, 80)}...`));
  
  // Look for download buttons
  const buttons = $('a.btn').map((i, el) => ({
    text: $(el).text().trim(),
    href: $(el).attr('href'),
    classes: $(el).attr('class')
  })).get();
  console.log(`\nButtons found: ${buttons.length}`);
  buttons.forEach(btn => console.log(`  - "${btn.text}" [${btn.classes}]`));
  console.log();
  buttons.forEach(btn => console.log(`     ${btn.href}`));
}

testDriveseedFile().catch(console.error);
