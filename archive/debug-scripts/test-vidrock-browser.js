/**
 * Hue Stream API
 * Professional Streaming Aggregator
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */

const { getStreams } = require('./providers/vidrock');

async function test() {
    console.log('Testing VidRock (Browser Version)...');
    try {
        // Fight Club
        const streams = await getStreams('550', 'movie');
        console.log('Streams found:', streams.length);
        if (streams.length > 0) {
            console.log(JSON.stringify(streams[0], null, 2));
        }
    } catch (e) {
        console.error('Test failed:', e);
    }
}

test().catch(console.error).finally(() => process.exit());
