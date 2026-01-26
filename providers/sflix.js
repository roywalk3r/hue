const cheerio = require("cheerio");
const axios = require("axios");
const { getNewContext } = require("../utils/browserManager");

const BASE_URL = "https://sflix.film";
const API_BASE = "https://h5-api.aoneroom.com/wefeed-h5api-bff";
const USER_AGENT = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

const axiosInstance = axios.create({
    headers: { "User-Agent": USER_AGENT },
    timeout: 15000
});

async function getTmdbInfo(tmdbId, mediaType) {
    if (!tmdbId) return { title: null, year: null };
    const { getTmdbApiKey } = require("../utils/tmdbKey");
    const TMDB_API_KEY = getTmdbApiKey();
    if (!TMDB_API_KEY) return { title: null, year: null };

    const type = mediaType === 'tv' ? 'tv' : 'movie';
    const url = `https://api.themoviedb.org/3/${type}/${tmdbId}?api_key=${TMDB_API_KEY}`;
    try {
        const response = await axiosInstance.get(url);
        const data = response.data;
        const releaseDate = data.release_date || data.first_air_date;
        const year = releaseDate ? parseInt(releaseDate.split("-")[0], 10) : null;
        const title = data.title || data.name;
        return { title, year: isNaN(year) ? null : year };
    } catch (e) {
        return { title: null, year: null };
    }
}

async function searchSflixViaApi(keyword) {
    try {
        const url = `${API_BASE}/subject/search`;
        const payload = {
            keyword: keyword,
            page: "1",
            perPage: 24,
            subjectType: 0
        };
        const response = await axiosInstance.post(url, payload);
        return response.data?.data?.items || [];
    } catch (e) {
        return [];
    }
}

async function searchSflix(title, mediaType, year, season) {
    let keyword = title;
    if (mediaType === "movie" && year) {
        keyword = `${title} ${year}`;
    }
    
    try {
        const items = await searchSflixViaApi(keyword);
        const normalize = (str) => str.toLowerCase().replace(/[^a-z0-9]/g, "");
        const target = normalize(title);
        const targetSeason = season ? parseInt(season) : null;
        
        let bestMatch = { item: null, score: -1 };

        items.forEach(item => {
            const text = item.title || "";
            const itemYearMatch = item.releaseDate ? item.releaseDate.match(/\d{4}/) : null;
            const itemYear = itemYearMatch ? itemYearMatch[0] : null;
            
            const cleanText = text.replace(/S\d+(-S\d+)?|Season \d+|\(\d{4}\)/gi, "").trim();
            const normClean = normalize(cleanText);
            
            let currentScore = 0;
            if (normClean === target) currentScore += 1000;
            else if (normClean.includes(target)) currentScore += 50;

            if (mediaType === "tv" && targetSeason) {
                const specificSeasonRegex = new RegExp(`S(eason)?\\s*${targetSeason}\b`, 'i');
                if (specificSeasonRegex.test(text)) currentScore += 100;
            }

            if (year && (text.includes(year.toString()) || itemYear === year.toString())) currentScore += 30;

            if (mediaType === "tv") currentScore += (item.subjectType === 2) ? 50 : -100;
            else currentScore += (item.subjectType === 1) ? 50 : -100;

            if (currentScore > bestMatch.score) bestMatch = { item, score: currentScore };
        });

        if (bestMatch.item) {
            console.log(`[SFlix API] Found match: ${bestMatch.item.title} (ID: ${bestMatch.item.subjectId})`);
            return { 
                slug: bestMatch.item.detailPath, 
                subjectId: bestMatch.item.subjectId,
                detailUrl: `${BASE_URL}/detail/${bestMatch.item.detailPath}`
            };
        }

        return null;
    } catch (e) {
        return null;
    }
}

async function getCaptionsViaApi(subjectId, streamId, detailPath) {
    try {
        const url = `${API_BASE}/subject/caption?format=MP4&id=${streamId}&subjectId=${subjectId}&detailPath=${detailPath}`;
        const response = await axiosInstance.get(url);
        const captions = response.data?.data?.captions;
        if (!Array.isArray(captions)) return [];
        
        return captions.map(c => ({
            url: c.url,
            lang: c.lan || 'en',
            label: c.lanName || 'English'
        }));
    } catch (e) {
        return [];
    }
}

async function getStreamsViaApi(slug, subjectId, season, episode) {
    try {
        const se = season || 1;
        const ep = episode || 1;
        const playUrl = `${BASE_URL}/wefeed-h5api-bff/subject/play?subjectId=${subjectId}&se=${se}&ep=${ep}&detailPath=${slug}`;
        
        const playResp = await axiosInstance.get(playUrl, {
            headers: {
                "Referer": `${BASE_URL}/detail/${slug}`,
                "User-Agent": USER_AGENT,
                "Accept": "application/json"
            }
        });
        
        const data = playResp.data?.data;
        if (!data || (!data.streams?.length && !data.hls?.length)) return null;

        const streams = [];
        
        if (Array.isArray(data.streams) && data.streams.length > 0) {
            const captions = await getCaptionsViaApi(subjectId, data.streams[0].id, slug);
            data.streams.forEach(s => {
                streams.push({
                    title: `Nova - ${s.resolutions}p (MP4)`,
                    url: s.url,
                    quality: `${s.resolutions}p`,
                    format: 'mp4',
                    provider: 'Nova',
                    subtitles: captions.length > 0 ? captions : undefined,
                    headers: { "Referer": `${BASE_URL}/`, "User-Agent": USER_AGENT }
                });
            });
        }
        
        if (Array.isArray(data.hls)) {
            data.hls.forEach(s => {
                streams.push({
                    title: `Nova - Auto (HLS)`,
                    url: s.url,
                    quality: 'Auto',
                    format: 'hls',
                    provider: 'Nova',
                    headers: { "Referer": `${BASE_URL}/`, "User-Agent": USER_AGENT }
                });
            });
        }
        return streams.length > 0 ? streams : null;
    } catch (e) {
        return null;
    }
}

async function triggerPlayback(page, label) {
    if (page.isClosed()) return;
    try {
        await page.evaluate(() => {
            const v = document.querySelector('video');
            if (v) v.play().catch(() => {});
            ['.art-state', '.art-icon-play', '.art-video-player', 'button[aria-label="Play"]', '.jw-icon-playback', '.watch-btn'].forEach(s => {
                const el = document.querySelector(s);
                if (el) el.click();
            });
        }).catch(() => {});
    } catch (err) {}
}

async function getSflixStreams(tmdbId, mediaType, season, episode, directTitle = null) {
    console.log(`[SFlix] Processing TMDB:${tmdbId || 'N/A'} Title:${directTitle || 'N/A'} (${mediaType})`);
    try {
        let title = directTitle;
        let year = null;

        if (tmdbId) {
            const info = await getTmdbInfo(tmdbId, mediaType);
            title = info.title;
            year = info.year;
        }

        if (!title) return [];

        const match = await searchSflix(title, mediaType, year, season);
        if (!match) return [];

        // 1. FAST PATH (API)
        const apiStreams = await getStreamsViaApi(match.slug, match.subjectId, season, episode);
        if (apiStreams) return apiStreams;

        // 2. SLOW PATH (Browser Fallback)
        console.log(`[SFlix] API failed, using browser fallback for ${match.slug}`);
        let context;
        let page;
        try {
            context = await getNewContext();
            page = await context.newPage();
            const capturedUrls = { video: new Set(), subtitles: new Set() };
            
            page.on("request", (request) => {
                const url = request.url();
                const isVideo = (url.includes(".mp4") || url.includes(".m3u8") || url.includes("hakunaymatata.com") || url.includes("lightningbolt.site") || url.includes("vidbox.site")) 
                                && !url.includes("/subtitle/") && !url.includes(".vtt") && !url.includes(".srt");
                if (isVideo && !url.includes("image/") && !url.includes(".js") && !url.includes(".css")) capturedUrls.video.add(url);
                if (url.includes(".vtt") || url.includes(".srt") || url.includes("/subtitle/")) capturedUrls.subtitles.add(url);
            });

            await page.goto(match.detailUrl, { waitUntil: "domcontentloaded", timeout: 30000 });
            await page.evaluate(() => document.querySelector('.watch-btn')?.click());
            
            if (mediaType === "tv") {
                await page.evaluate(async ({ s, e }) => {
                    const sleep = (ms) => new Promise(r => setTimeout(r, ms));
                    let seasonBox = document.querySelector('.season-box, .season-box2, .ss-box');
                    if (seasonBox) {
                        const current = seasonBox.textContent || "";
                        if (!current.includes(`Season ${s}`)) {
                            seasonBox.click();
                            await sleep(1000);
                            const target = Array.from(document.querySelectorAll('.choice-season, .season-item')).find(i => i.textContent.includes(`Season ${s}`));
                            if (target) { target.click(); await sleep(2000); }
                        }
                    }
                    const targetEp = Array.from(document.querySelectorAll('.ep-item, .eps-item')).find(i => i.textContent.trim() === e.toString() || i.textContent.trim() === e.toString().padStart(2, '0'));
                    if (targetEp) { targetEp.click(); await sleep(1500); }
                }, { s: season || 1, e: episode || 1 }).catch(() => {});
            }

            await triggerPlayback(page, "fallback");
            const start = Date.now();
            while (Date.now() - start < 10000) {
                if (capturedUrls.video.size > 0) break;
                await new Promise(r => setTimeout(r, 500));
            }

            const streams = [];
            const subtitles = Array.from(capturedUrls.subtitles).map(url => ({ url, lang: "en", label: "English" }));
            Array.from(capturedUrls.video).forEach(url => {
                const isHls = url.includes('.m3u8');
                streams.push({
                    url,
                    title: `Nova - ${isHls ? 'HLS' : 'MP4'} (Fallback)`,
                    quality: "Auto", 
                    provider: "Nova",
                    format: isHls ? 'hls' : 'mp4',
                    subtitles: subtitles.length > 0 ? subtitles : undefined,
                    headers: { "Referer": `${BASE_URL}/`, "User-Agent": USER_AGENT }
                });
            });
            return streams;
        } finally {
            if (context) await context.close();
        }
    } catch (error) {
        console.error(`[SFlix] Error: ${error.message}`);
        return [];
    }
}

async function getStreamsDirect(title, episode, season = 1) {
    const mediaType = (season || episode) ? "tv" : "movie";
    return await getSflixStreams(null, mediaType, season, episode, title);
}

module.exports = { getStreams: getSflixStreams, getStreamsDirect };