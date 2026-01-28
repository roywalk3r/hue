const axios = require('axios');
const cheerio = require('cheerio');
const https = require('https');
const { resolveWithPlaywright } = require('../utils/playwrightResolver');

const BASE_URL = 'https://livetv.sx';
const COMMON_HEADERS = {
    'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36',
    'Referer': BASE_URL,
    'Accept-Language': 'en-US,en;q=0.9',
};

const axiosInstance = axios.create({
    headers: COMMON_HEADERS,
    timeout: 10000,
    validateStatus: () => true,
    httpsAgent: new https.Agent({  
        rejectUnauthorized: false
    })
});

/**
 * 1. Sports List
 */
async function getSports() {
    try {
        const { data } = await axiosInstance.get(`${BASE_URL}/enx/`);
        const $ = cheerio.load(data);
        const sports = [];

        // Sport icons in the sidebar
        $('a[href^="/enx/allupcomingsports/"]').each((i, el) => {
            const href = $(el).attr('href');
            const id = href.split('/').filter(Boolean).pop();
            const img = $(el).find('img');
            const title = img.attr('title');
            const icon = img.attr('src');

            sports.push({
                id,
                title,
                url: `${BASE_URL}${href}`,
                icon: icon.startsWith('//') ? `https:${icon}` : `${BASE_URL}${icon}`
            });
        });

        return sports;
    } catch (error) {
        console.error('LiveTV getSports error:', error.message);
        return [];
    }
}

/**
 * 2. Match Listings
 */
async function getMatches(sportId = null) {
    try {
        const url = sportId 
            ? `${BASE_URL}/enx/allupcomingsports/${sportId}/`
            : `${BASE_URL}/enx/allupcoming/`;

        const { data } = await axiosInstance.get(url);
        const $ = cheerio.load(data);
        const matches = [];

        // Events are usually inside tables with eventinfo links
        $('a[href^="/enx/eventinfo/"]').each((i, el) => {
            const href = $(el).attr('href');
            const title = $(el).text().trim();
            
            if (!title) return; // Skip image-only links

            // Try to find status/time
            const parentTable = $(el).closest('table');
            const evDesc = parentTable.find('.evdesc').text().trim();
            
            // Extract time from evDesc (e.g. "20:45 (Champions League)")
            // or "Today, 21:00" if it was in the older format, but evdesc seems consistent now
            let timeText = evDesc;
            
            const isLive = parentTable.find('img[src*="live.gif"]').length > 0;

            matches.push({
                slug: href.split('/').filter(Boolean).pop(),
                title,
                url: `${BASE_URL}${href}`,
                timeText,
                isLive,
                id: href.match(/eventinfo\/(\d+)/)?.[1]
            });
        });

        // De-duplicate matches by URL
        const uniqueMatches = Array.from(new Map(matches.map(m => [m.url, m])).values());
        return uniqueMatches;

    } catch (error) {
        console.error('LiveTV getMatches error:', error.message);
        return [];
    }
}

/**
 * 3. Match -> Stream Variants
 */
async function getStreamVariants(matchUrl) {
    try {
        const { data } = await axiosInstance.get(matchUrl);
        const $ = cheerio.load(data);
        const variants = [];

        // Stream links are usually in a table below the match info
        // They look like: /enx/export/webplayer.php?t=...
        $('a[href*="/webplayer.php"]').each((i, el) => {
            const href = $(el).attr('href');
            const title = $(el).text().trim() || `Stream ${i + 1}`;
            
            // Check for quality/language icons in the same row
            const row = $(el).closest('tr');
            const quality = row.find('td').eq(3).text().trim(); // heuristic
            const language = row.find('img[src*="/img/lang/"]').attr('title') || 'Unknown';

            variants.push({
                title,
                url: href.startsWith('//') ? `https:${href}` : (href.startsWith('/') ? `${BASE_URL}${href}` : href),
                quality,
                language,
                isLive: true
            });
        });

        return variants;
    } catch (error) {
        console.error('LiveTV getStreamVariants error:', error.message);
        return [];
    }
}

/**
 * 4. Resolve Webplayer -> Direct Link
 */
async function resolveStream(playerUrl) {
    try {
        console.log(`Resolving LiveTV stream: ${playerUrl}`);

        // OPTIMIZATION: Try to extract iframe source directly via HTTP first
        try {
            const { data } = await axiosInstance.get(playerUrl);
            const $ = cheerio.load(data);
            const iframeSrc = $('iframe').attr('src');
            
            if (iframeSrc) {
                console.log(`  Found iframe source: ${iframeSrc}`);
                
                // If it's a direct m3u8, return immediately
                if (iframeSrc.includes('.m3u8')) {
                    return {
                        streams: [{
                            url: iframeSrc.startsWith('//') ? `https:${iframeSrc}` : iframeSrc,
                            headers: { Referer: playerUrl },
                            quality: 'Auto'
                        }]
                    };
                }
                
                // If we found a deeper link, use that for Playwright instead of the wrapper
                // This saves one navigation step
                if (iframeSrc.startsWith('http')) {
                    // Use the original playerUrl (webplayer.php) as the referer for the iframe
                    const originalReferer = playerUrl;
                    playerUrl = iframeSrc;
                    
                    // Call resolver with NEW target but OLD referer
                    const result = await resolveWithPlaywright(playerUrl, originalReferer);
                    
                    if (result.type === 'stream_list') {
                        return { streams: result.streams };
                    } else if (result.type === 'stream') {
                        return { streams: [{ url: result.url, headers: result.headers, quality: 'Auto' }] };
                    }
                    // If Playwright failed on the direct link, fall through to retry original (unlikely to help but safe)
                }
            }
        } catch (err) {
            console.warn(`  Direct extraction failed, falling back to full surf: ${err.message}`);
        }

        const result = await resolveWithPlaywright(playerUrl, playerUrl);

        if (result.type === 'stream_list') {
            return {
                streams: result.streams
            };
        } else if (result.type === 'stream') {
             // Fallback for older resolver version compatibility
             return {
                 streams: [{
                     url: result.url,
                     headers: result.headers,
                     quality: 'Auto'
                 }]
             };
        }

        return { error: 'Could not resolve direct stream link' };
    } catch (error) {
        return { error: error.message };
    }
}

/**
 * Search for a match by name (basic implementation)
 * Searches top sports (Football, etc.) for the match.
 */
async function search(query) {
    // Default to searching Football (1), Ice Hockey (2), Basketball (3), Tennis (4)
    const sportIds = [1, 2, 3, 4];
    const results = [];
    const q = query.toLowerCase().replace(/[^a-z0-9]/g, '');

    for (const id of sportIds) {
        const matches = await getMatches(id);
        const found = matches.filter(m => 
            m.title.toLowerCase().replace(/[^a-z0-9]/g, '').includes(q)
        );
        results.push(...found);
        // If we found good matches, maybe stop?
        if (results.length >= 5) break;
    }
    return results;
}


module.exports = {
    getSports,
    getMatches,
    getStreamVariants,
    resolveStream,
    search
};