const hianime = require('./services/hianime');

(async () => {
    try {
        console.log('--- HIANIME 2026 FULL FLOW TEST (v2) ---');
        
        // 1. SEARCH
        const query = 'Naruto';
        console.log(`[Step 1] Searching for: ${query}...`);
        const searchResults = await hianime.search(query);
        const bestMatch = searchResults[0];
        const animeId = bestMatch._id;
        console.log(`[Step 1] SUCCESS: Found "${bestMatch.English}" (ID: ${animeId})`);
        
        // 2. GET EPISODES
        console.log(`[Step 2] Fetching episode list for ${animeId}...`);
        const epData = await hianime.getEpisodes(animeId);
        
        if (!epData || !epData.episodes || epData.episodes.length === 0) {
            throw new Error('No episodes returned for this anime');
        }
        
        const firstEp = epData.episodes[0];
        console.log(`[Step 2] SUCCESS: Found ${epData.episodes.length} episodes. Target: Ep ${firstEp.episode_number}`);

        // 3. GET SERVERS (Using the specific episode slug)
        const epSlug = firstEp.slug; 
        console.log(`[Step 3] Fetching servers for episode slug: ${epSlug}...`);
        const servers = await hianime.getServers(epSlug);
        
        if (!servers.sub?.length && !servers.dub?.length) {
            throw new Error('No servers returned for this episode slug');
        }
        console.log(`[Step 3] SUCCESS: Found ${servers.sub?.length || 0} sub servers.`);

        // 4. GET STREAM
        const targetServer = servers.sub[0].name;
        console.log(`[Step 4] Extracting stream from ${targetServer}...`);
        const result = await hianime.getAnimeStream({ id: epSlug, server: targetServer, type: 'sub' });

        if (result && result.stream?.link?.file) {
            console.log('\n--- FULL FLOW SUCCESS ---');
            console.log('Title:', bestMatch.English);
            console.log('Episode:', firstEp.episode_number);
            console.log('Stream URL:', result.stream.link.file);
            console.log('-------------------------\n');
        } else {
            console.log('\n--- FLOW FAILED AT EXTRACTION ---');
            console.log(JSON.stringify(result, null, 2));
        }

    } catch (err) {
        console.error('\n❌ FLOW ERROR:', err.message);
    } finally {
        process.exit();
    }
})();
