require('dotenv').config();

// Initialize config system to get TMDB API key
const { config } = require('./utils/config');

// Import the ShowBox playwright provider
const { getStreamsFromTmdbId } = require('./providers/showbox-playwright');

// Test parameters
const TMDB_ID = '310131'; // The Witch (2016)
const MEDIA_TYPE = 'movie';
const SEASON = null;
const EPISODE = null;
const IMDB_ID = null;
const USER_COOKIE = null;

async function testShowBoxPlaywright() {
    console.log('=== Testing ShowBox Playwright Provider ===');
    console.log(`TMDB ID: ${TMDB_ID}`);
    console.log(`Media Type: ${MEDIA_TYPE}`);
    console.log(`TMDB API Keys configured: ${config.tmdbApiKeys ? config.tmdbApiKeys.length : 0}`);
    console.log('=====================================\n');
    
    if (!config.tmdbApiKeys || config.tmdbApiKeys.length === 0) {
        console.error('ERROR: No TMDB API keys configured. Please add TMDB_API_KEY to .env file or tmdbApiKeys to user-config.json');
        process.exit(1);
    }
    
    try {
        const startTime = Date.now();
        const streams = await getStreamsFromTmdbId(TMDB_ID, MEDIA_TYPE, SEASON, EPISODE, IMDB_ID, USER_COOKIE);
        const duration = Date.now() - startTime;
        
        console.log(`\nTest completed in ${duration}ms`);
        console.log(`Found ${streams.length} streams:`);
        
        if (streams.length > 0) {
            streams.forEach((stream, index) => {
                console.log(`\nStream ${index + 1}:`);
                console.log(`  Name: ${stream.name}`);
                console.log(`  Quality: ${stream.quality}`);
                console.log(`  Provider: ${stream.provider}`);
                console.log(`  URL: ${stream.url.substring(0, 100)}...`);
            });
        } else {
            console.log('\nNo streams found. Check the logs above for any errors.');
        }
        
    } catch (error) {
        console.error('\nTest failed with error:');
        console.error(error.message);
        console.error(error.stack);
    }
    
    process.exit(0);
}

// Run the test
testShowBoxPlaywright();
