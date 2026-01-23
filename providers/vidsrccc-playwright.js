const { getNewContext } = require("../utils/browserManager");

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
        
        context = await getNewContext({
            extraHTTPHeaders: {
                "Accept-Language": "en-US,en;q=0.9",
                "Accept": "text/html,application/xhtml+xml,application/xml;q=0.9,image/webp,*/*;q=0.8",
                "Accept-Encoding": "gzip, deflate, br",
                "Connection": "keep-alive",
                "Upgrade-Insecure-Requests": "1"
            }
        });

        const page = await context.newPage();
        
        // Set up request interception for all frames
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
            
            // Capture all m3u8 URLs
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
        
        // Navigate to the page
        console.log("[VidsrcCC] Navigating to page...");
        await page.goto(url, { 
            waitUntil: "networkidle",
            timeout: 30000 
        });
        
        // Wait for content to load
        console.log("[VidsrcCC] Waiting for page to fully load...");
        await page.waitForTimeout(5000);
        
        // Wait for servers to be populated if they exist
        try {
            await page.waitForSelector('#servers .dropdown-item', { timeout: 10000 });
        } catch (e) {
            console.log("[VidsrcCC] Servers dropdown items not found within timeout");
        }
        
        // Get the HTML content
        const html = await page.content();
        console.log(`[VidsrcCC] Page loaded, HTML length: ${html.length}`);
        
        // Extract server options from the page
        const servers = await page.evaluate(() => {
            const serverElements = document.querySelectorAll('#servers .dropdown-item');
            const servers = [];
            
            serverElements.forEach(el => {
                const name = el.textContent.trim();
                const hash = el.getAttribute('data-hash');
                const isActive = el.classList.contains('active');
                
                if (name && hash) {
                    servers.push({ name, hash, isActive });
                }
            });
            
            return servers;
        });
        
        console.log(`[VidsrcCC] Found ${servers.length} servers:`, servers.map(s => ({ name: s.name, isActive: s.isActive })));
        
        // First check if we already have a stream from the default server
        const allUrls = [...lightningboltUrls, ...vidboxUrls, ...allM3u8Urls];
        if (allUrls.length > 0) {
            console.log(`[VidsrcCC] Found stream from default server`);
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
                    console.log(`[VidsrcCC] Added stream from default server: ${streamUrl.substring(0, 100)}...`);
                    break;
                }
            }
        }
        
        // Try each inactive server and capture the stream URL
        console.log(`[VidsrcCC] Starting server iteration, total servers: ${servers.length}`);
        for (const server of servers) {
            console.log(`[VidsrcCC] Processing server: ${server.name}, isActive: ${server.isActive}`);
            
            // Skip the active server since we already processed it
            if (server.isActive) {
                console.log(`[VidsrcCC] Skipping active server: ${server.name}`);
                continue;
            }
            
            console.log(`[VidsrcCC] Trying server: ${server.name}`);
            
            // Clear previous URLs
            lightningboltUrls.clear();
            vidboxUrls.clear();
            allM3u8Urls.clear();
            
            try {
                // First open the dropdown menu
                await page.click('#server-btn');
                console.log(`[VidsrcCC] Opened server dropdown`);
                await page.waitForTimeout(1000);
                
                // Use page.evaluate to click the server item
                await page.evaluate((hash) => {
                    const item = document.querySelector(`.dropdown-item[data-hash="${hash}"]`);
                    if (item) {
                        item.click();
                    }
                }, server.hash);
                console.log(`[VidsrcCC] Clicked server: ${server.name}`);
                await page.waitForTimeout(5000);
            } catch (e) {
                console.log(`[VidsrcCC] Failed to click server ${server.name}:`, e.message);
                continue;
            }
            
            // Wait for stream to load
            await page.waitForTimeout(3000);
            
            // Check if we got a stream URL for this server
            const serverUrls = [...lightningboltUrls, ...vidboxUrls, ...allM3u8Urls];
            if (serverUrls.length > 0) {
                for (const streamUrl of serverUrls) {
                    const urlWithoutQuery = streamUrl.split('?')[0];
                    if (urlWithoutQuery.endsWith('.m3u8')) {
                        // Extract subtitles for this stream
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
                        console.log(`[VidsrcCC] Added stream for server ${server.name} with ${subtitles.length} subtitles: ${streamUrl.substring(0, 100)}...`);
                        break; // Only take one stream per server
                    }
                }
            }
        }
        
        console.log(`[VidsrcCC] Returning ${streams.length} streams`);
        return streams;
        
    } catch (e) {
        console.error("[VidsrcCC] Error:", e.message);
        console.error("[VidsrcCC] Stack:", e.stack);
        return [];
    } finally {
        if (context) {
            await context.close();
        }
    }
}

module.exports = {
    getVidsrcStreams
};