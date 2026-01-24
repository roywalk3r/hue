const axios = require('axios');
const http = require('http');
const https = require('https');

const BASE_URL = "https://api.xyra.stream/v1/dramacool";
const API_KEY = "key1";

// Force IPv4 for stability
const axiosInstance = axios.create({
    baseURL: BASE_URL,
    httpAgent: new http.Agent({ family: 4 }),
    httpsAgent: new https.Agent({ family: 4 }),
    timeout: 15000
});

async function search(query, page = 1) {
    try {
        const { data } = await axiosInstance.get('/search', {
            params: { api_key: API_KEY, query, page }
        });
        return data;
    } catch (e) {
        console.error(`[Dramacool API] Search failed: ${e.message}`);
        return { success: false, data: [] };
    }
}

async function getLatest(page = 1) {
    try {
        const { data } = await axiosInstance.get('/latest', {
            params: { api_key: API_KEY, page }
        });
        return data;
    } catch (e) {
        return { success: false, data: [] };
    }
}

async function getLatestKdrama(page = 1) {
    try {
        const { data } = await axiosInstance.get('/latest_kdrama', {
            params: { api_key: API_KEY, page }
        });
        return data;
    } catch (e) {
        return { success: false, data: [] };
    }
}

async function getPopular(page = 1) {
    try {
        const { data } = await axiosInstance.get('/popular', {
            params: { api_key: API_KEY, page }
        });
        return data;
    } catch (e) {
        return { success: false, data: [] };
    }
}

async function getOngoing(page = 1) {
    try {
        const { data } = await axiosInstance.get('/ongoing', {
            params: { api_key: API_KEY, page }
        });
        return data;
    } catch (e) {
        return { success: false, data: [] };
    }
}

async function getUpcoming(page = 1) {
    try {
        const { data } = await axiosInstance.get('/upcoming', {
            params: { api_key: API_KEY, page }
        });
        return data;
    } catch (e) {
        return { success: false, data: [] };
    }
}

async function getHome() {
    try {
        const { data } = await axiosInstance.get('/home', {
            params: { api_key: API_KEY }
        });
        return data;
    } catch (e) {
        return { success: false, data: {} };
    }
}

async function getDetail(id) {
    try {
        const { data } = await axiosInstance.get('/info', {
            params: { api_key: API_KEY, id }
        });
        return data;
    } catch (e) {
        return { success: false };
    }
}

async function getStream(episode_id) {
    try {
        const { data } = await axiosInstance.get('/stream', {
            params: { api_key: API_KEY, episode_id }
        });
        return data;
    } catch (e) {
        return { success: false };
    }
}

module.exports = {
    search,
    getLatest,
    getLatestKdrama,
    getPopular,
    getOngoing,
    getUpcoming,
    getHome,
    getDetail,
    getStream
};
