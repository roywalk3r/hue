/**
 * Hue Stream API
 * Professional Streaming Aggregator
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */

const axios = require('axios');

const BASE_URL = 'https://h5-api.aoneroom.com/wefeed-h5api-bff';
const DEFAULT_HEADERS = {
    'Accept': 'application/json',
    'Content-Type': 'application/json',
    'User-Agent': 'Mozilla/5.0 (Linux; Android 6.0; Nexus 5 Build/MRA58N) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/147.0.0.0 Mobile Safari/537.36',
    'Origin': 'https://thesports.today',
    'Referer': 'https://thesports.today/',
    'x-client-info': JSON.stringify({ timezone: 'Africa/Accra', system_language: 'en' }),
    'x-device-info': '{}'
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
