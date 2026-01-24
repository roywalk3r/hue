const axios = require("axios");
const { getNewPage } = require("../utils/browserManager");

const XYRA_BASE = "https://api.xyra.stream/v1/dramacool";
const XYRA_KEY = "key1";
const USER_AGENT = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

async function makeRequest(endpoint, params = {}) {
    try {
        const url = `${XYRA_BASE}${endpoint}`;
        const queryParams = new URLSearchParams({ api_key: XYRA_KEY, ...params }).toString();
        const response = await axios.get(`${url}?${queryParams}`, { 
            headers: { "User-Agent": USER_AGENT },
            timeout: 10000 
        });
        return response.data;
    } catch (e) {
        return e.response ? e.response.data : null;
    }
}

async function extractFromUrlWithPlaywright(url) {
    console.log(`[Dramacool] [Custom] Extracting from: ${url}`);
    const streams = [];
    let page;

    try {
        page = await getNewPage();
        const capturedUrls = new Set();
        
        page.on("request", (req) => {
            const u = req.url();
            const isVideo = u.includes(".m3u8") || u.includes(".mp4") || u.includes("master.m3u8") || u.includes("index.m3u8");
            if (isVideo && !u.includes("google") && !u.includes("analytics")) {
                capturedUrls.add(u);
            }
        });

        await page.goto(url, { waitUntil: "domcontentloaded", timeout: 30000 });

        // Trigger Playback
        await page.evaluate(() => {
            const triggers = ['video', '.vjs-big-play-button', '#play-now', '.play-btn', '.jw-display-icon-container'];
            triggers.forEach(s => {
                const el = document.querySelector(s);
                if (el) el.click();
            });
            const v = document.querySelector('video');
            if (v) v.play().catch(() => {});
        });

        // Wait for capture
        const start = Date.now();
        while (Date.now() - start < 10000 && capturedUrls.size === 0) {
            await new Promise(r => setTimeout(r, 500));
        }

        for (const u of capturedUrls) {
            streams.push({
                url: u,
                quality: "Auto",
                provider: "Dramacool",
                headers: { "Referer": url, "User-Agent": USER_AGENT }
            });
        }
    } catch (e) {
        console.error(`[Dramacool] [Custom] Error: ${e.message}`);
    } finally {
        if (page) await page.context().close();
    }
    return streams;
}

async function getStreams(tmdbId, mediaType, season, episode) {
    const { getTmdbApiKey } = require("../utils/tmdbKey");
    const TMDB_API_KEY = getTmdbApiKey();
    if (!TMDB_API_KEY) return [];

    try {
        const type = (mediaType === 'tv' || mediaType === 'series') ? 'tv' : 'movie';
        const { data: tmdbData } = await axios.get(`https://api.themoviedb.org/3/${type}/${tmdbId}?api_key=${TMDB_API_KEY}`);
        const baseTitle = tmdbData.name || tmdbData.title;
        const releaseYear = (tmdbData.first_air_date || tmdbData.release_date || "").split('-')[0];
        
        let title = baseTitle;
        if (type === 'tv' && season && season > 1) title = `${baseTitle} Season ${season}`;

        console.log(`[Dramacool] Processing: ${title} (${releaseYear})`);

        let searchRes = await makeRequest("/search", { query: baseTitle });
        if (!searchRes || !searchRes.success || !searchRes.data) return [];

        const normalize = s => s.toLowerCase().replace(/[^a-z0-9]/g, "");
        const targetNorm = normalize(baseTitle);
        const seasonNorm = season ? normalize(`season${season}`) : "";

        // Find best match: check title + season + year
        let bestMatch = searchRes.data.find(r => {
            const rTitle = normalize(r.title);
            const matchesTitle = rTitle.includes(targetNorm);
            const matchesSeason = season > 1 ? rTitle.includes(seasonNorm) : (!rTitle.includes("season") || rTitle.includes("season1") || rTitle.includes("season01"));
            const matchesYear = releaseYear && r.releaseYear === releaseYear;
            return matchesTitle && matchesSeason && matchesYear;
        });

        // Fallback 1: Title + Season
        if (!bestMatch) {
            bestMatch = searchRes.data.find(r => {
                const rTitle = normalize(r.title);
                return rTitle.includes(targetNorm) && (season > 1 ? rTitle.includes(seasonNorm) : true);
            });
        }

        // Fallback 2: Just use first result if title matches
        if (!bestMatch) {
            bestMatch = searchRes.data.find(r => normalize(r.title).includes(targetNorm)) || searchRes.data[0];
        }

        if (!bestMatch) {
            console.log(`[Dramacool] No drama found for: ${title}`);
            return [];
        }

        const dramaId = bestMatch.id;
        console.log(`[Dramacool] Using Drama: "${bestMatch.title}" (ID: ${dramaId})`);

        const infoRes = await makeRequest("/info", { id: dramaId });
        if (!infoRes || !infoRes.success) return [];
        const epNum = episode || 1;
        const ep = infoRes.data.episodes.find(e => {
            const m = e.title.match(/Episode\s+(\d+)/i);
            return m && parseFloat(m[1]) === parseFloat(epNum);
        });

        if (!ep) {
            console.log(`[Dramacool] Episode ${epNum} not found in episode list.`);
            return [];
        }

        const streamRes = await makeRequest("/stream", { episode_id: ep.episode_id });
        
        // If Xyra found the link directly, use it
        if (streamRes && streamRes.success && streamRes.data && streamRes.data.link && streamRes.data.link.includes('.m3u8')) {
            console.log("[Dramacool] Xyra found direct link.");
            return [{
                url: streamRes.data.link,
                quality: "Auto",
                provider: "Dramacool",
                headers: { "Referer": "https://dramacool.sh/", "User-Agent": USER_AGENT }
            }];
        }

        // Otherwise use the Custom Browser Scraper on the server_url
        // server_url can be in streamRes.server_url (on error) or streamRes.data.server_url (on success)
        const serverUrl = streamRes?.server_url || streamRes?.data?.server_url;
        if (serverUrl) {
            return await extractFromUrlWithPlaywright(serverUrl);
        }

        console.log("[Dramacool] No playable stream or server URL found.");
        return [];
    } catch (e) {
        console.error(`[Dramacool] Provider Error: ${e.message}`);
        return [];
    }
}

module.exports = { getStreams };
