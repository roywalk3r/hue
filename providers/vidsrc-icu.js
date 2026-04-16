const { getNewPage } = require("../utils/browserManager");

const BASE_URL = "https://vidsrcme.vidsrc.icu/embed";

async function getStreams(tmdbId, mediaType, season, episode) {
    const url = mediaType === 'tv' 
        ? `${BASE_URL}/tv?tmdb=${tmdbId}&season=${season}&episode=${episode}`
        : `${BASE_URL}/movie?tmdb=${tmdbId}`;

    console.log(`[VidsrcICU] Fetching (Browser): ${url}`);
    
    const streams = [];
    let page;
    let context;

    try {
        // Use firefox to match Vidsrc behavior
        page = await getNewPage({ browserType: 'firefox' });
        context = page.context();
        
        let hlsUrl = null;
        let hlsReferer = null;
        const subtitles = [];

        page.on('request', req => {
            const u = req.url();
            if (u.includes('.m3u8') && !u.includes('{v')) {
                if(!hlsUrl) {
                    hlsUrl = u;
                    hlsReferer = req.frame().url(); // Capture the correct frame URL
                    console.log(`[VidsrcICU] Captured HLS: ${u.substring(0, 60)}...`);
                }
            }
            if ((u.includes('.vtt') || u.includes('.srt')) && !subtitles.some(s => s.url === u)) {
                subtitles.push({ url: u, lang: 'en', label: 'English' });
            }
        });

        await page.goto(url, { waitUntil: 'commit', timeout: 30000 });
        
        // Wait for player load and potential Turnstile
        const maxWait = 25000;
        const startWait = Date.now();
        
        while (Date.now() - startWait < maxWait && !hlsUrl) {
            // Try to find the play button in iframes (rcp)
            for (const frame of page.frames()) {
                const fUrl = frame.url();
                if (fUrl.includes('/rcp/') || fUrl.includes('cloudnestra')) {
                    try {
                        await frame.evaluate(() => {
                            const btn = document.querySelector('#pl_but') || document.querySelector('.play-btn');
                            if (btn) btn.click();
                        });
                    } catch(e) {}
                }
            }
            await new Promise(r => setTimeout(r, 1000));
        }

        if (hlsUrl) {
            streams.push({
                url: hlsUrl,
                quality: "Auto",
                provider: "VidsrcICU",
                subtitles: subtitles.length > 0 ? subtitles : undefined,
                headers: {
                    "Referer": hlsReferer,
                    "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:121.0) Gecko/20100101 Firefox/121.0"
                }
            });
        }

    } catch (error) {
        console.error(`[VidsrcICU] Error: ${error.message}`);
    } finally {
        if (context) await context.close();
    }

    return streams;
}

module.exports = { getStreams };