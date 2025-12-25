const axios = require('axios');
const { createAxiosConfig, requestWithRetry, randomDelay } = require('../utils/proxyHelper');

/**
 * Vixsrc streaming provider integration (standalone)
 * React Native compatible variant - no external addon framework
 */

// Constants
const TMDB_API_KEY = "68e094699525b18a70bab2f86b1fa706";
const BASE_URL = 'https://vixsrc.to';
let vixsrcUrl;
let subtitleApiUrl;
// Helper function to make HTTP requests with proxy and browser headers
async function makeRequest(url, options = {}) {
    await randomDelay(1000, 3000);
    
    const config = createAxiosConfig({
        url,
        method: options.method || 'GET',
        data: options.body,
        headers: options.headers,
        timeout: 15000
    });
    
    try {
        const response = await requestWithRetry(async () => axios(config));
        return response.data;
    } catch (error) {
        throw error;
    }
}

// Helper function to get TMDB info
async function getTmdbInfo(tmdbId, mediaType) {
    const url = `https://api.themoviedb.org/3/${mediaType === 'tv' ? 'tv' : 'movie'}/${tmdbId}?api_key=${TMDB_API_KEY}`;
    
    try {
        const data = await makeRequest(url);
        return { data };
    } catch (error) {
        throw error;
    }
}

// Helper function to parse M3U8 playlist
// Removed obsolete parseM3U8Playlist implementation

// Removed unused helpers getQualityFromResolution and resolveUrl

// Helper function to extract stream URL from Vixsrc page
async function extractStreamFromPage(url, contentType, contentId, seasonNum, episodeNum) {
    if (contentType === 'movie') {
        vixsrcUrl = `${BASE_URL}/movie/${contentId}`;
        subtitleApiUrl = `https://sub.wyzie.ru/search?id=${contentId}`;
    } else {
        vixsrcUrl = `${BASE_URL}/tv/${contentId}/${seasonNum}/${episodeNum}`;
        subtitleApiUrl = `https://sub.wyzie.ru/search?id=${contentId}&season=${seasonNum}&episode=${episodeNum}`;
    }

    console.log(`[Vixsrc] Fetching: ${vixsrcUrl}`);

    try {
        const html = await makeRequest(vixsrcUrl, {
            headers: {
                'Accept': 'text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8'
            }
        });
        
        console.log(`[Vixsrc] HTML length: ${html.length} characters`);
        let masterPlaylistUrl = null;

        // Method 1: Look for window.masterPlaylist (primary method)
        if (html.includes('window.masterPlaylist')) {
            console.log('[Vixsrc] Found window.masterPlaylist');

            const urlMatch = html.match(/url:\s*['"]([^'"]+)['"]/);
            const tokenMatch = html.match(/['"]?token['"]?\s*:\s*['"]([^'"]+)['"]/);
            const expiresMatch = html.match(/['"]?expires['"]?\s*:\s*['"]([^'"]+)['"]/);

            if (urlMatch && tokenMatch && expiresMatch) {
                const baseUrl = urlMatch[1];
                const token = tokenMatch[1];
                const expires = expiresMatch[1];

                console.log('[Vixsrc] Extracted tokens:');
                console.log(`  - Base URL: ${baseUrl}`);
                console.log(`  - Token: ${token.substring(0, 20)}...`);
                console.log(`  - Expires: ${expires}`);

                // Construct the master playlist URL
                if (baseUrl.includes('?b=1')) {
                    masterPlaylistUrl = `${baseUrl}&token=${token}&expires=${expires}&h=1&lang=en`;
                } else {
                    masterPlaylistUrl = `${baseUrl}?token=${token}&expires=${expires}&h=1&lang=en`;
                }
                
                console.log(`[Vixsrc] Constructed master playlist URL: ${masterPlaylistUrl}`);
            }
        }

        // Method 2: Look for direct .m3u8 URLs
        if (!masterPlaylistUrl) {
            const m3u8Match = html.match(/(https?:\/\/[^'"\s]+\.m3u8[^'"\s]*)/);
            if (m3u8Match) {
                masterPlaylistUrl = m3u8Match[1];
                console.log('[Vixsrc] Found direct .m3u8 URL:', masterPlaylistUrl);
            }
        }

        // Method 3: Look for stream URLs in script tags
        if (!masterPlaylistUrl) {
            const scriptMatches = html.match(/<script[^>]*>(.*?)<\/script>/gs);
            if (scriptMatches) {
                for (const script of scriptMatches) {
                    const streamMatch = script.match(/['"]?(https?:\/\/[^'"\s]+(?:\.m3u8|playlist)[^'"\s]*)/);
                    if (streamMatch) {
                        masterPlaylistUrl = streamMatch[1];
                        console.log('[Vixsrc] Found stream in script:', masterPlaylistUrl);
                        break;
                }
                }
            }
        }

        if (!masterPlaylistUrl) {
            console.log('[Vixsrc] No master playlist URL found');
            return null;
        }

        return { masterPlaylistUrl, subtitleApiUrl };
    } catch (error) {
        console.error('[Vixsrc] Error extracting stream:', error);
        throw error;
    }
}

// Helper function to get subtitles
async function getSubtitles(subtitleApiUrl) {
    try {
        const subtitleData = await makeRequest(subtitleApiUrl);
        
        // Find English subtitle track (same logic as original)
        let subtitleTrack = subtitleData.find(track =>
            track.display.includes('English') && (track.encoding === 'ASCII' || track.encoding === 'UTF-8')
        );

        if (!subtitleTrack) {
            subtitleTrack = subtitleData.find(track => track.display.includes('English') && track.encoding === 'CP1252');
        }

        if (!subtitleTrack) {
            subtitleTrack = subtitleData.find(track => track.display.includes('English') && track.encoding === 'CP1250');
        }

        if (!subtitleTrack) {
            subtitleTrack = subtitleData.find(track => track.display.includes('English') && track.encoding === 'CP850');
        }

        const subtitles = subtitleTrack ? subtitleTrack.url : '';
        console.log(subtitles ? `[Vixsrc] Found subtitles: ${subtitles}` : '[Vixsrc] No English subtitles found');
        return subtitles;
    } catch (error) {
        console.error('[Vixsrc] Error fetching subtitles:', error);
        return '';
    }
}

// Main function to get streams
async function getVixsrcStreams(tmdbId, mediaType = 'movie', seasonNum = null, episodeNum = null) {
    console.log(`[Vixsrc] Fetching streams for TMDB ID: ${tmdbId}, Type: ${mediaType}`);

    try {
        const { data } = await getTmdbInfo(tmdbId, mediaType);
        
        const streamData = await extractStreamFromPage(null, mediaType, tmdbId, seasonNum, episodeNum);
        
        if (!streamData) {
            console.log('[Vixsrc] No stream data found');
            return [];
        }

        const { masterPlaylistUrl, subtitleApiUrl } = streamData;

        // Return single master playlist with Auto quality
        console.log('[Vixsrc] Returning master playlist with Auto quality...');

        // Get subtitles
        const subtitles = await getSubtitles(subtitleApiUrl);
        
        // Return single stream with master playlist
        const tmdbStreams = [{
            name: "Vixsrc",
            title: "Auto Quality Stream",
            url: masterPlaylistUrl,
            quality: '1080p',
            provider: 'Vidsrc',
            headers: {
                'Referer': vixsrcUrl,
                'User-Agent': 'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/142.0.0.0 Safari/537.36'
            }
        }];

        console.log('[Vixsrc] Successfully processed 1 stream with Auto quality');
        return tmdbStreams;
    } catch (error) {
        console.error(`[Vixsrc] Error in getVixsrcStreams: ${error.message}`);
        return [];
    }
}

module.exports = { getVixsrcStreams };
