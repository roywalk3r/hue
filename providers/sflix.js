const cheerio = require("cheerio");
const axios = require("axios");
const { getNewContext } = require("../utils/browserManager");

const BASE_URL = "https://sflix.film";
const USER_AGENT = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

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
    // Optimization: Search for the base title to get all seasons
    let keyword = title;
    if (mediaType === "movie" && year) {
        keyword = `${title} ${year}`;
    }
    
    const searchUrl = `${BASE_URL}/search-result?keyword=${encodeURIComponent(keyword)}`;
    console.log(`[SFlix] Searching (${mediaType}) with keyword "${keyword}": ${searchUrl}`);
    
    try {
        const response = await axios.get(searchUrl, {
            headers: { "User-Agent": USER_AGENT }
        });
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
                const normalizedText = normalize(text);
                let currentScore = 0;

                // Exact Title Match (ignoring season suffixes)
                // Remove "S1", "S1-S23", "Season 1", "(2018)" from the SFlix title
                const cleanText = text.replace(/S\d+(-S\d+)?|Season \d+|\(\d{4}\)/gi, "").trim();
                const normClean = normalize(cleanText);
                
                if (normClean === target) {
                    currentScore += 1000; // Massive boost for exact title match
                } else if (normClean.includes(target)) {
                    currentScore += 50;
                }

                // Season Matching
                if (mediaType === "tv" && targetSeason) {
                    // Check for "S1" or "Season 1"
                    const specificSeasonRegex = new RegExp(`S(eason)?\\s*${targetSeason}\\b`, 'i');
                    if (specificSeasonRegex.test(text)) {
                        currentScore += 100; // Perfect season match
                    } else {
                        // Check for ranges like "S1-S23"
                        const rangeMatch = text.match(/S(\d+)-S(\d+)/i);
                        if (rangeMatch) {
                            const start = parseInt(rangeMatch[1]);
                            const end = parseInt(rangeMatch[2]);
                            if (targetSeason >= start && targetSeason <= end) {
                                currentScore += 80; // Valid range match
                            }
                        }
                    }
                }

                // Year match (check title and meta)
                if (year) {
                    if (text.includes(year.toString())) currentScore += 30;
                    else if (itemYear === year.toString()) currentScore += 30;
                }

                // Media type match/mismatch
                const isTVResult = metaText.includes("series") || metaText.includes("season") || metaText.includes("ss") || text.match(/S\d+/i);
                
                if (mediaType === "tv") {
                    if (isTVResult) currentScore += 50;
                    else currentScore -= 50;
                } else {
                    if (isTVResult) currentScore -= 100;
                    else currentScore += 20;
                }

                if (currentScore > bestMatch.score) {
                    bestMatch = { href, score: currentScore };
                }
            }
        });

        if (bestMatch.href) {
            console.log(`[SFlix] Selected best match: ${bestMatch.href} with score ${bestMatch.score}`);
        } else {
            console.log(`[SFlix] No suitable matches found for "${title}"`);
        }

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

        // Strategy 1: Common JSON patterns
        const match1 = html.match(/"id"\s*:\s*"?(\d{15,})"?/) || html.match(/id\s*=\s*"?(\d{15,})"?/);
        if (match1) id = match1[1];

        // Strategy 2: NUXT Data (The most reliable for SFlix)
        if (!id) {
            // Look for the subjectId in the flattened Nuxt array
            // It usually appears as a string of digits in the big JSON array
            // We look for a string of 15-20 digits that appears in the context of the title
            const titlePart = title.split(' ')[0].replace(/[.*+?^${}()|[\\]/g, '\\{new_string}');
            const nuxtMatch = html.match(new RegExp(`\"${titlePart}\"[\\s\\S]{0,500}?\"subjectId\":\s*\"?(\\d{18,20})\"?`, 'i')) || 
                              html.match(/"subjectId":\s*"?(\d{18,20})"?/);
            if (nuxtMatch) {
                // If the first match group is the title, then the ID is later? No, regex is tricky.
                // Let's stick to the simplest unique pattern: a long ID string.
                // SFlix IDs are distinctively long (18-20 chars).
                // Actually, just looking for ANY long number might be safer if we assume the page is for this show.
                const longId = html.match(/"(\d{18,20})"/);
                if (longId) id = longId[1];
            }
        }

        // Strategy 3: Fallback regex
        if (!id) {
            const safeTitle = title.replace(/[.*+?^${}()|[\\]/g, '\\$&');
            const regex = new RegExp(`"(\\d{15,})",\\d+,"${safeTitle}"`);
            const match2 = html.match(regex);
            if (match2) id = match2[1];
        }

        if (!id) {
             const match3 = html.match(/"(\\d{18,20})"/); // SFlix IDs are usually long
             if (match3) id = match3[1];
        }
        
        // Extract subjectType (1=Movie, 2=Series) from Nuxt if possible
        let type = null;
        const typeMatch = html.match(/"subjectType":\s*(\\d+)/);
        if (typeMatch) {
            const rawType = parseInt(typeMatch[1]);
            const linkMatch = html.match(/type=(%2F| \/)(movie|series)(%2F| \/)detail/);
            if (linkMatch) {
                type = linkMatch[2] === 'movie' ? 1 : 2;
            }
        }

        const slug = detailUrl.split('/').pop() || null;
        
        return { id, slug, type };
    } catch {
        return { id: null, slug: null, type: null };
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

        context = await getNewContext();
        page = await context.newPage();
        
        const capturedUrls = { video: new Set(), subtitles: new Set() };
        page.on("request", (request) => {
            const url = request.url();
            const isVideo = (url.includes(".mp4") || url.includes(".m3u8") || url.includes("hakunaymatata.com") || url.includes("lightningbolt.site") || url.includes("vidbox.site") || url.includes(".m3u8?")) 
                            && !url.includes("/subtitle/") && !url.includes(".vtt") && !url.includes(".srt");
            if (isVideo && !url.includes("image/") && !url.includes("banner") && !url.includes("logo") && !url.includes(".js") && !url.includes(".css")) capturedUrls.video.add(url);
            if (url.includes(".vtt") || url.includes(".srt") || url.includes("/subtitle/")) capturedUrls.subtitles.add(url);
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
                }, { timeout: 10000 });
            } catch (e) {
                console.log("[SFlix] UI hydration timeout (proceeding anyway)");
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

async function getStreamsDirect(title, episode, season = 1) {
    console.log(`[SFlix] Smart Fetch for: "${title}" S${season} E${episode}`);
    const streams = [];
    let context;
    
    try {
        const mediaType = (season || episode) ? "tv" : "movie";
        const detailUrl = await searchSflix(title, mediaType, null, season);
        
        if (!detailUrl) {
            console.log(`[SFlix] Title not found: "${title}"`);
            return [];
        }

        const { id, slug, type } = await getMediaInfo(detailUrl, title);
        if (!id || !slug) {
            console.log(`[SFlix] Failed to extract ID/Slug for: ${detailUrl}`);
            return [];
        }
        
        console.log(`[SFlix] Found: ${slug} (ID: ${id})`);

        context = await getNewContext();
        const page = await context.newPage();
        const capturedUrls = { video: new Set(), subtitles: new Set() };
        
        page.on("request", (request) => {
            const url = request.url();
            const isVideo = (url.includes(".mp4") || url.includes(".m3u8") || url.includes("hakunaymatata.com") || url.includes("lightningbolt.site") || url.includes("vidbox.site") || url.includes(".m3u8?")) 
                            && !url.includes("/subtitle/") && !url.includes(".vtt") && !url.includes(".srt")
                            && !url.includes("aoneroom.com") && !url.includes("macdn");
            
            if (isVideo && !url.includes("image/") && !url.includes("banner") && !url.includes("logo") && !url.includes(".js") && !url.includes(".css")) capturedUrls.video.add(url);
            if (url.includes(".vtt") || url.includes(".srt") || url.includes("/subtitle/")) capturedUrls.subtitles.add(url);
        });

        // Construct Play URL directly
        // Use extracted type if available (1=Movie, 2=Series), otherwise fallback to request mediaType
        // const effectiveType = type ? (type === 1 ? "movie" : "tv") : mediaType;
        // const pathSegment = effectiveType === "movie" ? "movies" : "series";
        // const typeParam = effectiveType === "movie" ? "/movie/detail" : "/series/detail";
        // const playUrl = `${BASE_URL}/spa/videoPlayPage/${pathSegment}/${slug}?id=${id}&type=${encodeURIComponent(typeParam)}&lang=en`;
        
        // console.log(`[SFlix] Navigating to play page: ${playUrl}`);
        // await page.goto(playUrl, { waitUntil: "commit", timeout: 60000 }).catch(() => {});

        // Safer approach: Go to detail page and Click "Watch Free"
        // This avoids guessing the /movies/ vs /series/ routing logic
        console.log(`[SFlix] Navigating to detail page: ${detailUrl}`);
        await page.goto(detailUrl, { waitUntil: "domcontentloaded", timeout: 60000 });

        try {
            // Wait for and click the Watch button
            const btnSelector = '.watch-btn, button:has-text("Watch Free")';
            const btn = await page.waitForSelector(btnSelector, { timeout: 8000, state: 'attached' });
            
            if (btn) {
                // Try Playwright click
                await btn.click({ force: true, timeout: 5000 }).catch(async () => {
                    console.log(`[SFlix] Playwright click failed, trying JS click...`);
                    // Fallback to JS click
                    await page.evaluate(() => {
                        const el = document.querySelector('.watch-btn') || document.querySelector('button.btn');
                        if (el) el.click();
                    });
                });
                console.log(`[SFlix] Clicked Watch Button`);
            }
            
            // Don't wait for navigation, just wait for streams immediately
            // await page.waitForURL('**/spa/videoPlayPage/**', { timeout: 15000 });
        } catch (e) {
            console.log(`[SFlix] Watch button click failed: ${e.message}`);
            // Try blind JS click as last resort
            await page.evaluate(() => {
                const b = document.querySelector('.watch-btn');
                if (b) b.click();
            }).catch(() => {});
        }

        // Handle Season/Episode selection for TV
        if (mediaType === "tv") {
             // Reuse the picker logic logic from getSflixStreams...
             try {
                await page.waitForFunction(() => {
                    const seasonBox = document.querySelector('.season-box, .season-box2, .ss-box');
                    return seasonBox && seasonBox.textContent?.trim().length > 0;
                }, { timeout: 5000 }); // Reduced timeout
             } catch {} 
        }

        await triggerPlayback(page, "direct");
        
        const start = Date.now();
        while (Date.now() - start < 20000) {
            if (capturedUrls.video.size > 0) {
                console.log(`[SFlix] Captured ${capturedUrls.video.size} streams!`);
                break;
            }
            await new Promise(resolve => setTimeout(resolve, 500));
        }

        const allCaptured = Array.from(capturedUrls.video);
        const valid = allCaptured.filter(url => !url.includes("-ld.mp4") && !url.includes("trailer"));
        const subtitles = Array.from(capturedUrls.subtitles).map(url => ({ url, lang: "en", label: "English" }));
        
        for (const videoUrl of valid) {
             streams.push({
                 url: videoUrl, 
                 quality: "Auto", 
                 provider: "Nova",
                 subtitles: subtitles.length > 0 ? subtitles : undefined,
                 headers: { "Referer": "https://sflix.film/", "User-Agent": USER_AGENT, "Origin": "https://sflix.film" }
             });
        }
        
    } catch (error) {
        console.error(`[SFlix] Direct Error: ${error.message}`);
    } finally {
        if (context) await context.close();
    }
    return streams;
}

async function getStreamsBySlug(id, slug, episode, season = 1) {
    console.log(`[SFlix] Direct URL Fetch for: ${slug} (ID: ${id}) E${episode}`);
    const streams = [];
    let context;
    
    try {
        context = await getNewContext();
        page.on("request", (request) => {
            const url = request.url();
            const isVideo = (url.includes(".mp4") || url.includes(".m3u8") || url.includes("hakunaymatata.com") || url.includes("lightningbolt.site") || url.includes("vidbox.site") || url.includes(".m3u8?")) 
                            && !url.includes("/subtitle/") && !url.includes(".vtt") && !url.includes(".srt")
                            && !url.includes("aoneroom.com") && !url.includes("macdn");
            
            if (isVideo && !url.includes("image/") && !url.includes("banner") && !url.includes("logo") && !url.includes(".js") && !url.includes(".css")) capturedUrls.video.add(url);
            if (url.includes(".vtt") || url.includes(".srt") || url.includes("/subtitle/")) capturedUrls.subtitles.add(url);
        });
        
        console.log(`[SFlix] Navigating to detail page: ${detailUrl}`);
        await page.goto(detailUrl, { waitUntil: "domcontentloaded", timeout: 60000 });

        try {
            const btn = await page.waitForSelector('.watch-btn, button:has-text("Watch Free")', { timeout: 10000 });
            await btn.click();
            console.log(`[SFlix] Clicked Watch Button`);
            await page.waitForURL('**/spa/videoPlayPage/**', { timeout: 15000 });
        } catch (e) {
            console.log(`[SFlix] Watch button click failed: ${e.message}`);
        }

        // Handle Season/Episode picker if it's a series
        // Note: SFlix will often auto-play Ep 1, but we should trigger the picker if needed.
        await triggerPlayback(page, "slug-direct");
        
        const start = Date.now();
        while (Date.now() - start < 15000) {
            if (capturedUrls.video.size > 0) break;
            await new Promise(r => setTimeout(r, 500));
        }

        const allCaptured = Array.from(capturedUrls.video);
        const valid = allCaptured.filter(url => !url.includes("-ld.mp4") && !url.includes("trailer"));
        const subtitles = Array.from(capturedUrls.subtitles).map(url => ({ url, lang: "en", label: "English" }));
        
        for (const videoUrl of valid) {
             streams.push({
                 url: videoUrl, 
                 quality: "Auto", 
                 provider: "Nova",
                 subtitles: subtitles.length > 0 ? subtitles : undefined,
                 headers: { "Referer": "https://sflix.film/", "User-Agent": USER_AGENT, "Origin": "https://sflix.film" }
             });
        }
        
    } catch (error) {
        console.error(`[SFlix] Slug Fetch Error: ${error.message}`);
    } finally {
        if (context) await context.close();
    }
    return streams;
}

module.exports = { getStreams: getSflixStreams, getStreamsDirect, getStreamsBySlug };
