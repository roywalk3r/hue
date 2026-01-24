const { getNewPage } = require("../utils/browserManager");

const BASE_URL = "https://vidsrc.cc";

// Main function to get streams using Playwright
async function getVidsrcStreams(tmdbId, mediaType, season, episode, imdbId) {
    console.log(`[VidsrcCC] Called with tmdbId=${tmdbId}, mediaType=${mediaType}, season=${season}, episode=${episode}, imdbId=${imdbId}`);
    const streams = [];
    let context;
    
    try {
        // Build the URL
        let url;
        if (mediaType === "movie") {
            url = `${BASE_URL}/v2/embed/movie/${tmdbId}`;
        } else if (mediaType === "anime") {
            url = `${BASE_URL}/v2/embed/anime/${tmdbId}/${season}/${episode}`;
        } else {
            // For TV series, prefer IMDB ID if available, otherwise use TMDB ID
            const id = imdbId || tmdbId;
            url = `${BASE_URL}/v2/embed/tv/${id}/${season}/${episode}`;
        }
        
        console.log(`[VidsrcCC] Fetching: ${url}`);
        
        const page = await getNewPage({
            extraHTTPHeaders: {
                "Accept-Language": "en-US,en;q=0.9",
                "Accept": "text/html,application/xhtml+xml,application/xml;q=0.9,image/webp,*/*;q=0.8",
                "Accept-Encoding": "gzip, deflate, br",
                "Connection": "keep-alive",
                "Upgrade-Insecure-Requests": "1"
            }
        });
        context = page.context();

        const lightningboltUrls = new Set();
        const vidboxUrls = new Set();
        const allM3u8Urls = new Set();
        
        // Intercept requests
        page.on("request", request => {
            const url = request.url();
            if (url.includes("lightningbolt.site")) {
                console.log(`[VidsrcCC] Found lightningbolt request: ${url}`);
                lightningboltUrls.add(url);
            } else if (url.includes("vidbox.site")) {
                console.log(`[VidsrcCC] Found vidbox request: ${url}`);
                vidboxUrls.add(url);
            }
            
            if (url.includes(".m3u8")) {
                console.log(`[VidsrcCC] Found m3u8 request: ${url}`);
                allM3u8Urls.add(url);
            }
        });
        
        page.on("response", response => {
            const url = response.url();
            if (url.includes("lightningbolt.site")) {
                console.log(`[VidsrcCC] Lightningbolt response status: ${response.status()}`);
            }
        });
        
        console.log("[VidsrcCC] Navigating to page...");
        await page.goto(url, { waitUntil: "networkidle", timeout: 30000 });
        
        console.log("[VidsrcCC] Waiting for page to fully load...");
        await page.waitForTimeout(5000);
        
        try {
            await page.waitForSelector('#servers .dropdown-item', { timeout: 10000 });
        } catch (e) {
            console.log("[VidsrcCC] Servers dropdown items not found within timeout");
        }
        
        const html = await page.content();
        console.log(`[VidsrcCC] Page loaded, HTML length: ${html.length}`);
        
        const servers = await page.evaluate(() => {
            const serverElements = document.querySelectorAll('#servers .dropdown-item');
            const servers = [];
            serverElements.forEach(el => {
                const name = el.textContent.trim();
                const hash = el.getAttribute('data-hash');
                const isActive = el.classList.contains('active');
                if (name && hash) servers.push({ name, hash, isActive });
            });
            return servers;
        });
        
        console.log(`[VidsrcCC] Found ${servers.length} servers`);
        
        const allUrls = [...lightningboltUrls, ...vidboxUrls, ...allM3u8Urls];
        if (allUrls.length > 0) {
            for (const streamUrl of allUrls) {
                const urlWithoutQuery = streamUrl.split('?')[0];
                if (urlWithoutQuery.endsWith('.m3u8')) {
                    const activeServer = servers.find(s => s.isActive);
                    streams.push({
                        url: streamUrl,
                        provider: "VidsrcCC",
                        quality: activeServer ? activeServer.name : "Default",
                        headers: {
                            "Referer": BASE_URL,
                            "Origin": BASE_URL,
                            "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36"
                        }
                    });
                    break;
                }
            }
        }
        
        for (const server of servers) {
            if (server.isActive) continue;
            
            console.log(`[VidsrcCC] Trying server: ${server.name}`);
            lightningboltUrls.clear();
            vidboxUrls.clear();
            allM3u8Urls.clear();
            
            try {
                await page.click('#server-btn');
                await page.waitForTimeout(1000);
                await page.evaluate((hash) => {
                    const item = document.querySelector(`.dropdown-item[data-hash="${hash}"]`);
                    if (item) item.click();
                }, server.hash);
                await page.waitForTimeout(5000);
            } catch (e) {
                continue;
            }
            
            const serverUrls = [...lightningboltUrls, ...vidboxUrls, ...allM3u8Urls];
            if (serverUrls.length > 0) {
                for (const streamUrl of serverUrls) {
                    const urlWithoutQuery = streamUrl.split('?')[0];
                    if (urlWithoutQuery.endsWith('.m3u8')) {
                        const subtitles = await page.evaluate(() => {
                            const subtitleTracks = [];
                            const tracks = document.querySelectorAll('track[kind="subtitles"]');
                            tracks.forEach(track => {
                                if (track.src) {
                                    subtitleTracks.push({
                                        url: track.src,
                                        lang: track.srclang || 'en',
                                        label: track.label || track.srclang || 'English'
                                    });
                                }
                            });
                            return subtitleTracks;
                        });
                        
                        streams.push({
                            url: streamUrl,
                            provider: "VidsrcCC",
                            quality: server.name,
                            subtitles: subtitles,
                            headers: {
                                "Referer": BASE_URL,
                                "Origin": BASE_URL,
                                "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36"
                            }
                        });
                        break;
                    }
                }
            }
        }
        
        return streams;
        
    } catch (e) {
        console.error("[VidsrcCC] Error:", e.message);
        return [];
    } finally {
        if (context) await context.close();
    }
}

module.exports = { getVidsrcStreams };