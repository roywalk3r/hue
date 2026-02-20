const { getStreamedStreams } = require('./providers/streamed');

async function test() {
    console.log('--- Testing Streamed.pk Discovery ---');
    // We'll simulate a request for Willow Cricket
    const ctx = {
        matchName: 'admin-willow-cricket',
        req: { path: '/sports-stream/admin-willow-cricket' }
    };

    try {
        const streams = await getStreamedStreams(ctx);
        console.log(`Found ${streams.length} streams:`);
        streams.forEach(s => {
            console.log(`- ${s.title} [${s.quality}] -> ${s.url.substring(0, 60)}...`);
        });
    } catch (e) {
        console.error('Test failed:', e);
    }
}

test();
