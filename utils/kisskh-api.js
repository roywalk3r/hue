/**
 * Hue Stream API
 * Professional Streaming Aggregator
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */

const axios = require("axios");
const http = require('http');
const https = require('https');

const BASE_URL = "https://kisskh.ovh/api";
const USER_AGENT = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

const axiosInstance = axios.create({
    headers: { "User-Agent": USER_AGENT, "Accept": "application/json" },
    httpAgent: new http.Agent({ family: 4 }),
    httpsAgent: new https.Agent({ family: 4 }),
    timeout: 10000
});

const COUNTRY_MAP = {
    'all': 0,
    'cn': 1,
    'kr': 2,
    'jp': 3,
    'hk': 4,
    'th': 5,
    'tw': 7,
    'ph': 8
};

const ORDER_MAP = {
    'popular': 1,
    'latest': 2,
    'release': 3
};

const STATUS_MAP = {
    'all': 0,
    'ongoing': 1,
    'completed': 2
};

async function fetchWithAxios(endpoint) {
    try {
        const url = `${BASE_URL}${endpoint}`;
        const res = await axiosInstance.get(url);
        return res.data;
    } catch (e) {
        console.error(`[KissKH API] Fetch failed for ${endpoint}: ${e.message}`);
        return null;
    }
}

async function search(query) {
    console.log(`[KissKH API] Searching: ${query}`);
    const data = await fetchWithAxios(`/DramaList/Search?q=${encodeURIComponent(query)}`);
    return data || [];
}

async function getList({ page = 1, type = 0, sub = 0, country = 0, status = 0, order = 2 } = {}) {
    const params = new URLSearchParams({ page, type, sub, country, status, order });
    const data = await fetchWithAxios(`/DramaList/List?${params.toString()}`);
    return data || { data: [] };
}

async function getDetail(id) {
    const data = await fetchWithAxios(`/DramaList/Drama/${id}?isq=false`);
    return data;
}

async function getUpcoming() {
    const data = await fetchWithAxios(`/DramaList/upcoming`);
    return data || [];
}

async function getShowList() {
    const data = await fetchWithAxios(`/DramaList/Show`);
    return data || [];
}

async function getLastUpdate({ ispc = false } = {}) {
    const params = new URLSearchParams({ ispc: String(ispc) });
    const data = await fetchWithAxios(`/DramaList/LastUpdate?${params.toString()}`);
    return data || [];
}

async function getMostView({ ispc = false, c = 1 } = {}) {
    const params = new URLSearchParams({ ispc: String(ispc), c: String(c) });
    const data = await fetchWithAxios(`/DramaList/MostView?${params.toString()}`);
    return data || [];
}

async function getTopRating({ ispc = false } = {}) {
    const params = new URLSearchParams({ ispc: String(ispc) });
    const data = await fetchWithAxios(`/DramaList/TopRating?${params.toString()}`);
    return data || [];
}

async function getAnimate({ ispc = false } = {}) {
    const params = new URLSearchParams({ ispc: String(ispc) });
    const data = await fetchWithAxios(`/DramaList/Animate?${params.toString()}`);
    return data || [];
}

async function getEpisodeIds(id) {
    const data = await getDetail(id);
    if (!data || !data.episodes) return [];
    return data.episodes;
}

module.exports = {
    search,
    getList,
    getDetail,
    getUpcoming,
    getShowList,
    getLastUpdate,
    getMostView,
    getTopRating,
    getAnimate,
    getEpisodeIds,
    COUNTRY_MAP,
    ORDER_MAP,
    STATUS_MAP
};
