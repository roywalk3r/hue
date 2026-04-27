/**
 * Hue Stream API
 * Professional Streaming Aggregator
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */

const { getUHDMoviesStreams } = require('./providers/uhdmovies.js');

async function test() {
  console.log('Testing UHDMovies for TV show...');
  const streams = await getUHDMoviesStreams('219246', 'tv', 1, 2);
  console.log('\n=== RESULTS ===');
  console.log(`Found ${streams.length} streams:`);
  streams.forEach((stream, i) => {
    console.log(`\n${i + 1}. ${stream.title}`);
    console.log(`   Quality: ${stream.quality}`);
    console.log(`   URL: ${stream.url.substring(0, 100)}...`);
  });
}

test().catch(console.error);
