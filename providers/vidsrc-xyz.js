const { getNewPage } = require("../utils/browserManager");

const BASE_URL = "https://vidsrc.xyz/embed";

/**
 * Vidsrc.xyz Provider (Optimized for VPS)
 * Uses Firefox via BrowserManager to bypass Chromium-specific blocks
 */
async function getStreams(tmdbId, mediaType, season, episode) {
    const url = mediaType === 'tv' 
        ? `${BASE_URL}/tv/${tmdbId}/${season}/${episode}`
        : `${BASE_URL}/movie/${tmdbId}`;

    console.log(`[VidsrcXYZ] Fetching (Firefox Queue): ${url}`);
    
    const streams = [];
    let page;
    let context;

    try {
        // Use firefox via the global queue
        page = await getNewPage({ browserType: 'firefox' });
        context = page.context();
        
        let hlsUrl = null;
        let hlsReferer = null;
        const subtitles = [];

        page.on('response', response => {
            const u = response.url();
            if (u.includes('.m3u8') && !u.includes('{v') && !u.includes('{s') && !hlsUrl) {
                hlsUrl = u;
                hlsReferer = response.request().frame().url(); // Capture the correct frame URL
                console.log(`[VidsrcXYZ] Captured HLS: ${u.substring(0, 60)}...`);
            }
            if ((u.includes('.vtt') || u.includes('.srt')) && !subtitles.some(s => s.url === u)) {
                subtitles.push({ url: u, lang: 'en', label: 'English' });
            }
        });

        await page.goto(url, { waitUntil: 'commit', timeout: 30000 });
        
        // Wait for initial load
        await new Promise(r => setTimeout(r, 2000));

        // Logic to handle frames and play button
        const maxWait = 25000;
        const startWait = Date.now();
        
        while (Date.now() - startWait < maxWait && !hlsUrl) {
            const frames = page.frames();
            for (const frame of frames) {
                const frameUrl = frame.url();
                if (frameUrl.includes('cloudnestra') || frameUrl.includes('/rcp/')) {
                    await frame.evaluate(() => {
                        const selectors = ['#pl_but', '#pl_but_background', '.play-btn'];
                        for (const sel of selectors) {
                            const btn = document.querySelector(sel);
                            if (btn) {
                                btn.click();
                                return true;
                            }
                        }
                        return false;
                    }).catch(() => {});
                }
            }
            if (hlsUrl) break;
            await new Promise(r => setTimeout(r, 2000));
        }

        if (hlsUrl) {
            streams.push({
                url: hlsUrl,
                quality: "Auto",
                provider: "VidsrcXYZ",
                subtitles: subtitles.length > 0 ? subtitles : undefined,
                headers: {
                    "Referer": hlsReferer,
                    "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:121.0) Gecko/20100101 Firefox/121.0"
                }
            });
        }

    } catch (error) {
        console.error(`[VidsrcXYZ] Error: ${error.message}`);
    } finally {
        if (context) await context.close();
    }

    return streams;
}

module.exports = { getStreams };