const { getStreams } = require('./providers/kisskh');

(async () => {
    const tmdbId = "221112"; // Amidst a Snowstorm of Love
    const mediaType = "series";
    const season = 1;
    const episode = 1;

    console.log(`Testing KissKH Stream Extraction for TMDB ${tmdbId}...`);
    try {
        const streams = await getStreams(tmdbId, mediaType, season, episode);
        console.log("Extracted Streams:", JSON.stringify(streams, null, 2));
        
        if (streams.length > 0) {
            console.log("SUCCESS: Found streams!");
        } else {
            console.log("FAILURE: No streams found.");
        }
    } catch (e) {
        console.error("Test Error:", e);
    }
})();
