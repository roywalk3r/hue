const cheerio = require("cheerio");
const axios = require("axios");
const { getNewContext } = require("../utils/browserManager");

const BASE_URL = "https://sflix.film";
const USER_AGENT = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

async function getTmdbInfo(tmdbId, mediaType) {
    const { getTmdbApiKey } = require("../utils/tmdbKey");
    const TMDB_API_KEY = getTmdbApiKey();
    if (!TMDB_API_KEY) return { title: null, year: null };

    const url = `https://api.themoviedb.org/3/${mediaType}/${tmdbId}?api_key=${TMDB_API_KEY}`;
    const response = await axios.get(url);
    const data = response.data;
    const releaseDate = data.release_date || data.first_air_date;
    const year = releaseDate ? parseInt(releaseDate.split("-")[0], 10) : null;
    const title = data.title || data.name;
    return { title, year: isNaN(year) ? null : year };
}

async function searchSflix(title, mediaType, year, season) {
    const keyword = (mediaType === "tv" && season) ? `${title} Season ${season}` : title;
    const searchUrl = `${BASE_URL}/search-result?keyword=${encodeURIComponent(keyword)}`;
    console.log(`[SFlix] Searching (${mediaType}): ${searchUrl}`);
    
    try {
        const response = await axios.get(searchUrl, {
            headers: { "User-Agent": USER_AGENT }
        });
        const $ = cheerio.load(response.data);

        const normalize = (str) => str.toLowerCase().replace(/[^a-z0-9]/g, "");
        const target = normalize(title);
        const targetSeason = season ? normalize(`s${season}`) : "";
        
        const items = $(".subject-card");
        let bestMatch = { href: null, score: -1 };

        items.each((i, el) => {
            const item = $(el);
            const titleEl = item.find(".subject-card-bottom .line-1").length ? item.find(".subject-card-bottom .line-1") : item.find("h2.line-1");
            const href = item.attr("href");
            
            if (titleEl.length && href) {
                const text = titleEl.text().trim();
                const normalizedText = normalize(text);
                let currentScore = 0;

                if (normalizedText === target) currentScore += 100;
                else if (normalizedText.includes(target)) currentScore += 50;
                
                if (season && (normalizedText.includes(target + targetSeason) || normalizedText.includes(target + "season" + season))) {
                    currentScore += 150;
                }

                if (year && text.includes(year.toString())) currentScore += 80;

                if (mediaType === "tv") {
                    if (!normalizedText.includes("s1") && !normalizedText.includes("season")) currentScore += 10;
                } else {
                    if (normalizedText.includes("s1") || normalizedText.includes("season")) currentScore -= 150;
                }

                if (currentScore > bestMatch.score) {
                    bestMatch = { href, score: currentScore };
                }
            }
        });

        return bestMatch.href ? (bestMatch.href.startsWith("http") ? bestMatch.href : `${BASE_URL}${bestMatch.href}`) : null;
    } catch (e) {
        console.error(`[SFlix] Search error: ${e.message}`);
        return null;
    }
}

async function getMediaInfo(detailUrl, title) {
    try {
        const response = await axios.get(detailUrl, { headers: { "User-Agent": USER_AGENT } });
        const html = response.data;
        
        let id = null;

        const match1 = html.match(/"id"\s*:\s*"?(\d{15,})"?/) || html.match(/id\s*=\s*"?(\d{15,})"?/);
        if (match1) id = match1[1];

        if (!id) {
            const safeTitle = title.replace(/[.*+?^${}()|[\\]/g, '\\$&');
            const regex = new RegExp(`"(\d{15,})",\d+,\"${safeTitle}"`);
            const match2 = html.match(regex);
            if (match2) id = match2[1];
        }

        if (!id) {
             const match3 = html.match(/"(\d{15,})"/);
             if (match3) id = match3[1];
        }
        
        const slug = detailUrl.split('/').pop() || null;
        
        return { id, slug };
    } catch {
        return { id: null, slug: null };
    }
}

async function triggerPlayback(page, label) {
    if (page.isClosed()) return;
    try {
        await page.evaluate(() => {
            const video = document.querySelector('video');
            if (video) video.play().catch(() => {});
            const selectors = ['.art-state', '.art-icon-play', '.art-video-player', 'button[aria-label="Play"]', '.jw-icon-playback', '.watch-btn'];
            for (const s of selectors) {
                const el = document.querySelector(s);
                if (el) el.click();
            }
        }).catch(() => {});
        console.log(`[SFlix] Triggered playback (${label})`);
    } catch (err) {}
}

async function getSflixStreams(tmdbId, mediaType, season, episode) {
    console.log(`[SFlix] Fetching streams for TMDB ID: ${tmdbId} (${mediaType})`);
    const streams = [];
    let context;
    let page;

    try {
        const { title, year } = await getTmdbInfo(tmdbId, mediaType);
        if (!title) return [];
        console.log(`[SFlix] Resolved title: ${title} (${year})`);

        const detailUrl = await searchSflix(title, mediaType, year, season);
        if (!detailUrl) return [];

        const { id, slug } = await getMediaInfo(detailUrl, title);
        console.log(`[SFlix] Extracted ID: ${id}, Slug: ${slug}`);

        page = await require("../utils/browserManager").getNewPage();
        context = page.context();
        
        const capturedUrls = { video: new Set(), subtitles: new Set() };
        page.on("request", (request) => {
            const url = request.url();
            const isSubtitle = url.includes(".vtt") || url.includes(".srt") || url.includes("/subtitle/");
            const isVideo = (url.includes(".mp4") || url.includes(".m3u8") || url.includes("hakunaymatata.com") || url.includes("lightningbolt.site") || url.includes("vidbox.site") || url.includes(".m3u8?")) && !isSubtitle;
            
            if (isVideo && !url.includes("image/") && !url.includes("banner") && !url.includes("logo") && !url.includes(".js") && !url.includes(".css")) {
                capturedUrls.video.add(url);
            }
            if (isSubtitle) {
                capturedUrls.subtitles.add(url);
            }
        });

        let playUrl = "";
        if (id && slug) {
            const pathSegment = mediaType === "movie" ? "movies" : "series";
            const typeParam = mediaType === "movie" ? "/movie/detail" : "/series/detail";
            playUrl = `${BASE_URL}/spa/videoPlayPage/${pathSegment}/${slug}?id=${id}&type=${encodeURIComponent(typeParam)}&lang=en`;
        }

        const waitForStreams = async (timeout) => {
            const start = Date.now();
            while (Date.now() - start < timeout) {
                if (capturedUrls.video.size > 0) return true;
                await new Promise(r => setTimeout(r, 200));
            }
            return false;
        };

        if (playUrl) {
            console.log(`[SFlix] Navigating directly to play page: ${playUrl}`);
            await page.goto(playUrl, { waitUntil: "commit", timeout: 60000 }).catch(() => {});
        } else {
            console.log(`[SFlix] Falling back to detail page navigation: ${detailUrl}`);
            await page.goto(detailUrl, { waitUntil: "domcontentloaded", timeout: 60000 });
            
            console.log("[SFlix] Falling back to button click strategy...");
            const watchSelectors = [".watch-btn", "button.btn", "h4:has-text('Watch Free')"];
            let clicked = false;
            for (const selector of watchSelectors) {
                try {
                    const btn = await page.waitForSelector(selector, { timeout: 10000, state: 'visible' }).catch(() => null);
                    if (btn) {
                        await btn.click({ force: true }).catch(() => {});
                        clicked = true;
                        break;
                    }
                } catch { continue; }
            }
            if (!clicked) await page.evaluate(() => document.querySelector('.watch-btn')?.click());
            await new Promise(r => setTimeout(r, 10000));
        }

        try {
            await page.waitForSelector('.art-video-player, video, iframe', { timeout: 5000 });
        } catch { /* ignore */ }

        if (mediaType === "tv" && (season || episode)) {
            console.log(`[SFlix] Series detected, looking for S${season} E${episode}...`);
            
            try {
                await page.waitForFunction(() => {
                    const seasonBox = document.querySelector('.season-box, .season-box2, .ss-box');
                    const epItems = document.querySelectorAll('.ep-item, .eps-item');
                    return seasonBox && seasonBox.textContent?.trim().length > 0 && 
                           epItems.length > 0 && Array.from(epItems).some(i => i.textContent?.trim().length > 0);
                }, { timeout: 25000 });
            } catch (e) {
                console.log("[SFlix] UI hydration timeout");
            }

            await page.evaluate(async ({ s, e }) => {
                const sleep = (ms) => new Promise(resolve => setTimeout(resolve, ms));
                const log = (msg) => console.log(`[Picker] ${msg}`);
                let hasChanged = false;
                
                const clickSafe = (el) => {
                    if (!el) return;
                    el.scrollIntoView({ block: 'center' });
                    el.click();
                    el.dispatchEvent(new MouseEvent('mousedown', { bubbles: true }));
                    el.dispatchEvent(new MouseEvent('mouseup', { bubbles: true }));
                    el.dispatchEvent(new MouseEvent('click', { bubbles: true }));
                };

                const findByText = (text, selector = 'div, span, button, h4') => {
                    return Array.from(document.querySelectorAll(selector)).find(el => {
                        const t = (el.textContent || el.innerText || "").trim().toLowerCase();
                        return t === text.toLowerCase() || t.includes(text.toLowerCase());
                    });
                };

                let seasonBox = null;
                for (let i = 0; i < 25; i++) {
                    const boxes = Array.from(document.querySelectorAll('.season-box, .season-box2, .ss-box, .choice-header .season-box'));
                    seasonBox = boxes.find(el => {
                        const t = el.textContent?.trim() || "";
                        return t.toLowerCase().includes('season') || /\d+/.test(t);
                    }) || findByText('Season');
                    
                    if (seasonBox && seasonBox.textContent?.trim()) break;
                    await sleep(1000);
                }
                
                const currentSeasonText = (seasonBox?.innerText || seasonBox?.textContent || "").trim();
                const normS = s ? s.toString().padStart(2, '0') : "";
                
                const isCorrectSeason = currentSeasonText.includes(normS) || 
                                      currentSeasonText.includes(`Season ${s}`) ||
                                      currentSeasonText.includes(`Season ${normS}`);

                if (seasonBox && s && !isCorrectSeason) {
                    clickSafe(seasonBox);
                    await sleep(2500);
                    let items = Array.from(document.querySelectorAll('.choice-season, .season-item, .ss-item, [data-season], .ss-dropdown-item, .se-item, .choice-seasons div'));
                    if (items.length === 0) {
                        clickSafe(seasonBox);
                        await sleep(2000);
                        items = Array.from(document.querySelectorAll('.choice-season, .season-item, .ss-item, [data-season], .ss-dropdown-item, .se-item, .choice-seasons div'));
                    }
                    const target = items.find(i => {
                        const t = (i.textContent || i.innerText || "").trim();
                        return t.includes(`Season ${normS}`) || t.includes(`Season ${s}`) || t === s.toString() || t === normS;
                    }) || findByText(`Season ${normS}`) || findByText(`Season ${s}`);
                    if (target) {
                        clickSafe(target);
                        hasChanged = true;
                        await sleep(5000);
                    }
                }
                
                const normE = e ? e.toString().padStart(2, '0') : "";
                const eStr = e ? e.toString() : "";
                let epItems = [];
                for (let i = 0; i < 20; i++) {
                    if (i === 5 && epItems.length === 0 && hasChanged) {
                         const active = document.querySelector('.se-item-active, .active-season, .choice-season.se-item-active');
                         if (active) clickSafe(active);
                    }
                    epItems = Array.from(document.querySelectorAll('.ep-item, .eps-item, [data-episode]'));
                    if (epItems.length > 0 && epItems.some(item => (item.innerText || item.textContent || "").trim())) break;
                    await sleep(1000);
                }
                
                const targetEp = epItems.find(i => {
                    const t = (i.innerText || i.textContent || "").trim();
                    const dataEp = i.getAttribute('data-episode');
                    return e && (t === normE || t === eStr || t.includes(`Episode ${e}`) || dataEp === eStr || dataEp === normE);
                }) || epItems.find(i => {
                    const span = i.querySelector('span');
                    const st = span?.textContent?.trim() || "";
                    return st === normE || st === eStr;
                });
                
                if (targetEp) {
                    const isActive = targetEp.classList.contains('ep-item-active') || targetEp.classList.contains('active');
                    if (!isActive || hasChanged) {
                        clickSafe(targetEp);
                        hasChanged = true;
                        await sleep(3000);
                    }
                }
                return hasChanged;
            }, { s: season, e: episode }).catch(err => {
                console.log(`[SFlix] Picker error: ${err.message}`);
                return false;
            });
            
            await new Promise(r => setTimeout(r, 3000));
            capturedUrls.video.clear();
            capturedUrls.subtitles.clear();
        }

        if (capturedUrls.video.size === 0) {
            await triggerPlayback(page, "final");
            const gotStreams = await waitForStreams(10000);
            
            if (!gotStreams) {
                 await triggerPlayback(page, "retry");
                 await waitForStreams(8000);
            }
        }

        const allCaptured = Array.from(capturedUrls.video);
        let movieStreams = allCaptured.filter(url => {
            const lowUrl = url.toLowerCase();
            return !lowUrl.includes("-ld.mp4") && 
                   !lowUrl.includes("trailer") && 
                   !lowUrl.includes("teaser") &&
                   !lowUrl.includes("preview") &&
                   !lowUrl.includes("promo");
        });

        const trustedDomains = ["hakunaymatata.com", "vidbox.site", "lightningbolt.site", "bunnycdn"];
        const hasTrusted = movieStreams.some(url => trustedDomains.some(d => url.includes(d)));
        
        if (hasTrusted) {
            movieStreams = movieStreams.filter(url => trustedDomains.some(d => url.includes(d)));
        } 

        if (movieStreams.length > 0) {
             const subtitles = Array.from(capturedUrls.subtitles).map(url => ({ url, lang: "en", label: "English" }));
             for (const videoUrl of movieStreams) {
                 streams.push({
                     url: videoUrl, 
                     quality: "Auto", 
                     provider: "SFlix",
                     subtitles: subtitles.length > 0 ? subtitles : undefined,
                     headers: { "Referer": "https://sflix.film/", "User-Agent": USER_AGENT, "Origin": "https://sflix.film" }
                 });
             }
        } else {
            const domStreams = await page.evaluate(() => {
                const results = [];
                document.querySelectorAll("video, source").forEach(el => {
                    const src = el.src;
                    if (src && src.startsWith("http")) results.push(src);
                });
                return results;
            });
            const valid = domStreams.filter(url => !url.includes("-ld.mp4") && !url.includes("trailer"));
            for (const videoUrl of valid) {
                streams.push({
                    url: videoUrl, 
                    quality: "Auto",
                    provider: "SFlix",
                    headers: { "Referer": "https://sflix.film/", "User-Agent": USER_AGENT, "Origin": "https://sflix.film" }
                });
            }
        }
    } catch (error) {
        console.error(`[SFlix] Error: ${error.message}`);
    } finally {
        if (context) await context.close();
    }
    return streams;
}

module.exports = { getStreams: getSflixStreams };
