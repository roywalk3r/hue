const provider = require('./providers/9anime');

async function test() {
    // Solo Leveling
    const tmdbId = '120998';
    console.log(`Testing 9Anime Provider with TMDB ID: ${tmdbId} (Solo Leveling Season 1, Ep 1)...`);
    
    const streams = await provider.getStreams(tmdbId, 'tv', 1, 1);
    console.log('Streams found:', JSON.stringify(streams, null, 2));
}

test();
