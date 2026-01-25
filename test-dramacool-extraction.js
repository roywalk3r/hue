const { getStreamsByUrl } = require('./providers/dramacool');

(async () => {
    const slug = "fourever-you-season-2-2025";
    const episode = "1";
    const url = `https://dramacool.sh/${slug}-episode-${episode}/`;

    console.log(`Testing Dramacool Stream Extraction for: ${url}`);
    try {
        const streams = await getStreamsByUrl(url);
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
