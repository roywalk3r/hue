const puppeteer = require("puppeteer-extra");
const StealthPlugin = require("puppeteer-extra-plugin-stealth");

// Use stealth plugin
puppeteer.use(StealthPlugin());

const BASE_URL = "https://www.showbox.media";

// Main function to get streams using Puppeteer
async function getStreamsFromTmdbId(tmdbId, mediaType, season, episode, imdbId, userCookie) {
    console.log(`[ShowBoxPuppeteer] Called with tmdbId=${tmdbId}, mediaType=${mediaType}, season=${season}, episode=${episode}`);
    const streams = [];
    let browser;
    
    try {
        // Get TMDB data for title
        const { getTmdbApiKey } = require("../utils/tmdbKey");
        const TMDB_API_KEY = getTmdbApiKey();
        if (!TMDB_API_KEY) {
            console.log("[ShowBoxPuppeteer] No TMDB API key available");
            return streams;
        }
        
        const axios = require("axios");
        const tmdbApiUrl = `https://api.themoviedb.org/3/${mediaType}/${tmdbId}?api_key=${TMDB_API_KEY}`;
        const response = await axios.get(tmdbApiUrl, { timeout: 10000 });
        const tmdbData = response.data;
        
        const title = tmdbData.title || tmdbData.name;
        const year = tmdbData.release_date ? new Date(tmdbData.release_date).getFullYear() : 
                     tmdbData.first_air_date ? new Date(tmdbData.first_air_date).getFullYear() : null;
        
        if (!title) {
            console.log("[ShowBoxPuppeteer] No title found from TMDB");
            return streams;
        }
        
        console.log(`[ShowBoxPuppeteer] Searching for: ${title} (${year})`);
        
        // Launch browser with stealth settings
        browser = await puppeteer.launch({
            headless: true,
            args: [
                "--no-sandbox",
                "--disable-setuid-sandbox",
                "--disable-dev-shm-usage",
                "--disable-accelerated-2d-canvas",
                "--no-first-run",
                "--no-zygote",
                "--single-process",
                "--disable-gpu",
                "--disable-blink-features=AutomationControlled",
                "--disable-features=VizDisplayCompositor"
            ]
        });
        
        const page = await browser.newPage();
        
        // Set viewport and user agent
        await page.setViewport({ width: 1920, height: 1080 });
        await page.setUserAgent('Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36');
        
        // Hide webdriver
        await page.evaluateOnNewDocument(() => {
            Object.defineProperty(navigator, "webdriver", {
                get: () => undefined,
            });
        });
        
        // Set up request interception
        const streamUrls = new Set();
        const m3u8Urls = new Set();
        
        page.on("request", request => {
            const url = request.url();
            
            // Capture stream URLs from various domains
            if (url.includes(".m3u8") || 
                url.includes("lightningbolt.site") ||
                url.includes("vidbox.site") ||
                url.includes("febbox.com") ||
                url.includes("pstream.mov")) {
                console.log(`[ShowBoxPuppeteer] Found stream request: ${url}`);
                streamUrls.add(url);
                
                if (url.includes(".m3u8")) {
                    m3u8Urls.add(url);
                }
            }
        });
        
        page.on("response", response => {
            const url = response.url();
            if (url.includes("lightningbolt.site") || url.includes(".m3u8")) {
                console.log(`[ShowBoxPuppeteer] Stream response status: ${response.status()}`);
            }
        });
        
        // Navigate to search page
        const searchQuery = year ? `${title} ${year}` : title;
        const searchUrl = `${BASE_URL}/search?keyword=${encodeURIComponent(searchQuery)}`;
        console.log(`[ShowBoxPuppeteer] Navigating to search: ${searchUrl}`);
        
        await page.goto(searchUrl, {
            waitUntil: "networkidle2",
            timeout: 60000
        });
        
        // Handle Cloudflare if present
        const pageContent = await page.content();
        const isCloudflareChallenge = pageContent.includes('Cloudflare') && 
                                    (pageContent.includes('Checking your browser') || 
                                     pageContent.includes('DDoS protection'));
        
        if (isCloudflareChallenge) {
            console.log(`[ShowBoxPuppeteer] Cloudflare challenge detected, waiting...`);
            await new Promise(resolve => setTimeout(resolve, 15000)); // Wait 15 seconds
        }
        
        // Wait for search results to load
        await new Promise(resolve => setTimeout(resolve, 3000));
        
        // Debug: Log page content
        const searchPageContent = await page.content();
        console.log(`[ShowBoxPuppeteer] Search page title: ${await page.title()}`);
        console.log(`[ShowBoxPuppeteer] Search page content length: ${searchPageContent.length}`);
        
        // Check if we're still on Cloudflare or got blocked
        if (searchPageContent.includes('Cloudflare')) {
            console.log(`[ShowBoxPuppeteer] Still on Cloudflare page after wait`);
        }
        
        // Log a snippet of the page for debugging
        const contentSnippet = searchPageContent.substring(0, 1000);
        console.log(`[ShowBoxPuppeteer] Search page snippet: ${contentSnippet}`);
        
        // Parse search results
        const searchResults = await page.evaluate(() => {
            const results = [];
            const items = document.querySelectorAll('a[href*="/movie/"], a[href*="/tv/"]');
            
            items.forEach(item => {
                const href = item.getAttribute('href');
                const titleElement = item.querySelector('h3, .title, [class*="title"]');
                const title = titleElement ? titleElement.textContent.trim() : '';
                
                if (href && title) {
                    results.push({
                        href: href,
                        title: title
                    });
                }
            });
            
            return results;
        });
        
        console.log(`[ShowBoxPuppeteer] Found ${searchResults.length} search results`);
        
        if (searchResults.length === 0) {
            console.log(`[ShowBoxPuppeteer] No search results found`);
            return streams;
        }
        
        // Find the best match (simple title matching for now)
        const bestMatch = searchResults.find(result => 
            result.title.toLowerCase().includes(title.toLowerCase()) ||
            title.toLowerCase().includes(result.title.toLowerCase())
        ) || searchResults[0];
        
        const movieUrl = `${BASE_URL}${bestMatch.href}`;
        console.log(`[ShowBoxPuppeteer] Navigating to movie page: ${movieUrl}`);
        
        // Navigate to the movie page
        await page.goto(movieUrl, {
            waitUntil: "networkidle2",
            timeout: 60000
        });
        
        // Wait for content to load
        await new Promise(resolve => setTimeout(resolve, 5000));
        
        // Look for FebBox URLs in the page
        const finalContent = await page.content();
        const febboxRegex = /https?:\/\/(?:www\.)?febbox\.com\/(?:s\/|share\/)?([a-zA-Z0-9]+)/g;
        const febboxMatches = finalContent.match(febboxRegex) || [];
        
        console.log(`[ShowBoxPuppeteer] Found ${febboxMatches.length} FebBox URLs`);
        
        if (febboxMatches.length > 0) {
            // Import the FebBox stream extraction function
            const { fetchSourcesForSingleFid } = require('./Showbox');
            
            for (const febboxUrl of febboxMatches) {
                try {
                    console.log(`[ShowBoxPuppeteer] Extracting streams from FebBox: ${febboxUrl}`);
                    
                    // Extract share key from URL
                    const shareKeyMatch = febboxUrl.match(/\/(?:s\/|share\/)?([a-zA-Z0-9]+)/);
                    const shareKey = shareKeyMatch ? shareKeyMatch[1] : null;
                    
                    if (shareKey) {
                        // Use the FebBox cookie from the registry
                        const { config } = require('../utils/config');
                        const cookies = config.febboxCookies || [];
                        const selectedCookie = cookies.length > 0 ? cookies[0] : null;
                        
                        // Use existing FebBox logic to get streams with proper cookie
                        const febboxStreams = await fetchSourcesForSingleFid(null, shareKey, null, selectedCookie);
                        
                        for (const source of febboxStreams) {
                            streams.push({
                                name: `ShowBox ${source.quality}`,
                                title: `ShowBox ${source.quality}`,
                                url: source.url,
                                quality: source.quality,
                                provider: "ShowBox Puppeteer",
                                headers: {}
                            });
                        }
                    }
                } catch (e) {
                    console.error(`[ShowBoxPuppeteer] Error extracting from FebBox: ${e.message}`);
                }
            }
        }
        
        // Also add any direct streams found
        for (const url of streamUrls) {
            let quality = "Unknown";
            
            if (url.includes("1080p") || url.includes("1920")) quality = "1080p";
            else if (url.includes("720p") || url.includes("1280")) quality = "720p";
            else if (url.includes("480p") || url.includes("854")) quality = "480p";
            else if (url.includes("360p") || url.includes("640")) quality = "360p";
            else if (url.includes("4k") || url.includes("2160p")) quality = "4K";
            
            streams.push({
                name: `ShowBox ${quality}`,
                title: `ShowBox ${quality}`,
                url: url,
                quality: quality,
                provider: "ShowBox Puppeteer",
                headers: {}
            });
        }
        
        console.log(`[ShowBoxPuppeteer] Found ${streams.length} total streams`);
        
    } catch (error) {
        console.error(`[ShowBoxPuppeteer] Error: ${error.message}`);
    } finally {
        if (browser) {
            await browser.close();
        }
    }
    
    return streams;
}

module.exports = {
    getStreamsFromTmdbId
};
