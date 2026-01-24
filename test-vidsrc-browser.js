const { getProvider } = require('./providers/registry');

async function test() {
    const providerName = 'vidsrc-embed';
    const provider = getProvider(providerName);

    if (!provider || !provider.enabled) {
        console.error(`Provider ${providerName} is not enabled or found.`);
        return;
    }

    const ctx = {
        tmdbId: '550', // Fight Club
        type: 'movie',
        season: null,
        episode: null
    };

    console.log(`Testing ${providerName} (Browser Version)...`);
    const t0 = Date.now();
    try {
        const streams = await provider.fetch(ctx);
        const duration = Date.now() - t0;
        
        console.log(`\nResults: ${streams.length}`);
        console.log(`Duration: ${duration}ms`);
        
        if (streams.length > 0) {
            console.log('--- Stream Details ---');
            console.log(JSON.stringify(streams[0], null, 2));
        } else {
            console.log('No streams found. Check console for browser errors or blocking.');
        }
    } catch (e) {
        console.error('Test failed:', e);
    }
}

test().catch(console.error).finally(() => process.exit());
