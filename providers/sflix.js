const cheerio = require("cheerio");
const axios = require("axios");
const { getNewPage, getNewContext } = require("../utils/browserManager");

const BASE_URL = "https://sflix.film";
const API_BASE = "https://h5-api.aoneroom.com/wefeed-h5api-bff";
const USER_AGENT = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

const axiosInstance = axios.create({
    headers: { "User-Agent": USER_AGENT },
    timeout: 15000
});

async function getTmdbInfo(tmdbId, mediaType) {
    const { getTmdbApiKey } = require("../utils/tmdbKey");
    const TMDB_API_KEY = getTmdbApiKey();
    if (!TMDB_API_KEY) return { title: null, year: null };

    const url = `https://api.themoviedb.org/3/${mediaType}/${tmdbId}?api_key=${TMDB_API_KEY}`;
    try {
        const response = await axiosInstance.get(url);
        const data = response.data;
        const releaseDate = data.release_date || data.first_air_date;
        const year = releaseDate ? parseInt(releaseDate.split("-")[0], 10) : null;
        const title = data.title || data.name;
        return { title, year: isNaN(year) ? null : year };
    } catch (e) {
        console.error(`[SFlix] TMDB lookup failed for ${tmdbId}: ${e.message}`);
        return { title: null, year: null };
    }
}

async function searchSflix(title, mediaType, year, season) {
    let keyword = title;
    if (mediaType === "movie" && year) {
        keyword = `${title} ${year}`;
    }
    
    const searchUrl = `${BASE_URL}/search-result?keyword=${encodeURIComponent(keyword)}`;
    console.log(`[SFlix] Searching (${mediaType}) with keyword "${keyword}": ${searchUrl}`);
    
    try {
        const response = await axiosInstance.get(searchUrl);
        const $ = cheerio.load(response.data);

        const normalize = (str) => str.toLowerCase().replace(/[^a-z0-9]/g, "");
        const target = normalize(title);
        const targetSeason = season ? parseInt(season) : null;
        
        const items = $(".subject-card");
        let bestMatch = { href: null, score: -1 };

        items.each((i, el) => {
            const item = $(el);
            const titleEl = item.find(".subject-card-bottom .line-1").length ? item.find(".subject-card-bottom .line-1") : item.find("h2.line-1");
            const href = item.attr("href");
            const metaEl = item.find(".subject-card-bottom .line-2");
            const metaText = metaEl.text().trim().toLowerCase();
            const itemYearMatch = metaText.match(/\d{4}/);
            const itemYear = itemYearMatch ? itemYearMatch[0] : null;
            
            if (titleEl.length && href) {
                const text = titleEl.text().trim();
                const cleanText = text.replace(/S\d+(-S\d+)?|Season \d+|\(\d{4}\)/gi, "").trim();
                const normClean = normalize(cleanText);
                
                let currentScore = 0;
                if (normClean === target) currentScore += 1000;
                else if (normClean.includes(target)) currentScore += 50;

                if (mediaType === "tv" && targetSeason) {
                    const specificSeasonRegex = new RegExp(`S(eason)?\\s*${targetSeason}\b`, 'i');
                    if (specificSeasonRegex.test(text)) currentScore += 100;
                    else {
                        const rangeMatch = text.match(/S(\d+)-S(\d+)/i);
                        if (rangeMatch) {
                            const start = parseInt(rangeMatch[1]);
                            const end = parseInt(rangeMatch[2]);
                            if (targetSeason >= start && targetSeason <= end) currentScore += 80;
                        }
                    }
                }

                if (year && (text.includes(year.toString()) || itemYear === year.toString())) currentScore += 30;

                const isTVResult = metaText.includes("series") || metaText.includes("season") || metaText.includes("ss") || text.match(/S\d+/i);
                if (mediaType === "tv") currentScore += isTVResult ? 50 : -50;
                else currentScore += isTVResult ? -100 : 20;

                if (currentScore > bestMatch.score) bestMatch = { href, score: currentScore };
            }
        });

        return bestMatch.href ? (bestMatch.href.startsWith("http") ? bestMatch.href : `${BASE_URL}${bestMatch.href}`) : null;
    } catch (e) {
        console.error(`[SFlix] Search error: ${e.message}`);
        return null;
    }
}

async function getStreamsViaApi(slug, season, episode) {
    try {
        // Detail API call
        const detailUrl = `${API_BASE}/detail?detailPath=${slug}`;
        console.log(`[SFlix API] Calling Detail: ${detailUrl}`);
        const detailResp = await axiosInstance.get(detailUrl);
        const subjectId = detailResp.data?.data?.subject?.subjectId;
        if (!subjectId) {
            console.warn('[SFlix API] Subject ID not found in detail response.');
            return null;
        }
        
        const se = season || 1;
        const ep = episode || 1;
        
        // Play API call
        const playUrl = `${BASE_URL}/wefeed-h5api-bff/subject/play?subjectId=${subjectId}&se=${se}&ep=${ep}&detailPath=${slug}`;
        console.log(`[SFlix API] Calling Play: ${playUrl}`);
        
        const playResp = await axiosInstance.get(playUrl, {
            headers: {
                "Referer": `${BASE_URL}/detail/${slug}`, 
                "User-Agent": USER_AGENT 
            }
        });
        
        const data = playResp.data?.data;
        if (!data || (!data.streams?.length && !data.hls?.length)) {
            console.warn('[SFlix API] Play response has no streams.');
            return null;
        }

        const streams = [];
        // Process MP4 streams
        if (Array.isArray(data.streams)) {
            data.streams.forEach(s => {
                streams.push({
                    title: `Nova - ${s.resolutions}p (MP4)`,
                    url: s.url,
                    quality: `${s.resolutions}p`,
                    format: 'mp4',
                    provider: 'Nova',
                    headers: { "Referer": `${BASE_URL}/`, "User-Agent": USER_AGENT }
                });
            });
        }
        // Process HLS streams if any
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
        console.warn(`[SFlix API] Path failed: ${e.message}`);
        return null;
    }
}

async function getMediaInfo(detailUrl, title) {
    try {
        const response = await axiosInstance.get(detailUrl);
        const html = response.data;
        const idMatch = html.match(/"id"\s*:\s*"?(\d{15,})"?/) || html.match(/"subjectId":\s*"?(\d{18,20})"?/);
        const id = idMatch ? idMatch[1] : null;
        const slug = detailUrl.split('/').pop() || null;
        return { id, slug };
    } catch { return { id: null, slug: null }; }
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
        console.log(`[SFlix] Triggered playback (${label})`);
    } catch (err) {}
}

async function getSflixStreams(tmdbId, mediaType, season, episode) {
    console.log(`[SFlix] Fetching streams for TMDB ID: ${tmdbId} (${mediaType})`);
    try {
        const { title, year } = await getTmdbInfo(tmdbId, mediaType);
        if (!title) return [];

        const detailUrl = await searchSflix(title, mediaType, year, season);
        if (!detailUrl) return [];
        const slug = detailUrl.split('/').pop();

        // 1. Try FAST PATH (API)
        const apiStreams = await getStreamsViaApi(slug, season, episode);
        if (apiStreams) {
            console.log(`[SFlix] API Path Success! Found ${apiStreams.length} streams.`);
            return apiStreams;
        }

        // 2. Fallback to SLOW PATH (Browser)
        console.log(`[SFlix] API Path failed or returned no streams. Falling back to Browser...`);
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

            console.log(`[SFlix] Navigating to detail page: ${detailUrl}`);
            await page.goto(detailUrl, { waitUntil: "domcontentloaded", timeout: 30000 });
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
