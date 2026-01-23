const { getNewContext } = require("../utils/browserManager");

const BASE_URL = "https://www.showbox.media";

// Main function to get streams using Playwright
async function getStreamsFromTmdbId(tmdbId, mediaType, season, episode, imdbId, userCookie) {
    console.log(`[ShowBoxPlaywright] Called with tmdbId=${tmdbId}, mediaType=${mediaType}, season=${season}, episode=${episode}`);
    const streams = [];
    let context;
    
    try {
        // Get TMDB data for title
        const { getTmdbApiKey } = require("../utils/tmdbKey");
        const TMDB_API_KEY = getTmdbApiKey();
        if (!TMDB_API_KEY) {
            console.log("[ShowBoxPlaywright] No TMDB API key available");
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
            console.log("[ShowBoxPlaywright] No title found from TMDB");
            return streams;
        }
        
        console.log(`[ShowBoxPlaywright] Searching for: ${title} (${year})`);
        
        context = await getNewContext();
        const page = await context.newPage();
        
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
                console.log(`[ShowBoxPlaywright] Found stream request: ${url}`);
                streamUrls.add(url);
                
                if (url.includes(".m3u8")) {
                    m3u8Urls.add(url);
                }
            }
        });
        
        page.on("response", response => {
            const url = response.url();
            if (url.includes("lightningbolt.site") || url.includes(".m3u8")) {
                console.log(`[ShowBoxPlaywright] Stream response status: ${response.status()}`);
            }
        });
        
        // Navigate to search page
        const searchQuery = year ? `${title} ${year}` : title;
        const searchUrl = `${BASE_URL}/search?keyword=${encodeURIComponent(searchQuery)}`;
        console.log(`[ShowBoxPlaywright] Navigating to search: ${searchUrl}`);
        
        await page.goto(searchUrl, {
            waitUntil: "networkidle",
            timeout: 60000
        });
        
        // Handle Cloudflare if present
        const pageContent = await page.content();
        const isCloudflareChallenge = pageContent.includes('Cloudflare') && 
                                    (pageContent.includes('Checking your browser') || 
                                     pageContent.includes('DDoS protection'));
        
        if (isCloudflareChallenge) {
            console.log(`[ShowBoxPlaywright] Cloudflare challenge detected, waiting...`);
            await page.waitForTimeout(15000); // Wait 15 seconds
        }
        
        // Wait for search results to load
        await page.waitForTimeout(3000);
        
        // Debug: Log page content
        const searchPageContent = await page.content();
        console.log(`[ShowBoxPlaywright] Search page title: ${await page.title()}`);
        console.log(`[ShowBoxPlaywright] Search page content length: ${searchPageContent.length}`);
        
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
        
        console.log(`[ShowBoxPlaywright] Found ${searchResults.length} search results`);
        
        if (searchResults.length === 0) {
            console.log(`[ShowBoxPlaywright] No search results found`);
            await browser.close();
            return streams;
        }
        
        // Find the best match (simple title matching for now)
        const bestMatch = searchResults.find(result => 
            result.title.toLowerCase().includes(title.toLowerCase()) ||
            title.toLowerCase().includes(result.title.toLowerCase())
        ) || searchResults[0];
        
        const movieUrl = `${BASE_URL}${bestMatch.href}`;
        console.log(`[ShowBoxPlaywright] Navigating to movie page: ${movieUrl}`);
        
        // Navigate to the movie page
        await page.goto(movieUrl, {
            waitUntil: "networkidle",
            timeout: 60000
        });
        
        // Wait for content to load
        await page.waitForTimeout(5000);
        
        // Look for FebBox URLs in the page
        const finalContent = await page.content();
        const febboxRegex = /https?:\/\/(?:www\.)?febbox\.com\/(?:s\/|share\/)?([a-zA-Z0-9]+)/g;
        const febboxMatches = finalContent.match(febboxRegex) || [];
        
        console.log(`[ShowBoxPlaywright] Found ${febboxMatches.length} FebBox URLs`);
        
        if (febboxMatches.length > 0) {
            // Import the FebBox stream extraction function
            const { fetchSourcesForSingleFid } = require('./Showbox');
            
            for (const febboxUrl of febboxMatches) {
                try {
                    console.log(`[ShowBoxPlaywright] Extracting streams from FebBox: ${febboxUrl}`);
                    
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
                                provider: "ShowBox Playwright",
                                headers: {}
                            });
                        }
                    }
                } catch (e) {
                    console.error(`[ShowBoxPlaywright] Error extracting from FebBox: ${e.message}`);
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
                provider: "ShowBox Playwright",
                headers: {}
            });
        }
        
        console.log(`[ShowBoxPlaywright] Found ${streams.length} total streams`);
        
    } catch (error) {
        console.error(`[ShowBoxPlaywright] Error: ${error.message}`);
    } finally {
        if (context) {
            await context.close();
        }
    }
    
    return streams;
}

module.exports = {
    getStreamsFromTmdbId
};