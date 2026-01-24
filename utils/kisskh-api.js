const axios = require('axios');
const http = require('http');
const https = require('https');

const BASE_URL = "https://kisskh.co/api";
const USER_AGENT = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

// Force IPv4 as per project convention for VPS stability
const axiosInstance = axios.create({
    baseURL: BASE_URL,
    headers: { 
        "User-Agent": USER_AGENT, 
        "Accept": "application/json" 
    },
    httpAgent: new http.Agent({ family: 4 }),
    httpsAgent: new https.Agent({ family: 4 }),
    timeout: 15000
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

async function search(query) {
    try {
        const url = `/DramaList/Search?q=${encodeURIComponent(query)}`;
        const { data } = await axiosInstance.get(url);
        return data || [];
    } catch (e) {
        console.error(`[KissKH API] Search failed: ${e.message}`);
        return [];
    }
}

async function getList({ page = 1, type = 0, sub = 0, country = 0, status = 0, order = 2 } = {}) {
    try {
        // type: 0=All?, 1=Drama? (Plan log mostly uses type=1 or 0)
        // sub: 0=All?, 1=English Sub?
        const params = new URLSearchParams({
            page, type, sub, country, status, order
        });
        const url = `/DramaList/List?${params.toString()}`;
        const { data } = await axiosInstance.get(url);
        return data; // Returns object with { page, pageSize, totalCount, data: [] }
    } catch (e) {
        console.error(`[KissKH API] List failed: ${e.message}`);
        return { data: [] };
    }
}

async function getDetail(id) {
    try {
        const url = `/DramaList/Drama/${id}?isq=false`;
        const { data } = await axiosInstance.get(url);
        return data;
    } catch (e) {
        console.error(`[KissKH API] Detail failed: ${e.message}`);
        return null;
    }
}

async function getUpcoming() {
    try {
        const url = `/DramaList/upcoming`;
        const { data } = await axiosInstance.get(url);
        return data || [];
    } catch (e) {
        console.error(`[KissKH API] Upcoming failed: ${e.message}`);
        return [];
    }
}

async function getEpisodeIds(id) {
    try {
        const data = await getDetail(id);
        if (!data || !data.episodes) return [];
        return data.episodes;
    } catch (e) {
        return [];
    }
}

module.exports = {
    search,
    getList,
    getDetail,
    getUpcoming,
    getEpisodeIds,
    COUNTRY_MAP,
    ORDER_MAP,
    STATUS_MAP
};
