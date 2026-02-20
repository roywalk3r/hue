const axios = require('axios');

const BASE_URL = 'https://h5.aoneroom.com/wefeed-h5-bff';
const DEFAULT_HEADERS = {
    'User-Agent': 'Mozilla/5.0 (iPhone; CPU iPhone OS 18_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.5 Mobile/15E148 Safari/604.1',
    'Origin': 'https://sportslivenow.top',
    'Referer': 'https://sportslivenow.top/'
};

async function getLeagues() {
    try {
        const url = `${BASE_URL}/live/league-tab`;
        const response = await axios.get(url, { headers: DEFAULT_HEADERS });
        if (response.data && response.data.code === 0) {
            return response.data.data.list || [];
        }
        return [];
    } catch (e) {
        console.error('[AiSports API] Leagues error:', e.message);
        return [];
    }
}

async function getMatchList(leagueId = 0, status = null, matchType = null) {
    try {
        let url;
        if (status !== null || matchType !== null) {
            // Use v3 for filtered lists by status/type
            const now = Date.now();
            const dayMs = 24 * 3600 * 1000;
            const startTime = now - dayMs;
            const endTime = now + dayMs;
            url = `${BASE_URL}/live/match-list-v3?status=${status || 0}&matchType=${matchType || 'football'}&startTime=${startTime}&endTime=${endTime}`;
        } else {
            // Default to v5 by leagueId
            url = `${BASE_URL}/live/match-list-v5?leagueId=${leagueId}`;
        }
        
        const response = await axios.get(url, { headers: DEFAULT_HEADERS });
        if (response.data && response.data.code === 0) {
            return response.data.data || { list: [] };
        }
        return { list: [] };
    } catch (e) {
        console.error('[AiSports API] Match list error:', e.message);
        return { list: [] };
    }
}

async function getMatchDetail(matchId) {
    try {
        const url = `${BASE_URL}/sport/detail-v1?matchId=${matchId}`;
        const response = await axios.get(url, { headers: DEFAULT_HEADERS });
        if (response.data && response.data.code === 0) {
            return response.data.data?.match || null;
        }
        console.warn(`[AiSports API] Detail returned code ${response.data?.code}: ${response.data?.message}`);
        return null;
    } catch (e) {
        console.error('[AiSports API] Detail error:', e.response?.data || e.message);
        return null;
    }
}

async function getMatchFlow(matchId, pageSize = 30, lastSort = 0) {
    try {
        const url = `${BASE_URL}/live/match-flow?id=${matchId}&pageSize=${pageSize}&lastSort=${lastSort}`;
        const response = await axios.get(url, { headers: DEFAULT_HEADERS });
        if (response.data && response.data.code === 0) {
            return response.data.data || { list: [] };
        }
        return { list: [] };
    } catch (e) {
        console.error('[AiSports API] Flow error:', e.message);
        return { list: [] };
    }
}

module.exports = {
    getLeagues,
    getMatchList,
    getMatchDetail,
    getMatchFlow
};
