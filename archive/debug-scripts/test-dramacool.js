/**
 * Hue Stream API
 * Professional Streaming Aggregator
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */

const { getStreams: getDramacoolStreams } = require('./providers/dramacool');

async function test() {
    const tmdbId = '93405'; // Squid Game
    const season = 1;
    const episode = 1;
    console.log(`Testing TMDB ID: ${tmdbId} (Squid Game S${season}E${episode})`);

    try {
        console.log('\n--- Testing Dramacool ---');
        const streams = await getDramacoolStreams(tmdbId, 'tv', season, episode);
        console.log(`Dramacool returned ${streams.length} streams:`);
        console.log(JSON.stringify(streams, null, 2));
    } catch (e) {
        console.error('Dramacool error:', e.message);
    }
    
    process.exit(0);
}

test();
