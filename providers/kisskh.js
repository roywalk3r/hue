const axios = require("axios");
const { getNewContext } = require("../utils/browserManager");
const http = require('http');
const https = require('https');

const BASE_URL = "https://kisskh.ovh";
const API_BASE = "https://kisskh.ovh/api";
const USER_AGENT = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

const axiosInstance = axios.create({
    headers: { "User-Agent": USER_AGENT, "Accept": "application/json" },
    httpAgent: new http.Agent({ family: 4, keepAlive: true }),
    httpsAgent: new https.Agent({ family: 4, keepAlive: true }),
    timeout: 30000
});

async function getTmdbInfo(tmdbId, mediaType) {
    const { getTmdbApiKey } = require("../utils/tmdbKey");
    const TMDB_API_KEY = getTmdbApiKey();
    if (!TMDB_API_KEY) return { title: null, year: null };

    const type = mediaType === 'series' || mediaType === 'tv' ? 'tv' : 'movie';
    const url = `https://api.themoviedb.org/3/${type}/${tmdbId}?api_key=${TMDB_API_KEY}`;
    const response = await axiosInstance.get(url);
    const data = response.data;
    const releaseDate = data.release_date || data.first_air_date;
    const year = releaseDate ? parseInt(releaseDate.split("-")[0], 10) : null;
    const title = data.title || data.name;
    return { title, year: isNaN(year) ? null : year };
}

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
            if (u.endsWith('.png') || u.endsWith('.jpg') || u.endsWith('.jpeg')) return route.abort();
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
            if (url.includes("/api/DramaList/Episode/") && url.includes(".png")) {
                try {
                    const data = await response.json();
                    if (data && data.Video) capturedUrls.video.add(data.Video);
                    if (data && data.ThirdParty) capturedUrls.video.add(data.ThirdParty);
                    if (data && Array.isArray(data.Subtitles)) {
                        data.Subtitles.forEach(sub => {
                            if (sub.src) {
                                capturedUrls.subtitles.add(JSON.stringify({
                                    url: sub.src,
                                    label: sub.label || 'English',
                                    lang: (sub.label || 'en').toLowerCase().substring(0, 2)
                                }));
                            }
                        });
                    }
                } catch (e) {}
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

        const subtitles = Array.from(capturedUrls.subtitles).map(s => {
            try { return JSON.parse(s); } catch { return { url: s, lang: 'en', label: 'English' }; }
        });

        for (const url of capturedUrls.video) {
            if (url.endsWith(".ts") && Array.from(capturedUrls.video).some(u => u.includes(".m3u8"))) continue;
            const isThirdParty = url.includes("sbplay") || url.includes("ok.ru") || url.includes("vidoza");
            streams.push({
                url,
                quality: isThirdParty ? "ThirdParty" : "Auto",
                provider: "Zenith",
                subtitles: subtitles.length > 0 ? subtitles : undefined,
                headers: { "Referer": BASE_URL, "Origin": BASE_URL, "User-Agent": USER_AGENT }
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

        const slug = selectedShow.title.replace(/[^a-zA-Z0-9]/g, "-");
        const directUrl = `${BASE_URL}/Drama/${slug}/Episode-${epNum}?id=${selectedShow.id}&ep=${episodeId}&page=0&pageSize=100`;
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
