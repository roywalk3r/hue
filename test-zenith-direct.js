const { getStreamsDirect } = require('./providers/kisskh');
const { closeBrowser } = require('./utils/browserManager');

(async () => {
    const showId = "9441";
    const episode = 1;
    console.log(`Testing Zenith (KissKH) Direct Extraction for ID: ${showId}, Episode: ${episode}`);
    
    try {
        const streams = await getStreamsDirect(showId, episode);
        console.log("Extracted Streams:", JSON.stringify(streams, null, 2));
        
        if (streams.length > 0) {
            console.log("\n--- Subtitle Verification ---");
            streams.forEach((s, i) => {
                console.log(`Stream ${i+1}: ${s.url.substring(0, 60)}...`);
                if (s.subtitles && s.subtitles.length > 0) {
                    console.log(`Found ${s.subtitles.length} subtitles:`);
                    s.subtitles.forEach(sub => console.log(`  - [${sub.lang}] ${sub.label}: ${sub.url.substring(0, 100)}...`));
                } else {
                    console.log("No subtitles found.");
                }
            });
            console.log("\nSUCCESS: Extraction test completed.");
        } else {
            console.log("\nFAILURE: No streams found.");
        }
    } catch (e) {
        console.error("Test Error:", e);
    } finally {
        await closeBrowser();
    }
})();

