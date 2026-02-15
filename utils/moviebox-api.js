const axios = require('axios');
const { config } = require('./config');

const BASE_URL = 'https://h5-api.aoneroom.com/wefeed-h5api-bff';
const PLAY_URL = 'https://123movienow.cc/wefeed-h5api-bff';

// working token from call_subject_play.sh
const DEFAULT_X_USER = '{"token":"eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJ1aWQiOjc1NjEyMzU2Nzc1NjA4NTMyODAsImF0cCI6MywiZXh0IjoiMTc3MTE0NDQ4MiIsImV4cCI6MTc3ODkyMDQ4MiwiaWF0IjoxNzcxMTQxNDk1fQ.BF7r1K_npeKWjkjuIxqZMTvUDxJ8oBjvSBc3glWcqtU","userId":"7561235677560853280","userType":0,"appType":3}';
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

    // Merge headers, prioritizing what's already in reqConfig
    reqConfig.headers = {
        'Accept': 'application/json',
        'User-Agent': config.movieboxUserAgent || DEFAULT_UA,
        'Origin': `https://${host}`,
        'Referer': `https://${host}/`,
        'x-user': DEFAULT_X_USER,
        'x-client-info': '{"timezone":"Pacific/Pago_Pago"}',
        'Authorization': `Bearer ${token}`,
        ...reqConfig.headers
    };

    if (cookie) {
        // If Cookie header already exists (passed in reqConfig), append or prioritize?
        // Axios typically overwrites if same key in object merge above.
        if (!reqConfig.headers['Cookie'] && !reqConfig.headers['cookie']) {
            reqConfig.headers['Cookie'] = cookie;
        }
    }

    return reqConfig;
});

function mapItem(item) {
    return {
        id: item.subjectId,
        title: item.title,
        description: item.description,
        cover: item.cover ? item.cover.url : null,
        rating: item.imdbRatingValue,
        releaseDate: item.releaseDate,
        detailPath: item.detailPath,
        genre: item.genre,
        type: item.subjectType === 1 ? 'movie' : 'tv'
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
        console.error('[MovieBox API] Home error:', e.message);
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
        'K-Drama'
    ];

    return sections.filter(s => 
        targetTitles.some(t => s.title.toLowerCase().includes(t.toLowerCase()))
    );
}

async function getDetail(detailPath) {
    try {
        const url = `${BASE_URL}/detail?detailPath=${detailPath}`;
        const response = await axiosInstance.get(url);
        if (response.data && response.data.code === 0) {
            return response.data.data;
        }
        return null;
    } catch (e) {
        console.error('[MovieBox API] Detail error:', e.message);
        return null;
    }
}

async function getStreams(subjectId, detailPath, season = 1, episode = 1) {
    try {
        // Construct SPA referer like in the script
        const typePath = (season === 0 && episode === 0) ? 'movies' : 'series';
        const referer = `https://123movienow.cc/spa/videoPlayPage/${typePath}/${detailPath}?id=${subjectId}&type=/${typePath.slice(0, -1)}/detail&detailSe=&detailEp=&lang=en`;

        const url = `${PLAY_URL}/subject/play?subjectId=${subjectId}&se=${season}&ep=${episode}&detailPath=${detailPath}`;
        console.log(`[MovieBox API] Fetching streams for ${detailPath} (se:${season}, ep:${episode})`);
        
        const response = await axiosInstance.get(url, {
            headers: {
                'Referer': referer
            }
        });
        
        if (response.data && response.data.code === 0) {
            return response.data.data;
        }
        console.warn(`[MovieBox API] Play returned code ${response.data?.code}: ${response.data?.message}`);
        return null;
    } catch (e) {
        console.error('[MovieBox API] Play error:', e.message);
        return null;
    }
}

async function getCaptions(subjectId, detailPath, streamId) {
    try {
        const url = `${BASE_URL}/subject/caption?format=MP4&id=${streamId}&subjectId=${subjectId}&detailPath=${detailPath}`;
        const response = await axiosInstance.get(url);
        if (response.data && response.data.code === 0) {
            return response.data.data?.captions || [];
        }
        return [];
    } catch (e) {
        console.error('[MovieBox API] Caption error:', e.message);
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
            sort: params.sort || "ForYou"
        };
        const response = await axiosInstance.post(url, payload);
        if (response.data && response.data.code === 0) {
            return {
                page: response.data.data.pager.page,
                hasNext: response.data.data.pager.hasMore,
                items: (response.data.data.items || []).map(mapItem)
            };
        }
        return { items: [] };
    } catch (e) {
        console.error('[MovieBox API] Filter error:', e.message);
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
    // Channel 4 is typically Short TV in this API ecosystem
    return await filter({
        channelId: 4,
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
        console.error('[MovieBox API] Search error:', e.message);
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
    getDetail,
    getStreams,
    getCaptions,
    filter,
    getIndianContent,
    getShorts,
    getStreamHeaders,
    search
};
