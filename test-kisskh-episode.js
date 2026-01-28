const { getStreamsDirect } = require("./providers/kisskh");
const { closeBrowser } = require("./utils/browserManager");

const showId = "12358";
const episode = 14;

(async () => {
    console.log(`[KissKH Test] showId=${showId} episode=${episode}`);
    try {
        const streams = await getStreamsDirect(showId, episode);
        console.log(JSON.stringify(streams, null, 2));
        if (!streams.length) {
            console.log("[KissKH Test] No streams found.");
        }
    } catch (error) {
        console.error("[KissKH Test] Error:", error);
    } finally {
        await closeBrowser();
    }
})();
