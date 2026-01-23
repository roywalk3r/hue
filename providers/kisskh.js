const axios = require("axios");
const { getNewContext } = require("../utils/browserManager");
const http = require('http');
const https = require('https');

const BASE_URL = "https://kisskh.ovh";
const API_BASE = "https://kisskh.ovh/api";
const USER_AGENT = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

const axiosInstance = axios.create({
    headers: { "User-Agent": USER_AGENT, "Accept": "application/json" },
    httpAgent: new http.Agent({ family: 4 }),
    httpsAgent: new https.Agent({ family: 4 }),
    timeout: 10000
});

async function getTmdbInfo(tmdbId, mediaType) {
    const { getTmdbApiKey } = require("../utils/tmdbKey");
    const TMDB_API_KEY = getTmdbApiKey();
    if (!TMDB_API_KEY) return { title: null, year: null };

    const url = `https://api.themoviedb.org/3/${mediaType}/${tmdbId}?api_key=${TMDB_API_KEY}`;
    const response = await axiosInstance.get(url);
    const data = response.data;
    const releaseDate = data.release_date || data.first_air_date;
    const year = releaseDate ? parseInt(releaseDate.split("-")[0], 10) : null;
    const title = data.title || data.name;
    return { title, year: isNaN(year) ? null : year };
}

async function searchShow(title, season) {
    let searchTitle = title;
    if (season && parseInt(season) > 0) {
        searchTitle = `${title} Season ${season}`;
    }

    console.log(`[KissKH] API Search: ${searchTitle}`);
    const url = `${API_BASE}/DramaList/Search?q=${encodeURIComponent(searchTitle)}`;
    try {
        const resp = await axiosInstance.get(url);
        const results = resp.data;
        if (!Array.isArray(results) || results.length === 0) {
            if (searchTitle !== title) {
                 console.log(`[KissKH] API Search fallback: ${title}`);
                 const url2 = `${API_BASE}/DramaList/Search?q=${encodeURIComponent(title)}`;
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

function findBestMatch(results, title, season) {
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

async function getKissKhStreams(tmdbId, mediaType, season, episode) {
    console.log(`[KissKH] Fetching streams for TMDB ID: ${tmdbId} (${mediaType}) S${season} E${episode}`);
    const streams = [];
    let context;
    let page;

    try {
        const { title, year } = await getTmdbInfo(tmdbId, mediaType);
        if (!title) return [];
        console.log(`[KissKH] Resolved title: ${title} (${year})`);

        const show = await searchShow(title, season);
        if (!show) {
            console.log("[KissKH] Show not found via API.");
            return [];
        }
        console.log(`[KissKH] Found Show: "${show.title}" (ID: ${show.id})`);

        const epNum = parseInt(episode || "1");
        const episodeId = await getEpisodeId(show.id, epNum);
        if (!episodeId) {
            console.log(`[KissKH] Episode ${epNum} not found in API.`);
            return [];
        }
        console.log(`[KissKH] Found Episode ID: ${episodeId}`);

        const slug = show.title.replace(/[^a-zA-Z0-9]/g, "-");
        const directUrl = `${BASE_URL}/Drama/${slug}/Episode-${epNum}?id=${show.id}&ep=${episodeId}&page=0&pageSize=100`;
        console.log(`[KissKH] Navigate: ${directUrl}`);

        page = await require("../utils/browserManager").getNewPage();
        context = page.context();

        const capturedUrls = { video: new Set(), subtitles: new Set() };
        
        page.on("request", (request) => {
            const url = request.url();
            if ((url.includes(".m3u8") || url.includes(".mp4") || url.includes(".ts") || url.includes("streamcdn") || url.includes("lovestreaming") || url.includes("index.m3u8")) && !url.includes("blob:")) {
                capturedUrls.video.add(url);
            }
            if (url.includes(".vtt") || url.includes(".srt")) {
                capturedUrls.subtitles.add(url);
            }
        });

        page.on("response", async (response) => {
            const url = response.url();
            if (url.includes("/api/DramaList/Episode/") && url.includes(".png")) {
                try {
                    const data = await response.json();
                    if (data && data.Video) {
                        capturedUrls.video.add(data.Video);
                    }
                    if (data && data.ThirdParty) {
                        capturedUrls.video.add(data.ThirdParty);
                    }
                } catch (e) {
                    // Ignore parsing errors
                }
            }
        });

        await page.goto(directUrl, { waitUntil: "domcontentloaded", timeout: 30000 });
        
        // Wait for player/video or some activity
        try {
            await page.waitForSelector('video', { timeout: 5000 });
        } catch (e) {
            // Ignore if video doesn't appear fast
        }
        
        await page.evaluate(() => {
            const v = document.querySelector('video');
            if (v) v.play().catch(() => {});
        });

        // Wait a bit for the requests to fire
        await new Promise(r => setTimeout(r, 2000));

        const subtitles = Array.from(capturedUrls.subtitles).map(url => ({ 
            url, 
            lang: "en", 
            label: "English"
        }));

        for (const url of capturedUrls.video) {
            if (url.endsWith(".ts") && Array.from(capturedUrls.video).some(u => u.includes(".m3u8"))) {
                continue;
            }

            const isThirdParty = url.includes("sbplay") || url.includes("ok.ru") || url.includes("vidoza");

            streams.push({
                url,
                quality: isThirdParty ? "ThirdParty" : "Auto",
                provider: "KissKH",
                subtitles: subtitles.length > 0 ? subtitles : undefined,
                headers: {
                    "Referer": BASE_URL,
                    "Origin": BASE_URL,
                    "User-Agent": USER_AGENT
                }
            });
        }
        
        console.log(`[KissKH] Found ${streams.length} streams.`);

    } catch (error) {
        console.error(`[KissKH] Error: ${error.message}`);
    } finally {
        if (context) await context.close();
    }
    return streams;
}

module.exports = { getStreams: getKissKhStreams };