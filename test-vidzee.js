require('dotenv').config();
const { config } = require('./utils/config');
const { getVidZeeStreams } = require('./providers/VidZee');
const { processStreamsForProxy } = require('./proxy/proxyServer');

async function testVidZee() {
    console.log('=== Testing VidZee Provider ===');
    console.log(`Proxy enabled: ${config.enableProxy}`);
    console.log('=====================================\n');
    
    try {
        // Test with The Witch (2016)
        const tmdbId = '310131';
        const mediaType = 'movie';
        
        const streams = await getVidZeeStreams(tmdbId, mediaType, null, null, null);
        console.log(`\nFound ${streams.length} streams from VidZee:`);
        
        if (streams.length > 0) {
            // Show first stream details
            const firstStream = streams[0];
            console.log('\nFirst stream details:');
            console.log(`  Name: ${firstStream.name}`);
            console.log(`  Quality: ${firstStream.quality}`);
            console.log(`  Provider: ${firstStream.provider}`);
            console.log(`  Has headers: ${firstStream.headers ? 'Yes' : 'No'}`);
            if (firstStream.headers) {
                console.log(`  Referer: ${firstStream.headers.Referer}`);
            }
            console.log(`  URL: ${firstStream.url.substring(0, 100)}...`);
            
            // Test if URL is encrypted
            if (firstStream.url.length < 200 && !firstStream.url.startsWith('http')) {
                console.log('\n⚠️  URL appears to be encrypted (not starting with http)');
            }
            
            // Process through proxy if enabled
            if (config.enableProxy) {
                const serverUrl = 'http://localhost:8787';
                const proxiedStreams = processStreamsForProxy([firstStream], serverUrl);
                console.log('\nAfter proxy processing:');
                console.log(`  URL: ${proxiedStreams[0].url}`);
            }
            
            // Test decoding the URL
            // First we need to require the VidZee module to access the decode function
            const VidZeeModule = require('./providers/VidZee');
            // The decode function is not exported, so let's check if we can access it another way
            console.log('\n📝 Available exports from VidZee:', Object.keys(VidZeeModule));
            
            // Let's manually test if the URL is properly formatted
            if (firstStream.url.startsWith('http')) {
                console.log('\n✅ URL is already decoded (starts with http)');
            } else {
                console.log('\n⚠️  URL is encrypted and needs decoding');
                console.log(`  Length: ${firstStream.url.length} characters`);
                console.log(`  Starts with: ${firstStream.url.substring(0, 50)}...`);
            }
        } else {
            console.log('No streams found');
        }
        
    } catch (error) {
        console.error('\nTest failed with error:');
        console.error(error.message);
    }
    
    process.exit(0);
}

testVidZee();
