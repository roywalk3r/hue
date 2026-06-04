/**
 * Hue Stream API
 * Professional Streaming Aggregator
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */

const axios = require("axios");
const { chromium } = require("playwright");
const { getTmdbInfo } = require("../utils/tmdb");

const BASE_URL = "https://kissasian.cam";
const USER_AGENT = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

function normalize(str) {
    return str.toLowerCase().replace(/[^a-z0-9]/g, "");
}

async function searchKissAsian(query) {
    try {
        const url = `${BASE_URL}/?s=${encodeURIComponent(query)}`;
        const { data } = await axios.get(url, {
            headers: { "User-Agent": USER_AGENT },
            timeout: 15000
        });

        const articles = data.match(/<article[\s\S]*?<\/article>/g) || [];
        const results = [];

        for (const article of articles) {
            const linkMatch = article.match(/href="(https:\/\/kissasian\.cam\/series\/([^"\/]+)\/)"/);
            const titleMatch = article.match(/<h[1-6][^>]*>([^<]+)<\/h[1-6]>/) || article.match(/title="([^"]+)"/);

            if (linkMatch && titleMatch) {
                results.push({
                    id: linkMatch[2],
                    title: titleMatch[1].trim(),
                    slug: linkMatch[2],
                    url: linkMatch[1]
                });
            }
        }

        return results;
    } catch (e) {
        console.error(`[Pearl] Search error: ${e.message}`);
        return [];
    }
}

async function getEpisodes(slug) {
    try {
        const url = `${BASE_URL}/series/${slug}/`;
        const { data } = await axios.get(url, {
            headers: { "User-Agent": USER_AGENT },
            timeout: 15000
        });

        // Extract all episode links, then filter to those matching this series slug
        const regex = /href="(https:\/\/kissasian\.cam\/([^"]*episode-\d+[^"]*)\/)"/g;
        const matches = [...data.matchAll(regex)];

        const episodes = [];
        const seen = new Set();

        for (const match of matches) {
            const epUrl = match[1];
            const path = match[2];

            if (seen.has(epUrl)) continue;
            seen.add(epUrl);

            // Only include URLs that contain the series slug
            if (!path.includes(slug)) continue;

            const numMatch = path.match(/episode-(\d+)/);
            const number = numMatch ? parseInt(numMatch[1]) : 0;
            if (number > 0) {
                episodes.push({ number, url: epUrl });
            }
        }

        episodes.sort((a, b) => a.number - b.number);
        return episodes;
    } catch (e) {
        console.error(`[Pearl] Episode list error: ${e.message}`);
        return [];
    }
}

async function extractStreams(url) {
    console.log(`[Pearl] Extracting from: ${url}`);
    const streams = [];
    let browser;

    try {
        browser = await chromium.launch({ headless: true });
        const context = await browser.newContext({
            userAgent: USER_AGENT,
            viewport: { width: 1280, height: 720 }
        });
        const page = await context.newPage();
        const captured = new Set();
        const subtitles = [];

        page.on("request", (req) => {
            const u = req.url();
            if (u.includes(".m3u8") && !u.includes("google") && !u.includes("analytics") && !u.includes("blob:")) {
                captured.add(u);
            }
            if ((u.includes(".vtt") || u.includes(".srt")) && !subtitles.some(s => s.url === u)) {
                subtitles.push({ url: u, lang: "en", label: "English" });
            }
        });

        await page.goto(url, { waitUntil: "networkidle", timeout: 30000 });
        await new Promise(r => setTimeout(r, 5000));

        const frames = page.frames();
        const videoFrame = frames.find(f =>
            f.url().includes("rupertisdivingintoocean.com") ||
            f.url().includes("justplay.cam") ||
            f.url().includes("sprintcdn")
        );

        if (videoFrame) {
            try {
                await videoFrame.evaluate(() => {
                    const triggers = ["video", ".vjs-big-play-button", ".play-btn", ".jw-display-icon-container", "#pl_but"];
                    triggers.forEach(s => {
                        const el = document.querySelector(s);
                        if (el) { try { el.click(); } catch (e) {} }
                    });
                    const v = document.querySelector("video");
                    if (v) v.play().catch(() => {});
                });
            } catch {}
        }

        await new Promise(r => setTimeout(r, 8000));

        for (const u of captured) {
            if (!streams.some(s => s.url === u)) {
                streams.push({
                    url: u,
                    title: "Pearl - Auto",
                    quality: "Auto",
                    provider: "Pearl",
                    format: "hls",
                    subtitles: subtitles.length > 0 ? subtitles : undefined,
                    headers: {
                        "Referer": BASE_URL + "/",
                        "User-Agent": USER_AGENT
                    }
                });
            }
        }

        return streams;
    } catch (e) {
        console.error(`[Pearl] Extraction error: ${e.message}`);
    } finally {
        if (browser) await browser.close();
    }

    return streams;
}

async function getStreams(tmdbId, mediaType, season, episode) {
    if (mediaType === "movie") return [];

    try {
        const { title: baseTitle, year: releaseYear } = await getTmdbInfo(tmdbId, mediaType);
        if (!baseTitle) return [];

        let searchTitle = baseTitle;
        if (season && season > 1) {
            searchTitle = `${baseTitle} Season ${season}`;
        }

        console.log(`[Pearl] Searching: ${searchTitle} (${releaseYear || "no year"})`);
        let results = await searchKissAsian(searchTitle);

        if (results.length === 0 && season && season > 1) {
            console.log(`[Pearl] Fallback search: ${baseTitle}`);
            results = await searchKissAsian(baseTitle);
        }

        if (results.length === 0) {
            console.log(`[Pearl] No results for: ${searchTitle}`);
            return [];
        }

        const targetNorm = normalize(baseTitle);
        let bestMatch = results.find(r => {
            const rNorm = normalize(r.title);
            return rNorm.includes(targetNorm) || targetNorm.includes(rNorm);
        });

        if (!bestMatch) {
            bestMatch = results[0];
        }

        console.log(`[Pearl] Using series: ${bestMatch.title} (${bestMatch.slug})`);
        const episodes = await getEpisodes(bestMatch.slug);

        const targetEp = episode || 1;
        const ep = episodes.find(e => e.number === targetEp);

        if (!ep) {
            console.log(`[Pearl] Episode ${targetEp} not found (${episodes.length} episodes available)`);
            return [];
        }

        return await extractStreams(ep.url);
    } catch (e) {
        console.error(`[Pearl] Provider error: ${e.message}`);
        return [];
    }
}

async function getStreamsByUrl(url) {
    console.log(`[Pearl] Fetching streams for URL: ${url}`);
    return await extractStreams(url);
}

module.exports = {
    getStreams,
    getStreamsByUrl
};
