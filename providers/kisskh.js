/**
 * Hue Stream API
 * Professional Streaming Aggregator
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */

const axios = require("axios");
const { getNewContext } = require("../utils/browserManager");
const http = require('http');
const https = require('https');

const { getTmdbInfo } = require('../utils/tmdb');

const BASE_URL = "https://kisskh.ovh";
const API_BASE = "https://kisskh.ovh/api";
const USER_AGENT = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

const axiosInstance = axios.create({
    headers: { "User-Agent": USER_AGENT, "Accept": "application/json" },
    httpAgent: new http.Agent({ family: 4, keepAlive: true }),
    httpsAgent: new https.Agent({ family: 4, keepAlive: true }),
    timeout: 30000
});

async function searchShow(title, season) {
    let searchTitle = title;
    // Prefer searching for the base title first as KissKH often groups seasons under one show
    // or has specific naming conventions.
    const baseTitle = title.replace(/\s+Season\s+\d+/i, '').trim();

    console.log(`[KissKH] API Search: ${baseTitle}`);
    const url = `${API_BASE}/DramaList/Search?q=${encodeURIComponent(baseTitle)}`;
    try {
        const resp = await axiosInstance.get(url);
        const results = resp.data;
        if (!Array.isArray(results) || results.length === 0) {
            // Fallback: Try with full title if base title failed
            if (searchTitle !== baseTitle) {
                 console.log(`[KissKH] API Search fallback: ${searchTitle}`);
                 const url2 = `${API_BASE}/DramaList/Search?q=${encodeURIComponent(searchTitle)}`;
                 const resp2 = await axiosInstance.get(url2);
                 const results2 = resp2.data;
                 if (Array.isArray(results2) && results2.length > 0) {
                     return findBestMatch(results2, title, season);
                 }
            }
            return null;
        }
        return findBestMatch(results, title, season);
    } catch (e) {
        console.error(`[KissKH] Search error: ${e.message}`);
        return null;
    }
}

function findBestMatch(results, title, season, targetYear) {
    const normalize = (str) => str.toLowerCase().replace(/[^a-z0-9]/g, "");
    const targetTitle = normalize(title);
    const targetSeason = season ? normalize(`season${season}`) : "";
    
    let bestMatch = { item: null, score: 0 };

    for (const item of results) {
        const itemTitle = normalize(item.title);
        let score = 0;

        if (itemTitle === targetTitle) score += 100;
        else if (itemTitle.includes(targetTitle)) score += 50;

        if (season) {
            if (itemTitle.includes(targetSeason) || itemTitle.includes(normalize(`Season ${season}`))) {
                score += 50;
            }
        }
        
        if (targetYear) {
            if (item.title.includes(targetYear.toString())) {
                score += 200;
            } else if (/\d{4}/.test(item.title) && !item.title.includes(targetYear.toString())) {
                score -= 150; 
            }
        }

        if (score > bestMatch.score) {
            bestMatch = { item, score };
        }
    }

    return bestMatch.item ? { id: bestMatch.item.id, title: bestMatch.item.title } : null;
}

async function getEpisodeId(showId, episodeNumber) {
    const url = `${API_BASE}/DramaList/Drama/${showId}?is_series=1`;
    try {
        const resp = await axiosInstance.get(url);
        const data = resp.data;
        const episodes = data.episodes;
        if (!Array.isArray(episodes)) return null;

        const ep = episodes.find((e) => Math.floor(e.number) === episodeNumber);
        return ep ? ep.id : null;
    } catch (e) {
        return null;
    }
}

async function extractStreamsFromKissKhPage(directUrl) {
    console.log(`[KissKH] Extracting from: ${directUrl}`);
    const streams = [];
    let context;
    let page;
    let episodeApiUrl = null;

    // KissKH episode API can return JSON or base64-encoded JSON.
    function parseEpisodePayload(payload) {
        if (!payload) return null;
        if (typeof payload === "object") return payload;
        if (typeof payload !== "string") return null;

        try {
            return JSON.parse(payload);
        } catch (_) {
            try {
                const decoded = Buffer.from(payload, "base64").toString("utf8");
                return JSON.parse(decoded);
            } catch (err) {
                return null;
            }
        }
    }

    function addSubtitleEntries(subtitles, capturedUrls) {
        if (!Array.isArray(subtitles)) return;
        subtitles.forEach((sub) => {
            if (!sub || !sub.src) return;
            const label = sub.label || sub.lang || sub.land || "Subtitle";
            const lang = (sub.lang || sub.land || label || "en").toLowerCase().substring(0, 2);
            capturedUrls.subtitles.add(JSON.stringify({
                url: sub.src,
                label,
                lang
            }));
        });
    }

    try {
        const { getNewPage } = require("../utils/browserManager");
        page = await getNewPage();
        context = page.context();

        // Aggressive ad-blocking
        await page.route('**/*', route => {
            const u = route.request().url();
            if (u.includes('google') || u.includes('facebook') || u.includes('analytics') || 
                u.includes('doubleclick') || u.includes('quantserve') || u.includes('adsystem') || 
                u.includes('fonts') || u.includes('disqus')) {
                return route.abort();
            }
            if ((u.endsWith('.png') || u.endsWith('.jpg') || u.endsWith('.jpeg')) &&
                !u.includes("/api/DramaList/Episode/")) {
                return route.abort();
            }
            route.continue();
        });

        const capturedUrls = { video: new Set(), subtitles: new Set() };
        
        page.on("request", (request) => {
            const url = request.url();
            if ((url.includes(".m3u8") || url.includes(".mp4") || url.includes(".ts") || url.includes("streamcdn") || url.includes("lovestreaming") || url.includes("index.m3u8")) && !url.includes("blob:")) {
                capturedUrls.video.add(url);
            }
            if (url.includes(".vtt") || url.includes(".srt") || url.includes("streamsub.top") || url.includes("sub.wyzie.ru")) {
                const isDuplicate = Array.from(capturedUrls.subtitles).some(s => {
                    try { return JSON.parse(s).url === url; } catch { return s === url; }
                });
                if (!isDuplicate) capturedUrls.subtitles.add(url);
            }
        });

        page.on("response", async (response) => {
            const url = response.url();
            if (url.includes("/api/DramaList/Episode/")) {
                episodeApiUrl = url;
                try {
                    const text = await response.text();
                    const data = parseEpisodePayload(text);
                    if (!data) return;
                    if (data.Video) capturedUrls.video.add(data.Video);
                    if (data.ThirdParty) capturedUrls.video.add(data.ThirdParty);
                    if (Array.isArray(data.Subtitles)) {
                        addSubtitleEntries(data.Subtitles, capturedUrls);
                    }
                } catch (e) {
                    console.warn(`[KissKH] Episode API parse failed: ${e.message}`);
                }
            }
            if (url.includes("/api/Sub/")) {
                try {
                    const text = await response.text();
                    const data = parseEpisodePayload(text);
                    addSubtitleEntries(data, capturedUrls);
                } catch (e) {
                    console.warn(`[KissKH] Subtitle API parse failed: ${e.message}`);
                }
            }
        });

        await page.goto(directUrl, { waitUntil: "commit", timeout: 30000 });
        try { await page.waitForSelector('video, iframe', { timeout: 15000 }); } catch (e) {}
        await page.evaluate(() => {
            const v = document.querySelector('video');
            if (v) v.play().catch(() => {});
        });
        
        // Dynamic wait for streams (max 15s)
        const start = Date.now();
        while (Date.now() - start < 15000) {
            if (capturedUrls.video.size > 0) break;
            await new Promise(r => setTimeout(r, 500));
        }

        if (capturedUrls.video.size === 0 && episodeApiUrl) {
            try {
                const cookies = await context.cookies(directUrl);
                const cookieString = cookies.map(c => `${c.name}=${c.value}`).join('; ');
                
                const apiResp = await axiosInstance.get(episodeApiUrl, {
                    headers: {
                        "Referer": directUrl,
                        "User-Agent": USER_AGENT,
                        "Accept": "application/json",
                        "Cookie": cookieString
                    }
                });
                const data = parseEpisodePayload(apiResp.data);
                if (data && data.Video) capturedUrls.video.add(data.Video);
                if (data && data.ThirdParty) capturedUrls.video.add(data.ThirdParty);
                if (data && Array.isArray(data.Subtitles)) {
                    addSubtitleEntries(data.Subtitles, capturedUrls);
                }
            } catch (error) {
                console.warn(`[KissKH] Episode API retry failed: ${error.message}`);
            }
        }

        if (capturedUrls.subtitles.size === 0 && episodeApiUrl) {
            try {
                const cookies = await context.cookies(directUrl);
                const cookieString = cookies.map(c => `${c.name}=${c.value}`).join('; ');
                
                const episodeUrl = new URL(directUrl);
                const episodeId = episodeUrl.searchParams.get("ep");
                const apiUrl = new URL(episodeApiUrl);
                const kkey = apiUrl.searchParams.get("kkey");
                if (episodeId && kkey) {
                    const subUrl = `${API_BASE}/Sub/${episodeId}?kkey=${kkey}`;
                    const subResp = await axiosInstance.get(subUrl, {
                        headers: {
                            "Referer": directUrl,
                            "User-Agent": USER_AGENT,
                            "Accept": "application/json",
                            "Cookie": cookieString
                        }
                    });
                    const data = parseEpisodePayload(subResp.data);
                    addSubtitleEntries(data, capturedUrls);
                }
            } catch (error) {
                if (capturedUrls.subtitles.size === 0) {
                    console.warn(`[KissKH] Subtitle API retry failed: ${error.message}`);
                }
            }
        }

        // Fallback: Scan page source for subtitle links if none captured
        const extraSubs = await page.evaluate(() => {
            const scripts = Array.from(document.querySelectorAll('script')).map(s => s.innerText);
            const found = [];
            scripts.forEach(s => {
                const m = s.match(/https?:\/\/[^"']+\.(vtt|srt)[^"']*/g);
                if (m) found.push(...m);
            });
            return found;
        });
        extraSubs.forEach(s => capturedUrls.subtitles.add(s));

        const subtitleMap = new Map();
        Array.from(capturedUrls.subtitles).forEach((s) => {
            let entry;
            try { entry = JSON.parse(s); } catch { entry = { url: s, lang: 'en', label: 'English' }; }
            if (entry && entry.url && !subtitleMap.has(entry.url)) {
                subtitleMap.set(entry.url, entry);
            }
        });
        const subtitles = Array.from(subtitleMap.values());

        for (const url of capturedUrls.video) {
            if (url.endsWith(".ts") && Array.from(capturedUrls.video).some(u => u.includes(".m3u8"))) continue;
            const isThirdParty = url.includes("sbplay") || url.includes("ok.ru") || url.includes("vidoza");
            const isHls = url.includes('.m3u8');
            streams.push({
                url,
                title: isThirdParty ? "ThirdParty" : "Auto",
                quality: isThirdParty ? "ThirdParty" : "Auto",
                provider: "Zenith",
                format: isHls ? 'hls' : 'mp4',
                subtitles: subtitles.length > 0 ? subtitles : undefined,
                // Use proxy URL mode (no tokenization) to preserve headers for browser playback.
                proxyMode: "url",
                headers: { "Referer": BASE_URL, "User-Agent": USER_AGENT }
            });
        }
    } catch (error) {
        console.error(`[KissKH] Browser Error: ${error.message}`);
    } finally {
        if (context) await context.close();
    }
    return streams;
}

async function getKissKhStreams(tmdbId, mediaType, season, episode) {
    console.log(`[KissKH] Fetching streams for TMDB ID: ${tmdbId} (${mediaType}) S${season} E${episode}`);
    try {
        const { title, year } = await getTmdbInfo(tmdbId, mediaType);
        if (!title) return [];
        const show = await searchShow(title, season);
        if (!show) return [];
        
        const finalShow = await (async () => {
            const url = `${API_BASE}/DramaList/Search?q=${encodeURIComponent(title)}`;
            const resp = await axiosInstance.get(url);
            return findBestMatch(resp.data, title, season, year);
        })();

        const selectedShow = finalShow || show;
        const epNum = parseInt(episode || "1");
        const episodeId = await getEpisodeId(selectedShow.id, epNum);
        if (!episodeId) return [];

        // Consistently use /Drama/Direct/ for direct extraction
        const directUrl = `${BASE_URL}/Drama/Direct/Episode-${epNum}?id=${selectedShow.id}&ep=${episodeId}&page=0&pageSize=100`;
        return await extractStreamsFromKissKhPage(directUrl);
    } catch (error) {
        console.error(`[KissKH] Error: ${error.message}`);
        return [];
    }
}

async function getStreamsDirect(idOrTitle, episodeNumber) {
    console.log(`[KissKH] Smart Fetch for: "${idOrTitle}" Episode ${episodeNumber}`);
    let showId = idOrTitle;

    try {
        // If input is NOT a number, assume it's a title and search for the ID
        if (isNaN(parseInt(idOrTitle))) {
            console.log(`[KissKH] Resolving title "${idOrTitle}" to ID...`);
            const show = await searchShow(idOrTitle);
            if (!show) {
                console.log(`[KissKH] Title not found: ${idOrTitle}`);
                return [];
            }
            showId = show.id;
            console.log(`[KissKH] Resolved "${idOrTitle}" -> ID ${showId}`);
        }

        const epNum = parseInt(episodeNumber || "1");
        const episodeId = await getEpisodeId(showId, epNum);
        if (!episodeId) return [];
        const directUrl = `${BASE_URL}/Drama/Direct/Episode-${epNum}?id=${showId}&ep=${episodeId}&page=0&pageSize=100`;
        return await extractStreamsFromKissKhPage(directUrl);
    } catch (error) {
        console.error(`[KissKH] Error: ${error.message}`);
        return [];
    }
}

module.exports = { 
    getStreams: getKissKhStreams,
    getStreamsDirect
};
