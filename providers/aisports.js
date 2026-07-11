/**
 * Hue Stream API
 * Professional Streaming Aggregator
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */

/**
 * AiSports Provider
 * Fetches live sports streams from thesports.today (Aoneroom API)
 */

const axios = require('axios');
const cheerio = require('cheerio');

class AiSportsProvider {
    constructor() {
        this.baseUrl = 'https://thesports.today/';
        this.apiBaseUrl = 'https://h5-api.aoneroom.com/wefeed-h5api-bff';
        this.apiDetailUrl = `${this.apiBaseUrl}/sport/detail-v1`;
        this.apiListV5Url = `${this.apiBaseUrl}/live/match-list-v5?leagueId=0`;
        this.userAgent = 'Mozilla/5.0 (Linux; Android 6.0; Nexus 5 Build/MRA58N) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/147.0.0.0 Mobile Safari/537.36';
        this.defaultHeaders = {
            'Accept': 'application/json',
            'Content-Type': 'application/json',
            'User-Agent': this.userAgent,
            'Origin': 'https://thesports.today',
            'Referer': 'https://thesports.today/',
            'x-client-info': JSON.stringify({ timezone: 'Africa/Accra', system_language: 'en' }),
            'x-device-info': '{}'
        };
    }

    /**
     * Extract match name from path or query
     */
    extractMatchName(ctx) {
        const { matchName, req } = ctx;
        if (matchName) return matchName;
        if (req && req.path) {
            const match = req.path.match(/\/sports-stream\/(.+?)(?:\/|$)/);
            if (match) return match[1].replace(/\/$/, '');
        }
        return null;
    }

    /**
     * Get matches using the v5 API (cleanest source)
     */
    async getMatchesV5() {
        try {
            const response = await axios.get(this.apiListV5Url, {
                headers: this.defaultHeaders
            });

            if (response.data && response.data.code === 0 && response.data.data?.list) {
                return response.data.data.list.map(m => ({
                    id: m.id,
                    title: `${m.team1?.name} vs ${m.team2?.name}`,
                    team1: m.team1?.name || 'Unknown',
                    team2: m.team2?.name || 'Unknown',
                    playPath: m.playPath,
                    playSource: m.playSource,
                    highlights: m.highlights,
                    status: m.status,
                    league: m.league
                }));
            }
            return [];
        } catch (error) {
            console.error('[AiSports] V5 API failed:', error.message);
            return [];
        }
    }

    /**
     * Get matches from homepage (fallback)
     */
    async getMatchesFromHome() {
        try {
            const response = await axios.get(this.baseUrl, {
                headers: { 'User-Agent': this.userAgent }
            });
            const $ = cheerio.load(response.data);
            const jsonData = $('#__NUXT_DATA__').html();
            
            if (!jsonData) return [];

            const data = JSON.parse(jsonData);
            const matches = [];
            
            const getValue = (idx) => {
                if (typeof idx === 'number' && idx >= 0 && idx < data.length) {
                    return data[idx];
                }
                return idx;
            };

            for (let i = 0; i < data.length; i++) {
                const item = data[i];
                if (item && typeof item === 'object' && !Array.isArray(item)) {
                    if (item.id && item.team1 && item.team2) {
                        const matchId = getValue(item.id);
                        if (typeof matchId !== 'string' || matchId.length < 10) continue;

                        const team1 = getValue(item.team1);
                        const team2 = getValue(item.team2);
                        if (!team1 || !team2) continue;

                        matches.push({
                            id: matchId,
                            title: `${getValue(team1.name)} vs ${getValue(team2.name)}`,
                            team1: getValue(team1.name),
                            team2: getValue(team2.name),
                            playPath: getValue(item.playPath),
                            status: getValue(item.status),
                            league: getValue(item.league)
                        });
                    }
                }
            }
            return matches;
        } catch (error) {
            console.error('[AiSports] Homepage fallback failed:', error.message);
            return [];
        }
    }

    /**
     * Get detailed stream info for a match
     */
    async getMatchDetail(matchId) {
        try {
            const url = `${this.apiDetailUrl}?matchId=${matchId}`;
            const response = await axios.get(url, {
                headers: this.defaultHeaders
            });

            if (response.data && response.data.code === 0 && response.data.data) {
                return response.data.data.match;
            }
            return null;
        } catch (error) {
            console.error(`[AiSports] Detail fetch failed for ${matchId}:`, error.message);
            return null;
        }
    }

    /**
     * Main fetch function
     */
    async getStreams(ctx) {
        const query = this.extractMatchName(ctx);
        if (!query) return [];

        console.log(`[AiSports] Searching for: ${query}`);

        let activeMatch = null;
        const isDirectId = /^\d{15,}$/.test(query);
        if (isDirectId) {
            activeMatch = await this.getMatchDetail(query);
        } else {
            // Try V5 first, then Home page
            let matches = await this.getMatchesV5();
            if (matches.length === 0) {
                matches = await this.getMatchesFromHome();
            }

            const cleanQuery = query.toLowerCase().replace(/-/g, ' ');
            const bestMatch = matches.find(m => {
                const title = m.title.toLowerCase();
                const team1 = m.team1.toLowerCase();
                const team2 = m.team2.toLowerCase();
                return title.includes(cleanQuery) || cleanQuery.includes(team1) || cleanQuery.includes(team2) || m.id === query;
            });

            if (!bestMatch) {
                console.log(`[AiSports] No match found for "${query}"`);
                return [];
            }

            console.log(`[AiSports] Found match: ${bestMatch.title} (ID: ${bestMatch.id}) Status: ${bestMatch.status}`);

            // Always try to fetch fresh detail for more sources
            activeMatch = await this.getMatchDetail(bestMatch.id) || bestMatch;
        }

        if (!activeMatch) {
            console.log(`[AiSports] No match detail found for "${query}"`);
            return [];
        }

        const streams = [];
        const headers = this.defaultHeaders;

        // Add Live Streams
        if (activeMatch.playPath) {
            const isM3U8 = activeMatch.playPath.includes('.m3u8');
            streams.push({
                title: `${activeMatch.team1?.name || activeMatch.team1} vs ${activeMatch.team2?.name || activeMatch.team2} (Main)`,
                url: activeMatch.playPath,
                quality: 'Auto',
                provider: 'Field',
                source: 'aisports',
                isLive: true,
                isProxied: isM3U8,
                headers: headers,
                originalUrl: activeMatch.playPath
            });
        }

        if (activeMatch.playSource && Array.isArray(activeMatch.playSource)) {
            activeMatch.playSource.forEach((source, index) => {
                if (source.path) {
                    const isM3U8 = source.path.includes('.m3u8');
                    streams.push({
                        title: `${activeMatch.team1?.name || activeMatch.team1} vs ${activeMatch.team2?.name || activeMatch.team2} (${source.title || 'Channel ' + (index + 1)})`,
                        url: source.path,
                        quality: 'Auto',
                        provider: 'Field',
                        source: 'aisports',
                        isLive: true,
                        isProxied: isM3U8,
                        headers: headers,
                        originalUrl: source.path
                    });
                }
            });
        }

        // Add Highlights
        if (activeMatch.highlights && Array.isArray(activeMatch.highlights)) {
            activeMatch.highlights.forEach(h => {
                if (h.path) {
                    streams.push({
                        title: h.title || 'Highlight',
                        url: h.path,
                        quality: '720p',
                        provider: 'Stadium',
                        source: 'aisports-highlight',
                        isLive: false,
                        duration: h.duration
                    });
                }
            });
        }

        return streams;
    }
}

async function getAiSportsStreams(ctx) {
    const provider = new AiSportsProvider();
    return await provider.getStreams(ctx);
}

module.exports = { getAiSportsStreams };
