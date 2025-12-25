const puppeteer = require('puppeteer-extra');
const StealthPlugin = require('puppeteer-extra-plugin-stealth');

// Use stealth plugin
puppeteer.use(StealthPlugin());

const BASE_URL = "https://vidsrc.cc";

// Main function to get streams using Puppeteer
async function getVidsrcStreams(tmdbId, mediaType, season, episode, imdbId) {
    console.log(`[VidsrcCC] Called with tmdbId=${tmdbId}, mediaType=${mediaType}, season=${season}, episode=${episode}, imdbId=${imdbId}`);
    const streams = [];
    let browser = null;
    
    // Retry with different proxies
    const maxRetries = 3;
    for (let attempt = 0; attempt < maxRetries; attempt++) {
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
        
        console.log(`[VidsrcCC] Launching Puppeteer to fetch: ${url} (attempt ${attempt + 1}/${maxRetries})`);
        
        // Launch browser with minimal stable settings for Render
        browser = await puppeteer.launch({
            headless: true,
            executablePath: '/usr/bin/chromium-browser',
            args: [
                "--no-sandbox",
                "--disable-setuid-sandbox",
                "--disable-dev-shm-usage",
                "--disable-accelerated-2d-canvas",
                "--no-first-run",
                "--no-zygote",
                "--disable-gpu",
                "--disable-blink-features=AutomationControlled",
                "--disable-web-security",
                "--disable-features=IsolateOrigins,site-per-process",
                "--disable-site-isolation-trials"
            ],
            dumpio: true
        });
        
        const page = await browser.newPage();
        
        // Hide webdriver
        await page.evaluateOnNewDocument(() => {
            Object.defineProperty(navigator, "webdriver", {
                get: () => undefined,
            });
        });
        
        // Set up request interception for all frames
        const lightningboltUrls = new Set();
        const vidboxUrls = new Set();
        const allM3u8Urls = new Set();
        
        // Intercept requests from the main page and all iframes
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
        
        // Also set up interception for any iframes that might be created
        page.on("frameattached", frame => {
            console.log(`[VidsrcCC] Frame attached: ${frame.url()}`);
            frame.on("request", request => {
                const url = request.url();
                if (url.includes("lightningbolt.site")) {
                    console.log(`[VidsrcCC] Found lightningbolt request in frame: ${url}`);
                    lightningboltUrls.add(url);
                } else if (url.includes("vidbox.site")) {
                    console.log(`[VidsrcCC] Found vidbox request in frame: ${url}`);
                    vidboxUrls.add(url);
                }
                
                // Capture all m3u8 URLs
                if (url.includes(".m3u8")) {
                    console.log(`[VidsrcCC] Found m3u8 request in frame: ${url}`);
                    allM3u8Urls.add(url);
                }
            });
        });
        
        // Set user agent and other headers
        await page.setUserAgent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/142.0.0.0 Safari/537.36");
        await page.setExtraHTTPHeaders({
            "Accept-Language": "en-US,en;q=0.9",
            "Accept": "text/html,application/xhtml+xml,application/xml;q=0.9,image/webp,*/*;q=0.8",
            "Accept-Encoding": "gzip, deflate, br",
            "DNT": "1",
            "Connection": "keep-alive",
            "Upgrade-Insecure-Requests": "1",
            "Sec-Fetch-Dest": "document",
            "Sec-Fetch-Mode": "navigate",
            "Sec-Fetch-Site": "none",
            "Sec-Fetch-User": "?1",
            "Cache-Control": "max-age=0",
            "sec-ch-ua": '"Chromium";v="142", "Google Chrome";v="142", "Not:A-Brand";v="24"',
            "sec-ch-ua-mobile": "?0",
            "sec-ch-ua-platform": '"Linux"'
        });
        
        // Navigate to the page
        console.log("[VidsrcCC] Navigating to page...");
        await page.goto(url, { 
            waitUntil: "networkidle2",
            timeout: 30000 
        });
        
        // Wait for content to load
        console.log("[VidsrcCC] Waiting for page to fully load...");
        await new Promise(resolve => setTimeout(resolve, 10000));
        
        // Check if we are on a Cloudflare challenge page
        const isCloudflarePage = await page.evaluate(() => {
            return document.title.includes("Just a moment") || 
                   (document.body && document.body.innerHTML.includes("Cloudflare")) ||
                   (document.body && document.body.innerHTML.includes("ray_id"));
        });
        
        if (isCloudflarePage) {
            console.log("[VidsrcCC] Cloudflare challenge detected, waiting longer...");
            await new Promise(resolve => setTimeout(resolve, 15000));
            
            // Check again
            const stillCloudflare = await page.evaluate(() => {
                return document.title.includes("Just a moment") || 
                       (document.body && document.body.innerHTML.includes("ray_id"));
            });
            
            if (stillCloudflare) {
                console.log("[VidsrcCC] Still on Cloudflare page, giving up");
                return [];
            }
        }
        
        // Get the HTML content
        const html = await page.content();
        console.log(`[VidsrcCC] Page loaded, HTML length: ${html.length}`);
        
        // Save HTML for debugging
        require('fs').writeFileSync('/tmp/vidsrc.html', html);
        
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
                            "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36"
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
                await new Promise(resolve => setTimeout(resolve, 1000));
                
                // Use page.evaluate to click the server item
                await page.evaluate((hash) => {
                    const item = document.querySelector(`.dropdown-item[data-hash="${hash}"]`);
                    if (item) {
                        item.click();
                    }
                }, server.hash);
                console.log(`[VidsrcCC] Clicked server: ${server.name}`);
                await new Promise(resolve => setTimeout(resolve, 5000));
            } catch (e) {
                console.log(`[VidsrcCC] Failed to click server ${server.name}:`, e.message);
                continue;
            }
            
            // Wait for stream to load
            await new Promise(resolve => setTimeout(resolve, 3000));
            
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
                            
                            // Also check for subtitle data in video element
                            const video = document.querySelector('video');
                            if (video && video.textTracks) {
                                for (let i = 0; i < video.textTracks.length; i++) {
                                    const textTrack = video.textTracks[i];
                                    if (textTrack.kind === 'subtitles' && textTrack.cues && textTrack.cues.length > 0) {
                                        subtitleTracks.push({
                                            url: null, // Embedded subtitles
                                            lang: textTrack.language || 'en',
                                            label: textTrack.label || `Track ${i + 1}`
                                        });
                                    }
                                }
                            }
                            
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
                                "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36"
                            }
                        });
                        console.log(`[VidsrcCC] Added stream for server ${server.name} with ${subtitles.length} subtitles: ${streamUrl.substring(0, 100)}...`);
                        break; // Only take one stream per server
                    }
                }
            }
        }
        
        // For anime, extract vidbox iframe URL if no m3u8 found
        if (mediaType === "anime" && streams.length === 0) {
            console.log(`[VidsrcCC] Checking for vidbox iframe for anime`);
            const iframeUrl = await page.evaluate(() => {
                const iframe = document.querySelector('#b-player');
                return iframe ? iframe.src : null;
            });
            
            if (iframeUrl && iframeUrl.includes('vidbox.site')) {
                console.log(`[VidsrcCC] Found vidbox iframe: ${iframeUrl}`);
                
                // Navigate to the vidbox iframe to capture m3u8
                console.log(`[VidsrcCC] Navigating to vidbox iframe...`);
                await page.goto(iframeUrl, { waitUntil: 'networkidle2', timeout: 30000 });
                await new Promise(resolve => setTimeout(resolve, 5000));
                
                // Check if we got any m3u8 URLs
                const m3u8Url = Array.from(allM3u8Urls).find(url => url.split('?')[0].endsWith('.m3u8'));
                if (m3u8Url) {
                    // Extract subtitles for vidbox stream
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
                        
                        // Also check for subtitle data in video element
                        const video = document.querySelector('video');
                        if (video && video.textTracks) {
                            for (let i = 0; i < video.textTracks.length; i++) {
                                const textTrack = video.textTracks[i];
                                if (textTrack.kind === 'subtitles' && textTrack.cues && textTrack.cues.length > 0) {
                                    subtitleTracks.push({
                                        url: null, // Embedded subtitles
                                        lang: textTrack.language || 'en',
                                        label: textTrack.label || `Track ${i + 1}`
                                    });
                                }
                            }
                        }
                        
                        return subtitleTracks;
                    });
                    
                    streams.push({
                        url: m3u8Url,
                        provider: "VidsrcCC",
                        quality: "Default",
                        subtitles: subtitles,
                        headers: {
                            "Referer": "https://lucky.vidbox.site/",
                            "Origin": "https://lucky.vidbox.site",
                            "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36"
                        }
                    });
                    console.log(`[VidsrcCC] Added vidbox stream with ${subtitles.length} subtitles: ${m3u8Url.substring(0, 100)}...`);
                }
            }
        }
        
        console.log(`[VidsrcCC] Returning ${streams.length} streams`);
            return streams;
            
        } catch (e) {
            console.error(`[VidsrcCC] Error on attempt ${attempt + 1}:`, e.message);
            if (attempt === maxRetries - 1) {
                console.error("[VidsrcCC] Max retries reached");
                return [];
            }
            console.log("[VidsrcCC] Retrying with different proxy...");
            await new Promise(resolve => setTimeout(resolve, 2000)); // Wait 2 seconds before retry
        } finally {
            if (browser) {
                await browser.close();
                browser = null;
            }
        }
    }
}

module.exports = {
    getVidsrcStreams
};
