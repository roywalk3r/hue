const livetvScraper = require('./services/livetvScraper');
const { closeBrowser } = require('./utils/browserManager');

async function testLiveTV() {
    console.log('Testing LiveTV scraper...\n');

    try {
        // Test 1: Get sports
        console.log('1. Getting sports categories...');
        const sports = await livetvScraper.getSports();
        console.log(`Found ${sports.length} sports:`);
        sports.slice(0, 5).forEach(s => console.log(`  - ${s.title} (${s.id})`));
        console.log();

        // Test 2: Get matches (first sport)
        if (sports.length > 0) {
            console.log(`2. Getting matches for ${sports[0].title}...`);
            const matches = await livetvScraper.getMatches(sports[0].id);
            console.log(`Found ${matches.length} matches:`);
            matches.slice(0, 5).forEach(m => console.log(`  - ${m.title} (${m.timeText || 'No time'})`));
            console.log();

            // Test 3: Get stream variants for a match that HAS them
            let matchWithVariants = null;
            let variants = [];
            
            // Try first 5 matches to find one with variants
            for (const m of matches.slice(0, 5)) {
                 console.log(`Checking ${m.title}...`);
                 const v = await livetvScraper.getStreamVariants(m.url);
                 if (v.length > 0) {
                     matchWithVariants = m;
                     variants = v;
                     break;
                 }
            }

            if (matchWithVariants) {
                console.log(`3. Getting stream variants for: ${matchWithVariants.title}`);
                console.log(`Found ${variants.length} stream variants:`);
                variants.forEach(v => console.log(`  - ${v.title} (${v.quality || 'Unknown quality'})`));
                console.log();

                // Test 4: Resolve first variant
                if (variants.length > 0) {
                    console.log(`4. Resolving stream: ${variants[0].title}`);
                    const resolved = await livetvScraper.resolveStream(variants[0].url);
                    if (resolved.error) {
                        console.log(`  Error: ${resolved.error}`);
                    } else if (resolved.streams) {
                        console.log(`  Resolved ${resolved.streams.length} streams:`);
                        resolved.streams.forEach(s => console.log(`    - [${s.quality}] ${s.url}`));
                    }
                }
            } else {
                console.log('No matches with variants found in the first 5 results.');
            }
        }
    } catch (error) {
        console.error('Test failed:', error);
    } finally {
        console.log('Closing browser...');
        await closeBrowser();
    }
}

testLiveTV();
