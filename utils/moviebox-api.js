const axios = require('axios');
const { config } = require('./config');
const { HttpsProxyAgent } = require('https-proxy-agent');

const BASE_URL = 'https://h5-api.aoneroom.com/wefeed-h5api-bff';
const PLAY_URL = 'https://movibox.net/wefeed-h5api-bff';

// working token from call_subject_play.sh, switched to appType 2 (Android) to bypass web restrictions
const DEFAULT_X_USER = '{"token":"eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJ1aWQiOjc1NjEyMzU2Nzc1NjA4NTMyODAsImF0cCI6MywiZXh0IjoiMTc3MTE0NDQ4MiIsImV4cCI6MTc3ODkyMDQ4MiwiaWF0IjoxNzcxMTQxNDk1fQ.BF7r1K_npeKWjkjuIxqZMTvUDxJ8oBjvSBc3glWcqtU","userId":"7561235677560853280","userType":0,"appType":2}';
const DEFAULT_COOKIE = 'uuid=202d061a-fbe4-443c-aa09-be0232791ef7';
const DEFAULT_UA = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36';

const USER_AGENTS = [
    'Mozilla/5.0 (Linux; Android 13; SM-S911B) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/116.0.0.0 Mobile Safari/537.36',
    'Mozilla/5.0 (Linux; Android 12; Pixel 6) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/115.0.0.0 Mobile Safari/537.36',
    'Mozilla/5.0 (iPhone; CPU iPhone OS 16_6 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/16.6 Mobile/15E148 Safari/604.1',
    'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/144.0.0.0 Safari/537.36'
];

const axiosInstance = axios.create({
    timeout: 45000,
});

axiosInstance.interceptors.request.use((reqConfig) => {
    const host = config.movieboxHost || 'moviebox.ph';
    const cookie = config.movieboxCookie || DEFAULT_COOKIE;

    // Use Proxy if configured to bypass region blocks
    if (config.movieboxProxyUrl) {
        try {
            console.log(`[MovieBox API] Using proxy: ${config.movieboxProxyUrl}`);
            reqConfig.httpsAgent = new HttpsProxyAgent(config.movieboxProxyUrl);
            reqConfig.proxy = false; // Disable default axios proxy handling to use HttpsProxyAgent
        } catch (e) {
            console.error('[MovieBox API] Proxy setup error:', e.message);
        }
    }

    const randomUA = USER_AGENTS[Math.floor(Math.random() * USER_AGENTS.length)];
    let token = null;
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
    setHeader('User-Agent', config.movieboxUserAgent || randomUA);
    setHeader('Origin', `https://${host}`);
    
    // Only set Referer if not already set by specific calls
    if (!reqConfig.headers['Referer'] && !reqConfig.headers['referer']) {
        setHeader('Referer', `https://${host}/`);
    }
    
    setHeader('x-user', DEFAULT_X_USER);
    setHeader('x-client-info', '{"timezone":"Europe/Berlin"}');
    
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
        'Popular Series',
        'Popular Movie',
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

    return matched.length > 1 ? matched : sections;
}

async function getTrending(page = 1) {
    try {
        const url = `${BASE_URL}/subject/trending?page=${page}&perPage=18`;
        const response = await axiosInstance.get(url);
        if (response.data && response.data.code === 0) {
            const items = response.data.data?.items || response.data.data?.subjectList || [];
            return items.map(mapItem);
        }
        console.warn(`[MovieBox API] Trending returned code ${response.data?.code}: ${response.data?.message}`);
        return [];
    } catch (e) {
        console.error('[MovieBox API] Trending error:', e.message);
        return [];
    }
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
    const isMovie = (season === 0 && episode === 0);
    const typeLabel = isMovie ? 'movie' : 'series';
    
    // List of hosts to try in case of region block
    const hosts = [
        'moviebox.ph',
        'movibox.net',
        '123movienow.cc',
        'moviebox.to'
    ];

    // List of timezones to try in case of region block
    const timezones = [
        'Pacific/Pago_Pago',
        'Asia/Hong_Kong',
        'Europe/London',
        'America/New_York'
    ];

    for (let h = 0; h < hosts.length; h++) {
        const host = hosts[h];
        for (let t = 0; t < (h === 0 ? 1 : 2); t++) { // Only try alternative timezone on fallback hosts or second attempt
            const timezone = timezones[t];
            
            try {
                const PLAY_URL_DYNAMIC = `https://${host}/wefeed-h5api-bff`;
                
                let refererPath = detailPath.startsWith('/') ? detailPath.substring(1) : detailPath;
                refererPath = refererPath.replace('movies/detail/', 'movies/').replace('series/detail/', 'series/');

                const referer = `https://${host}/spa/videoPlayPage/${refererPath}?id=${subjectId}&type=/${typeLabel}/detail&detailSe=&detailEp=&lang=en`;

                const url = `${PLAY_URL_DYNAMIC}/subject/play?subjectId=${subjectId}&se=${season}&ep=${episode}&detailPath=${encodeURIComponent(detailPath)}`;
                
                if (h > 0 || t > 0) {
                    console.log(`[MovieBox API] Retrying with host=${host} tz=${timezone}`);
                }

                const response = await axiosInstance.get(url, {
                    headers: {
                        'Referer': referer,
                        'x-client-info': JSON.stringify({ timezone })
                    }
                });
                
                if (response.data && response.data.code === 0) {
                    if (h > 0 || t > 0) {
                        console.log(`[MovieBox API] Play Success on fallback: id=${subjectId} host=${host} tz=${timezone}`);
                    } else {
                        console.log(`[MovieBox API] Play Success: id=${subjectId} streams=${response.data.data?.streams?.length || 0} hls=${response.data.data?.hls?.length || 0}`);
                    }
                    return response.data.data;
                }
                
                if (response.data && response.data.message === 'invalid region') {
                    console.warn(`[MovieBox API] Region Blocked on ${host}: id=${subjectId}`);
                    continue; // Try next host/tz
                }

                console.warn(`[MovieBox API] Play Failed on ${host}: code=${response.data?.code} msg=${response.data?.message}`);
                // If it's a different error than region block, we might still want to try next host, but maybe not
                continue; 
            } catch (e) {
                if (e.response?.data?.message === 'invalid region') {
                     console.warn(`[MovieBox API] Region Blocked on ${host}: id=${subjectId}`);
                     continue;
                }
                console.error(`[MovieBox API] Play error on ${host}:`, e.response?.data || e.message);
                continue;
            }
        }
    }
    
    return null;
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
        const response = await axiosInstance.post(url, payload);
        if (response.data && response.data.code === 0) {
            const items = response.data.data.items || response.data.data.subjectList || [];
            return {
                page: response.data.data.pager.page,
                hasNext: response.data.data.pager.hasMore,
                items: items.map(mapItem)
            };
        }
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
            const items = response.data.data.items || response.data.data.subjectList || [];
            return {
                page: response.data.data.pager.page,
                hasNext: response.data.data.pager.hasMore,
                total: response.data.data.pager.totalCount,
                items: items.map(mapItem)
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
