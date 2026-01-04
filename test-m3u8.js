#!/usr/bin/env node
/**
 * Test M3U8 URL directly
 */

const axios = require('axios');

async function testM3U8(url) {
    console.log(`Testing M3U8 URL: ${url}`);
    console.log('='.repeat(60));
    
    try {
        // Test with mobile headers (same as the working extractor)
        const response = await axios.get(url, {
            headers: {
                'User-Agent': 'Mozilla/5.0 (Linux; Android 6.0; Nexus 5 Build/MRA58N) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Mobile Safari/537.36',
                'Origin': 'https://embedsports.top',
                'Referer': 'https://embedsports.top/',
                'Accept': '*/*',
                'Accept-Language': 'en-US,en;q=0.9',
                'DNT': '1',
                'Sec-Ch-Ua': '"Brave";v="143", "Chromium";v="143", "Not A(Brand";v="24"',
                'Sec-Ch-Ua-Mobile': '?1',
                'Sec-Ch-Ua-Platform': '"Android"',
                'Sec-Fetch-Dest': 'empty',
                'Sec-Fetch-Mode': 'cors',
                'Sec-Fetch-Site': 'cross-site',
                'Sec-GPC': '1'
            },
            timeout: 10000
        });
        
        console.log(`✅ Status: ${response.status}`);
        console.log(`Content-Type: ${response.headers['content-type']}`);
        console.log(`Content-Length: ${response.headers['content-length'] || 'N/A'}`);
        
        // Show first few lines of M3U8 content
        const lines = response.data.split('\n').slice(0, 10);
        console.log('\nFirst 10 lines of M3U8:');
        console.log('-'.repeat(40));
        lines.forEach((line, i) => {
            console.log(`${i + 1}: ${line}`);
        });
        
        if (lines.length > 10) {
            console.log('... (truncated)');
        }
        
        return true;
        
    } catch (error) {
        console.error(`❌ Error: ${error.message}`);
        if (error.response) {
            console.error(`Status: ${error.response.status}`);
            console.error(`Headers:`, error.response.headers);
        }
        return false;
    }
}

// Test the M3U8 URL from your scraper
const m3u8Url = 'https://gg.poocloud.in/pl_sun/tracks-v1a1/mono.ts.m3u8';

testM3U8(m3u8Url).then(success => {
    if (success) {
        console.log('\n✅ M3U8 URL is working!');
        console.log('\nYou can use this URL directly in players like:');
        console.log('- VLC');
        console.log('- ffmpeg');
        console.log('- HLS.js');
        console.log('\nOr create a simple proxy with:');
        console.log('node -e "const http = require(\"http\"); const url = require(\"url\"); http.createServer((req, res) => { const options = url.parse(\"https://gg.poocloud.in/pl_sun/tracks-v1a1/mono.ts.m3u8\"); options.headers = {\"User-Agent\": \"Mozilla/5.0...\"}; require(\"https\").request(options, (r) => { res.writeHead(r.statusCode, r.headers); r.pipe(res); }).end(); }).listen(3000); console.log(\"Proxy running on http://localhost:3000\");');
    } else {
        console.log('\n❌ M3U8 URL is not accessible');
    }
});
