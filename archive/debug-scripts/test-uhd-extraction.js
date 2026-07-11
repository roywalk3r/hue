/**
 * Hue Stream API
 * Professional Streaming Aggregator
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */

// Direct test of the file page extraction fix
const axios = require('axios');
const cheerio = require('cheerio');
const { CookieJar } = require('tough-cookie');
const { wrapper } = require('axios-cookiejar-support');

async function createSession(jar) {
  const sessionConfig = {
    jar,
    headers: {
      'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/137.0.0.0 Safari/537.36',
      'Accept': 'text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8',
      'Accept-Language': 'en-US,en;q=0.5',
      'Connection': 'keep-alive',
      'Upgrade-Insecure-Requests': '1'
    }
  };
  return wrapper(axios.create(sessionConfig));
}

async function testExtraction() {
  console.log('Testing UHDMovies file page extraction fix...\n');
  
  const jar = new CookieJar();
  const session = await createSession(jar);
  
  // Test with a known driveseed redirect URL
  const driveleechRedirectUrl = 'https://driveseed.org/r?key=QWNZTTJuWUtPV2w3SU0yT1pIbk9GOGxKM0g1dUFLOEN6UWs3TWZ6S1J3QT0=&id=OHo4a3AwdWx6Yjl6aldHVko5ZzVIMzB3ejdlZlRGV0NNck1TT29nK1Q5Ty9NbmJZdHJpaSs3d01VVjVaY09ZWg==';
  
  console.log(`Step 1: Requesting driveseed redirect URL...`);
  const response = await session.get(driveleechRedirectUrl, { maxRedirects: 10 });
  let $ = cheerio.load(response.data);
  
  console.log(`  Response URL: ${response.request.res.responseUrl}`);
  console.log(`  HTML length: ${response.data.length} bytes`);
  
  // Check for JavaScript redirect
  const scriptContent = $('script').html();
  const redirectMatch = scriptContent && scriptContent.match(/window\.location\.replace\("([^"]+)"\)/);
  
  if (redirectMatch && redirectMatch[1]) {
    console.log(`\nStep 2: Found JS redirect: ${redirectMatch[1]}`);
    
    // THE FIX: Use current response URL as base, not hardcoded driveleech.net
    const baseUrl = response.request.res.responseUrl || driveleechRedirectUrl;
    const finalFilePageUrl = new URL(redirectMatch[1], baseUrl).href;
    
    console.log(`  Base URL: ${baseUrl}`);
    console.log(`  Final file page URL: ${finalFilePageUrl}`);
    console.log(`  ✓ Using driveseed.org (not driveleech.net)!`);
    
    console.log(`\nStep 3: Loading final file page...`);
    const finalResponse = await session.get(finalFilePageUrl, { maxRedirects: 10 });
    $ = cheerio.load(finalResponse.data);
    
    console.log(`  HTML length: ${finalResponse.data.length} bytes`);
    
    // Check for download buttons
    const resumeCloudButton = $('a:contains("Resume Cloud"), a:contains("Cloud Resume Download")');
    const instantDownloadButton = $('a:contains("Instant Download")');
    
    console.log(`\n=== Results ===`);
    console.log(`Resume Cloud button: ${resumeCloudButton.length > 0 ? '✓ FOUND' : '✗ NOT FOUND'}`);
    if (resumeCloudButton.length > 0) {
      console.log(`  href: ${resumeCloudButton.attr('href')}`);
    }
    
    console.log(`Instant Download button: ${instantDownloadButton.length > 0 ? '✓ FOUND' : '✗ NOT FOUND'}`);
    if (instantDownloadButton.length > 0) {
      const href = instantDownloadButton.attr('href');
      console.log(`  href: ${href ? href.substring(0, 80) + '...' : 'none'}`);
    }
    
    // Check file info
    const fileName = $('li.list-group-item:contains("Name")').text().replace('Name :', '').trim();
    const fileSize = $('li.list-group-item:contains("Size")').text().replace('Size :', '').trim();
    
    console.log(`\nFile Info:`);
    console.log(`  Name: ${fileName.substring(0, 80)}...`);
    console.log(`  Size: ${fileSize}`);
    
    if (resumeCloudButton.length > 0 || instantDownloadButton.length > 0) {
      console.log(`\n✓ SUCCESS: Download buttons found on driveseed.org!`);
    } else {
      console.log(`\n✗ FAILED: No download buttons found`);
    }
  } else {
    console.log(`No JS redirect found`);
  }
}

testExtraction().catch(console.error);
