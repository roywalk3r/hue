const { getNewPage } = require("./browserManager");

const BASE_URL = "https://kisskh.co/api";

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

async function fetchWithBrowser(endpoint) {
    let page;
    try {
        page = await getNewPage({ browserType: 'chromium' });
        const url = `${BASE_URL}${endpoint}`;
        
        const data = await page.evaluate(async (targetUrl) => {
            try {
                const res = await fetch(targetUrl);
                if (!res.ok) return null;
                return await res.json();
            } catch (e) { return null; }
        }, url);
        
        return data;
    } catch (e) {
        console.error(`[KissKH API] Browser fetch failed for ${endpoint}: ${e.message}`);
        return null;
    } finally {
        if (page) await page.context().close();
    }
}

async function search(query) {
    console.log(`[KissKH API] Searching (Browser): ${query}`);
    const data = await fetchWithBrowser(`/DramaList/Search?q=${encodeURIComponent(query)}`);
    return data || [];
}

async function getList({ page = 1, type = 0, sub = 0, country = 0, status = 0, order = 2 } = {}) {
    const params = new URLSearchParams({ page, type, sub, country, status, order });
    const data = await fetchWithBrowser(`/DramaList/List?${params.toString()}`);
    return data || { data: [] };
}

async function getDetail(id) {
    const data = await fetchWithBrowser(`/DramaList/Drama/${id}?isq=false`);
    return data;
}

async function getUpcoming() {
    const data = await fetchWithBrowser(`/DramaList/upcoming`);
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
    getEpisodeIds,
    COUNTRY_MAP,
    ORDER_MAP,
    STATUS_MAP
};
