const axios = require('axios');
const { config } = require('./config');

const BASE_URL = 'https://h5-api.aoneroom.com/wefeed-h5api-bff';
const PLAY_URL = 'https://123movienow.cc/wefeed-h5api-bff';

// working token from call_subject_play.sh, switched to appType 2 (Android) to bypass web restrictions
const DEFAULT_X_USER = '{"token":"eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJ1aWQiOjc1NjEyMzU2Nzc1NjA4NTMyODAsImF0cCI6MywiZXh0IjoiMTc3MTE0NDQ4MiIsImV4cCI6MTc3ODkyMDQ4MiwiaWF0IjoxNzcxMTQxNDk1fQ.BF7r1K_npeKWjkjuIxqZMTvUDxJ8oBjvSBc3glWcqtU","userId":"7561235677560853280","userType":0,"appType":2}';
const DEFAULT_COOKIE = 'uuid=202d061a-fbe4-443c-aa09-be0232791ef7';
const DEFAULT_UA = 'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/144.0.0.0 Safari/537.36';

const axiosInstance = axios.create({
    timeout: 15000,
});

axiosInstance.interceptors.request.use((reqConfig) => {
    const host = config.movieboxHost || 'moviebox.ph';
    const cookie = config.movieboxCookie || DEFAULT_COOKIE;
    
    let token = '';
    try {
        const userData = JSON.parse(DEFAULT_X_USER);
        token = userData.token;
    } catch {}

    // Use reqConfig.headers.set or direct assignment depending on axios version
    const setHeader = (name, value) => {
        if (!value) return;
        if (reqConfig.headers.set) {
            reqConfig.headers.set(name, value);
        } else {
            reqConfig.headers[name] = value;
        }
    };

    setHeader('Accept', 'application/json');
    setHeader('User-Agent', config.movieboxUserAgent || DEFAULT_UA);
    setHeader('Origin', `https://${host}`);
    
    // Only set Referer if not already set by specific calls
    if (!reqConfig.headers['Referer'] && !reqConfig.headers['referer']) {
        setHeader('Referer', `https://${host}/`);
    }
    
    setHeader('x-user', DEFAULT_X_USER);
    setHeader('x-client-info', '{"timezone":"Pacific/Pago_Pago"}');
    
    if (token) {
        setHeader('Authorization', `Bearer ${token}`);
    }

    if (cookie) {
        if (!reqConfig.headers['Cookie'] && !reqConfig.headers['cookie']) {
            setHeader('Cookie', cookie);
        }
    }

    return reqConfig;
});

function mapItem(item) {
    let type = 'tv';
    if (item.subjectType === 1) type = 'movie';
    else if (item.subjectType === 2 || item.subjectType === 7) type = 'tv';

    return {
        id: item.subjectId,
        title: item.title,
        description: item.description,
        cover: item.cover ? item.cover.url : null,
        rating: item.imdbRatingValue,
        releaseDate: item.releaseDate,
        detailPath: item.detailPath,
        genre: item.genre,
        type: type,
        subjectType: item.subjectType
    };
}

async function getHome() {
    try {
        const host = config.movieboxHost || 'moviebox.ph';
        const url = `${BASE_URL}/home?host=${host}`;
        const response = await axiosInstance.get(url);
        if (response.data && response.data.code === 0) {
            const data = response.data.data;
            const sections = (data.operatingList || [])
                .map(section => {
                    const items = (section.subjects || section.subjectList || [])
                        .map(mapItem);
                    
                    return {
                        title: section.title,
                        items: items
                    };
                })
                .filter(s => s.items.length > 0);

            return {
                platforms: data.platformList || [],
                sections: sections
            };
        }
        return { sections: [] };
    } catch (e) {
        console.error('[MovieBox API] Home error:', e);
        return { sections: [] };
    }
}

async function getDiscover() {
    const home = await getHome();
    const sections = home.sections || [];
    
    const targetTitles = [
        '🔥Hot Short TV', 
        'SA Drama', 
        'Thai-Drama', 
        '💓Teen Romance 💓', 
        'Turkish Drama',
        'C-Drama',
        'K-Drama',
        'Western TV',
        'Movie/TV Series',
        'Must-watch Black Shows'
    ];

    const matched = sections.filter(s => 
        targetTitles.some(t => s.title.toLowerCase().includes(t.toLowerCase()))
    );

    return matched.length > 0 ? matched : sections;
}

async function getTrending() {
    return await getSection('Trending Now');
}

async function getSection(titleQuery) {
    const home = await getHome();
    const sections = home.sections || [];
    const section = sections.find(s => s.title.toLowerCase().includes(titleQuery.toLowerCase()));
    return section ? section.items : [];
}

async function getDetail(detailPath) {
    try {
        const url = `${BASE_URL}/detail?detailPath=${encodeURIComponent(detailPath)}`;
        const response = await axiosInstance.get(url);
        if (response.data && response.data.code === 0) {
            return response.data.data;
        }
        return null;
    } catch (e) {
        console.error('[MovieBox API] Detail error:', e);
        return null;
    }
}

async function getStreams(subjectId, detailPath, season = 1, episode = 1) {
    try {
        const isMovie = (season === 0 && episode === 0);
        const typeFolder = isMovie ? 'movies' : 'series';
        const typeLabel = isMovie ? 'movie' : 'series';
        
        // Clean path for Referer: remove /detail/ if it exists, as the SPA URL doesn't use it
        // e.g. series/detail/slug -> series/slug
        let refererPath = detailPath.startsWith('/') ? detailPath.substring(1) : detailPath;
        refererPath = refererPath.replace('movies/detail/', 'movies/').replace('series/detail/', 'series/');

        const referer = `https://123movienow.cc/spa/videoPlayPage/${refererPath}?id=${subjectId}&type=/${typeLabel}/detail&detailSe=&detailEp=&lang=en`;

        const url = `${PLAY_URL}/subject/play?subjectId=${subjectId}&se=${season}&ep=${episode}&detailPath=${encodeURIComponent(detailPath)}`;
        console.log(`[MovieBox API] Play Request: ${url}`);
        console.log(`[MovieBox API] Play Referer: ${referer}`);
        
        const response = await axiosInstance.get(url, {
            headers: {
                'Referer': referer
            }
        });
        
        if (response.data && response.data.code === 0) {
            console.log(`[MovieBox API] Play Success: id=${subjectId} streams=${response.data.data?.streams?.length || 0} hls=${response.data.data?.hls?.length || 0}`);
            return response.data.data;
        }
        
        console.warn(`[MovieBox API] Play Failed: code=${response.data?.code} msg=${response.data?.message}`);
        return null;
    } catch (e) {
        console.error('[MovieBox API] Play error:', e.response?.data || e.message);
        return null;
    }
}

async function getCaptions(subjectId, detailPath, streamId) {
    try {
        const url = `${BASE_URL}/subject/caption?format=MP4&id=${streamId}&subjectId=${subjectId}&detailPath=${encodeURIComponent(detailPath)}`;
        const response = await axiosInstance.get(url);
        if (response.data && response.data.code === 0) {
            return response.data.data?.captions || [];
        }
        return [];
    } catch (e) {
        console.error('[MovieBox API] Caption error:', e);
        return [];
    }
}

async function filter(params) {
    try {
        const url = `${BASE_URL}/subject/filter`;
        const payload = {
            page: params.page || 1,
            perPage: params.perPage || 28,
            channelId: params.channelId || 2, 
            genre: params.genre || "All",
            country: params.country || "All",
            year: params.year || "All",
            sort: params.sort || "ForYou",
            tabId: params.tabId || 0,
            classify: params.classify || "All"
        };
        console.log(`[MovieBox API] Filter Request: ${JSON.stringify(payload)}`);
        const response = await axiosInstance.post(url, payload);
        if (response.data && response.data.code === 0) {
            console.log(`[MovieBox API] Filter Success: Found ${response.data.data.items?.length || 0} items`);
            return {
                page: response.data.data.pager.page,
                hasNext: response.data.data.pager.hasMore,
                items: (response.data.data.items || []).map(mapItem)
            };
        }
        console.warn(`[MovieBox API] Filter returned code ${response.data?.code}: ${response.data?.message}`);
        return { items: [] };
    } catch (e) {
        console.error('[MovieBox API] Filter error:', e);
        return { items: [] };
    }
}

async function getIndianContent(type = 'movie', page = 1) {
    return await filter({
        country: 'India',
        channelId: type === 'movie' ? 1 : 2,
        page: page,
        perPage: 28
    });
}

async function getShorts(page = 1) {
    // Channel 7 is explicitly for Short TV / Mini-series
    return await filter({
        channelId: 7,
        page: page,
        perPage: 28
    });
}

async function search(keyword, page = 1, type = 0) {
    try {
        const url = `${BASE_URL}/subject/search`;
        const payload = {
            keyword: keyword,
            page: page,
            perPage: 28,
            subjectType: type // 0: All, 1: Movie, 2: Series
        };
        const response = await axiosInstance.post(url, payload);
        if (response.data && response.data.code === 0) {
            return {
                page: response.data.data.pager.page,
                hasNext: response.data.data.pager.hasMore,
                total: response.data.data.pager.totalCount,
                items: (response.data.data.items || []).map(mapItem)
            };
        }
        return { items: [] };
    } catch (e) {
        console.error('[MovieBox API] Search error:', e);
        return { items: [] };
    }
}

function getStreamHeaders(referer = 'https://moviebox.ph/') {
    const host = config.movieboxHost || 'moviebox.ph';
    const cookie = config.movieboxCookie || DEFAULT_COOKIE;
    
    const headers = {
        'Accept': 'application/json',
        'User-Agent': config.movieboxUserAgent || DEFAULT_UA,
        'Origin': `https://${host}`,
        'Referer': referer,
        'x-user': DEFAULT_X_USER,
        'x-client-info': '{"timezone":"Pacific/Pago_Pago"}'
    };

    if (cookie) {
        headers['Cookie'] = cookie;
    }

    return headers;
}

module.exports = {
    getHome,
    getDiscover,
    getTrending,
    getSection,
    getDetail,
    getStreams,
    getCaptions,
    filter,
    getIndianContent,
    getShorts,
    getStreamHeaders,
    search
};
