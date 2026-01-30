const cheerio = require("cheerio");
const axios = require("axios");
const http = require("http");
const https = require("https");
const { getNewContext } = require("../utils/browserManager");
const { config } = require("../utils/config");

const BASE_URL = "https://sflix.film";
const API_BASE = "https://h5-api.aoneroom.com/wefeed-h5api-bff";
const DEFAULT_USER_AGENT = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";
const USER_AGENT = config.sflixUserAgent || DEFAULT_USER_AGENT;

// Default fallback cookie (Expires around Feb 2026 based on JWT, but session might expire sooner)
const DEFAULT_COOKIE = "sflix_token=%22eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJ1aWQiOjMxMTYwODUxODIwODQ5MjQ1MTIsImF0cCI6MywiZXh0IjoiMTc2ODA4NTkyNCIsImV4cCI6MTc3NTg2MTkyNCwiaWF0IjoxNzY4MDg1NjI0fQ.K6fuHWpTcLP0WjQx9ZFe6LSo_oNVNKiuC3MOA2HRw1s%22; account=2904345630854337384|0|H5|1768086238|; uuid=181f2237-fb79-4245-9a8a-2d5f0a60c4aa; sflix_i18n_lang=en";

const axiosInstance = axios.create({
    headers: { 
        "User-Agent": USER_AGENT,
        "Cookie": config.sflixCookie || DEFAULT_COOKIE
    },
    timeout: 15000,
    httpAgent: new http.Agent({ family: 4, keepAlive: false }),
    httpsAgent: new https.Agent({ family: 4, keepAlive: false })
});

const tmdbAxios = axios.create({
    timeout: 10000,
    httpAgent: new http.Agent({ family: 4, keepAlive: true }),
    httpsAgent: new https.Agent({ family: 4, keepAlive: true })
});

async function getTmdbInfo(tmdbId, mediaType) {
    if (!tmdbId) return { title: null, year: null };
    const { getTmdbApiKey } = require("../utils/tmdbKey");
    const TMDB_API_KEY = getTmdbApiKey();
    if (!TMDB_API_KEY) return { title: null, year: null };

    const type = mediaType === 'tv' ? 'tv' : 'movie';
    const url = `https://api.themoviedb.org/3/${type}/${tmdbId}?api_key=${TMDB_API_KEY}`;
    try {
        const response = await tmdbAxios.get(url);
        const data = response.data;
        const releaseDate = data.release_date || data.first_air_date;
        const year = releaseDate ? parseInt(releaseDate.split("-")[0], 10) : null;
        const title = data.title || data.name;
        return { title, year: isNaN(year) ? null : year };
    } catch (e) {
        return { title: null, year: null };
    }
}

async function searchSflixViaApi(keyword, mediaType) {
    try {
        const url = `${API_BASE}/subject/search`;
        // subjectType: 0 = All, 1 = Movies, 2 = Series
        const subjectType = mediaType === 'tv' ? 2 : 1;
        
        const payload = {
            keyword: keyword,
            page: "1",
            perPage: 24,
            subjectType: subjectType
        };
        console.log(`[SFlix API] POST ${url}`, payload);
        const response = await axiosInstance.post(url, payload, {
            headers: {
                'Accept': 'application/json',
                'Content-Type': 'application/json',
                'Referer': 'https://sflix.film/'
            }
        });
        console.log(`[SFlix API] Search Status: ${response.status} (Found: ${response.data?.data?.items?.length || 0})`);
        return response.data?.data?.items || [];
    } catch (e) {
        console.warn(`[SFlix API] Search failed: ${e.message}`);
        return [];
    }
}

async function searchSflix(title, mediaType, year, season) {
    let keyword = title;
    if (mediaType === "movie" && year) {
        keyword = `${title} ${year}`;
    }
    
    try {
        const items = await searchSflixViaApi(keyword, mediaType);
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
            
            // Strict Year Matching
            if (year && itemYear) {
                const diff = Math.abs(parseInt(itemYear) - parseInt(year));
                if (diff === 0) {
                    currentScore += 500; // Exact match
                } else if (diff <= 1) {
                    currentScore += 200; // Close enough
                } else {
                    currentScore -= 5000; // Wrong era -> Reject
                }
            } else if (year && !itemYear) {
                currentScore -= 50; 
            }

            if (normClean === target) currentScore += 1000;
            else if (normClean.includes(target)) currentScore += 50;
            else if (target.includes(normClean)) currentScore += 50;

            if (mediaType === "tv" && targetSeason) {
                const specificSeasonRegex = new RegExp(`S(eason)?\s*${targetSeason}\b`, 'i');
                if (specificSeasonRegex.test(text)) currentScore += 100;
            }

            if (year && (text.includes(year.toString()) || itemYear === year.toString())) currentScore += 30;

            if (mediaType === "tv") currentScore += (item.subjectType === 2) ? 50 : -100;
            else currentScore += (item.subjectType === 1) ? 50 : -100;

            if (currentScore > bestMatch.score) bestMatch = { item, score: currentScore };
        });

        if (bestMatch.item && bestMatch.score > 0) {
            console.log(`[SFlix API] Found match: ${bestMatch.item.title} (ID: ${bestMatch.item.subjectId}) type=${bestMatch.item.subjectType}`);
            return { 
                slug: bestMatch.item.detailPath, 
                subjectId: bestMatch.item.subjectId,
                subjectType: bestMatch.item.subjectType,
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
        const se = season || 0;
        const ep = episode || 0;
        
        // Force 'movies' structure as seen in working curl script
        const typePath = 'movies'; 
        const typeQuery = '/movie/detail';
        
        const refererUrl = `${BASE_URL}/spa/videoPlayPage/${typePath}/${slug}?id=${subjectId}&type=${encodeURIComponent(typeQuery)}&lang=en`;

        // Play API call
        const playUrl = `${BASE_URL}/wefeed-h5api-bff/subject/play?subjectId=${subjectId}&se=${se}&ep=${ep}&detailPath=${slug}`;
        console.log(`[SFlix API] Calling Play: ${playUrl}`);
        
        const playResp = await axiosInstance.get(playUrl, {
            headers: {
                "Referer": refererUrl, 
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
        return streams;
    } catch (e) {
        return [];
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

        // 1. Determine effective parameters based on matched type
        const isMovie = match.subjectType === 1;
        const effectiveSeason = isMovie ? 0 : (season || 1);
        const effectiveEpisode = isMovie ? 0 : (episode || 1);

        // 2. FAST PATH (API)
        const apiStreams = await getStreamsViaApi(match.slug, match.subjectId, effectiveSeason, effectiveEpisode);
        if (apiStreams && apiStreams.length > 0) return apiStreams;

        // 3. SLOW PATH (Browser Fallback)
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
                if (isVideo && !url.includes("image/") && !url.includes("banner") && !url.includes("logo") && !url.includes(".js") && !url.includes(".css")) capturedUrls.video.add(url);
                if (url.includes(".vtt") || url.includes(".srt") || url.includes("/subtitle/")) capturedUrls.subtitles.add(url);
            });

            await page.goto(match.detailUrl, { waitUntil: "domcontentloaded", timeout: 30000 });
            await page.evaluate(() => document.querySelector('.watch-btn')?.click());
            
            if (!isMovie) {
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
                }, { s: effectiveSeason, e: effectiveEpisode }).catch(() => {});
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

// Add getStreamsBySlug alias to use the API path directly if we have the slug
async function getStreamsBySlug(id, slug, episode) {
    // SFlix/HiTV share IDs and slugs. If we have the slug, we can use the API path.
    // However, getStreamsViaApi expects (slug, subjectId, season, episode).
    // HiTV "id" IS the "subjectId".
    // HiTV shows are usually Season 1 unless specified otherwise in title, but API takes 'se'.
    // We'll assume Season 1 for now or parse it if passed.
    const streams = await getStreamsViaApi(slug, id, 1, episode);
    return streams || [];
}

module.exports = { getStreams: getSflixStreams, getStreamsDirect, getStreamsBySlug };
