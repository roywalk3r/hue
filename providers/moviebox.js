const movieboxApi = require('../utils/moviebox-api');
const { getDetails } = require('../utils/tmdb');

async function getStreams(tmdbId, type, season = 1, episode = 1, tmdbDataProvided = null) {
    try {
        console.log(`[MovieBox Provider] Fetching for tmdbId=${tmdbId} type=${type} se=${season} ep=${episode}`);
        // 1. Get TMDB data to find title and year
        let tmdbData = tmdbDataProvided;
        if (!tmdbData) {
            tmdbData = await getDetails(type, tmdbId);
        }
        
        if (!tmdbData) {
            console.log(`[MovieBox Provider] No TMDB data found for ${tmdbId}`);
            return [];
        }

        const title = tmdbData.title || tmdbData.name;
        const year = (tmdbData.release_date || tmdbData.first_air_date || '').split('-')[0];
        
        if (!title) {
            console.log(`[MovieBox Provider] No title found in TMDB data for ${tmdbId}`);
            return [];
        }

        console.log(`[MovieBox Provider] Searching MovieBox for: "${title}" (${year})`);

        // 2. Search MovieBox
        const searchResults = await movieboxApi.search(title, 1, type === 'movie' ? 1 : 2);
        if (!searchResults || !searchResults.items || searchResults.items.length === 0) {
            console.log(`[MovieBox Provider] No MovieBox search results for: "${title}"`);
            return [];
        }

        console.log(`[MovieBox Provider] Found ${searchResults.items.length} search results`);

        // 3. Find the best match (compare title, year and subjectType)
        const targetType = type === 'movie' ? 1 : 2;
        const bestMatch = searchResults.items.find(item => {
            const itemTitle = (item.title || '').toLowerCase().replace(/[^a-z0-9]/g, '');
            const searchTitle = title.toLowerCase().replace(/[^a-z0-9]/g, '');
            const itemYear = (item.releaseDate || '').split('-')[0];
            
            const titleMatch = itemTitle === searchTitle;
            const yearMatch = !year || itemYear === year;
            const typeMatch = item.subjectType === targetType;
            
            if (titleMatch && yearMatch && typeMatch) return true;
            return false;
        }) || searchResults.items.find(item => {
            // Fallback: title and type match only
            const itemTitle = (item.title || '').toLowerCase().replace(/[^a-z0-9]/g, '');
            const searchTitle = title.toLowerCase().replace(/[^a-z0-9]/g, '');
            return itemTitle === searchTitle && item.subjectType === targetType;
        }) || searchResults.items[0]; 

        if (!bestMatch) {
            console.log(`[MovieBox Provider] No best match found for: "${title}"`);
            return [];
        }

        console.log(`[MovieBox Provider] Selected match: "${bestMatch.title}" (${bestMatch.id}) path: ${bestMatch.detailPath}`);

        // 4. Fetch streams for the match
        // For series, use the requested season/episode. For movies, use 0/0.
        const se = type === 'movie' ? 0 : (parseInt(season) || 1);
        const ep = type === 'movie' ? 0 : (parseInt(episode) || 1);

        const data = await movieboxApi.getStreams(bestMatch.id, bestMatch.detailPath, se, ep);
        if (!data) return [];

        const streams = [];
        
        // Extract subtitles
        let subtitles = [];
        const firstStream = data.streams && data.streams.length > 0 ? data.streams[0] : (data.hls && data.hls.length > 0 ? data.hls[0] : null);
        
        if (firstStream) {
            try {
                const captions = await movieboxApi.getCaptions(bestMatch.id, bestMatch.detailPath, firstStream.id || '');
                subtitles = (captions || []).map(c => ({
                    url: c.url,
                    lang: c.lan || 'en',
                    label: c.lanName || 'English'
                }));
            } catch (capErr) {
                console.warn(`[MovieBox Provider] Caption fetch failed:`, capErr.message);
            }
        }

        // Add headers for proxying
        const isMovie = (se === 0 && ep === 0);
        const typeFolder = isMovie ? 'movies' : 'series';
        const typeLabel = isMovie ? 'movie' : 'series';
        const cleanPath = bestMatch.detailPath.startsWith('/') ? bestMatch.detailPath.substring(1) : bestMatch.detailPath;
        const streamReferer = `https://123movienow.cc/spa/videoPlayPage/${typeFolder}/${cleanPath}?id=${bestMatch.id}&type=/${typeLabel}/detail&detailSe=&detailEp=&lang=en`;
        const mbHeaders = movieboxApi.getStreamHeaders(streamReferer);

        if (data.streams && Array.isArray(data.streams)) {
            data.streams.forEach(s => {
                streams.push({
                    title: `MovieBox - ${s.resolutions}p (MP4)`,
                    url: s.url,
                    quality: `${s.resolutions}p`,
                    format: 'mp4',
                    subtitles: subtitles.length > 0 ? subtitles : undefined,
                    headers: mbHeaders
                });
            });
        }

        if (data.hls && Array.isArray(data.hls)) {
            data.hls.forEach(s => {
                streams.push({
                    title: `MovieBox - Auto (HLS)`,
                    url: s.url,
                    quality: 'Auto',
                    format: 'hls',
                    subtitles: subtitles.length > 0 ? subtitles : undefined,
                    headers: mbHeaders
                });
            });
        }

        return streams;
    } catch (e) {
        console.error('[MovieBox Provider] Error:', e.message);
        return [];
    }
}

module.exports = { getStreams };
