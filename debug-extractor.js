const LivesportM3U8Extractor = require('./livesport-m3u8-extractor');

async function debug() {
    const extractor = new LivesportM3U8Extractor();
    const url = 'https://streamed.pk/watch/aston-villa-vs-red-bull-salzburg-2327999/alpha/1';
    console.log(`Debugging URL: ${url}`);
    
    const result = await extractor.extractAllStreams(url);
    console.log('Result:', JSON.stringify(result, null, 2));
}

debug();
