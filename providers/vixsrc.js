const axios = require("axios");
const http = require('http');
const https = require('https');

// Force IPv4 for VPS stability
const axiosInstance = axios.create({
    httpAgent: new http.Agent({ family: 4 }),
    httpsAgent: new https.Agent({ family: 4 }),
    timeout: 30000
});

const TMDB_API_KEY = "68e094699525b18a70bab2f86b1fa706";
const BASE_URL = 'https://vixsrc.to';

async function makeRequest(url, options = {}) {
    const defaultHeaders = {
        'User-Agent': 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36',
        'Accept': 'application/json,*/*',
        'Accept-Language': 'en-US,en;q=0.5',
        'Connection': 'keep-alive',
        ...options.headers
    };

    const response = await axiosInstance({
        url,
        method: options.method || 'GET',
        headers: defaultHeaders,
        ...options
    });
    return response;
}

async function getTmdbInfo(tmdbId, mediaType) {
    const url = `https://api.themoviedb.org/3/${mediaType === 'tv' ? 'tv' : 'movie'}/${tmdbId}?api_key=${TMDB_API_KEY}`;
    const response = await makeRequest(url);
    return { data: response.data };
}

async function extractStreamFromPage(contentType, contentId, seasonNum, episodeNum) {
    let vixsrcUrl;
    let subtitleApiUrl;

    if (contentType === 'movie') {
        vixsrcUrl = `${BASE_URL}/movie/${contentId}`;
        subtitleApiUrl = `https://sub.wyzie.ru/search?id=${contentId}`;
    } else {
        vixsrcUrl = `${BASE_URL}/tv/${contentId}/${seasonNum}/${episodeNum}`;
        subtitleApiUrl = `https://sub.wyzie.ru/search?id=${contentId}&season=${seasonNum}&episode=${episodeNum}`;
    }

    console.log(`[Vixsrc] Fetching: ${vixsrcUrl}`);

    const response = await makeRequest(vixsrcUrl, {
        headers: { 'Accept': 'text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8' }
    });
    const html = response.data;
    console.log(`[Vixsrc] HTML length: ${html.length} characters`);

    let masterPlaylistUrl = null;

    if (html.includes('window.masterPlaylist')) {
        const urlMatch = html.match(/url:\s*['"]([^'"]+)['"]/);
        const tokenMatch = html.match(/['"]?token['"]?\s*:\s*['"]([^'"]+)['"]/);
        const expiresMatch = html.match(/['"]?expires['"]?\s*:\s*['"]([^'"]+)['"]/);

        if (urlMatch && tokenMatch && expiresMatch) {
            const baseUrl = urlMatch[1];
            const token = tokenMatch[1];
            const expires = expiresMatch[1];
            masterPlaylistUrl = baseUrl.includes('?') ? `${baseUrl}&token=${token}&expires=${expires}&h=1&lang=en` : `${baseUrl}?token=${token}&expires=${expires}&h=1&lang=en`;
        }
    }

    if (!masterPlaylistUrl) {
        const m3u8Match = html.match(/(https?:\/\/[^'"\s]+\.m3u8[^'"\s]*)/);
        if (m3u8Match) masterPlaylistUrl = m3u8Match[1];
    }

    return { masterPlaylistUrl, subtitleApiUrl, vixsrcUrl };
}

async function getSubtitles(subtitleApiUrl) {
    try {
        const response = await makeRequest(subtitleApiUrl);
        const subtitleData = response.data;
        const subtitleTrack = subtitleData.find(track =>
            track.display.includes('English') && (track.encoding === 'ASCII' || track.encoding === 'UTF-8')
        ) || subtitleData.find(track => track.display.includes('English'));

        const subtitles = subtitleTrack ? subtitleTrack.url : '';
        console.log(subtitles ? `[Vixsrc] Found subtitles: ${subtitles}` : '[Vixsrc] No English subtitles found');
        return subtitles;
    } catch (error) {
        console.log('[Vixsrc] Subtitle fetch failed:', error.message);
        return '';
    }
}

async function getVixsrcStreams(tmdbId, mediaType = 'movie', seasonNum = null, episodeNum = null) {
    console.log(`[Vixsrc] Fetching streams for TMDB ID: ${tmdbId}, Type: ${mediaType}`);

    try {
        await getTmdbInfo(tmdbId, mediaType);
        const { masterPlaylistUrl, subtitleApiUrl, vixsrcUrl } = await extractStreamFromPage(mediaType, tmdbId, seasonNum, episodeNum);

        if (!masterPlaylistUrl) return [];

        const subtitles = await getSubtitles(subtitleApiUrl);
        return [{
            name: "Vixsrc",
            title: "Auto Quality Stream",
            url: masterPlaylistUrl,
            quality: '1080p',
            provider: 'Vidsrc',
            subtitles: subtitles ? [{ url: subtitles, lang: 'en', label: 'English' }] : undefined,
            headers: {
                'Referer': vixsrcUrl,
                'User-Agent': 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36'
            }
        }];
    } catch (error) {
        console.error(`[Vixsrc] Error: ${error.message}`);
        return [];
    }
}

module.exports = { getVixsrcStreams };