/**
 * Livesport M3U8 Extractor (Updated for Streamed.pk)
 * Extracts streams via API after parsing SvelteKit data
 */

const axios = require('axios');
const { getNewPage } = require('./utils/browserManager');
const { load } = require('cheerio');

// Team name to slug mapping for Premier League and other competitions
const TEAM_SLUG_MAP = {
    // Premier League teams
    'west-ham': 'pl_whu', 'west-ham-united': 'pl_whu', 'westham': 'pl_whu',
    'arsenal': 'pl_ars', 'afc': 'pl_ars',
    'aston-villa': 'pl_avl', 'villa': 'pl_avl',
    'bournemouth': 'pl_bou', 'afc-bournemouth': 'pl_bou',
    'brentford': 'pl_bre',
    'brighton': 'pl_bha', 'brighton-hove': 'pl_bha',
    'chelsea': 'pl_che',
    'crystal-palace': 'pl_cry', 'palace': 'pl_cry',
    'everton': 'pl_eve',
    'fulham': 'pl_ful',
    'ipswich': 'pl_ips', 'ipswich-town': 'pl_ips',
    'leicester': 'pl_lei', 'leicester-city': 'pl_lei',
    'liverpool': 'pl_liv',
    'manchester-city': 'pl_mci', 'man-city': 'pl_mci', 'mancity': 'pl_mci',
    'manchester-united': 'pl_mun', 'man-united': 'pl_mun', 'man-utd': 'pl_mun',
    'newcastle': 'pl_new', 'newcastle-united': 'pl_new',
    'nottingham-forest': 'pl_nfo', 'nottingham': 'pl_nfo', 'forest': 'pl_nfo',
    'southampton': 'pl_sou',
    'tottenham': 'pl_tot', 'spurs': 'pl_tot', 'tottenham-hotspur': 'pl_tot',
    'wolves': 'pl_wol', 'wolverhampton': 'pl_wol',
    // Day-based slugs (for scheduled matches)
    'sunday': 'pl_sun', 'saturday': 'pl_sat', 'monday': 'pl_mon',
    'tuesday': 'pl_tue', 'wednesday': 'pl_wed', 'thursday': 'pl_thu', 'friday': 'pl_fri',
    // Other competitions
    'afcon': 'afcon', 'africa': 'afcon',
    // NBA / Misc
    'nbatv': 'nbatv', 'nba': 'nbatv', 'espn': 'espn', 'tnt': 'tnt',
};

function deriveSlugsFromMatchName(matchName) {
    const slugs = [];
    const daySlugs = [];
    
    // Day-based slugs should be low priority
    const days = ['sunday', 'monday', 'tuesday', 'wednesday', 'thursday', 'friday', 'saturday'];
    const today = days[new Date().getDay()];
    if (TEAM_SLUG_MAP[today]) daySlugs.push(TEAM_SLUG_MAP[today]);

    if (matchName) {
        // Handle complex slugs from match name (replace hyphens with underscores or kept as is)
        const normalized = matchName.toLowerCase().replace(/[^a-z0-9-]/g, '-');
        
        // Try exact match slug if it looks like a long one
        if (normalized.includes('-vs-') || normalized.split('-').length > 3) {
            slugs.push(normalized);
            slugs.push(normalized.replace(/-/g, '_'));
        }

        // Try to find team names in the match string
        for (const [teamKey, slug] of Object.entries(TEAM_SLUG_MAP)) {
            if (days.includes(teamKey)) continue;
            if (normalized.includes(teamKey)) {
                if (!slugs.includes(slug)) slugs.push(slug);
            }
        }
    }
    
    return [...slugs, ...daySlugs];
}

class LivesportM3U8Extractor {
    constructor() {
        this.client = axios.create({
            headers: {
                'User-Agent': 'Mozilla/5.0 (iPhone; CPU iPhone OS 18_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.5 Mobile/15E148 Safari/604.1',
                'Accept': 'text/html,application/xhtml+xml,application/xml;q=0.9,image/webp,*/*;q=0.8',
                'Accept-Language': 'en-US,en;q=0.5',
                'Connection': 'keep-alive'
            },
            timeout: 10000
        });
        this.matchContext = null;
    }

    setMatchContext(matchName) {
        this.matchContext = matchName;
    }

    describeError(error) {
        if (!error) return 'Unknown error';
        if (typeof error === 'string') return error;
        if (error.response) return `HTTP ${error.response.status}: ${JSON.stringify(error.response.data).substring(0, 100)}`;
        return error.message || String(error);
    }

    async curlFetch(url) {
        const { execSync } = require('child_process');
        console.log(`[Extractor] Curl fetching: ${url}`);
        const userAgent = 'Mozilla/5.0 (iPhone; CPU iPhone OS 18_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.5 Mobile/15E148 Safari/604.1';
        try {
            const cmd = `curl -L -s -m 15 -H "User-Agent: ${userAgent}" -H "Referer: https://livesport.su/" "${url}"`;
            const output = execSync(cmd, { maxBuffer: 1024 * 1024 * 5 }).toString();
            return { data: output };
        } catch (e) {
            console.error(`[Extractor] Curl failed for ${url}: ${e.message}`);
            throw e;
        }
    }

    async extractAllStreams(watchUrl) {
        try {
            console.log(`[Extractor] Scraping (Curl): ${watchUrl}`);
            const urlMatch = watchUrl.match(/\/watch\/([^/]+)/);
            if (urlMatch) this.setMatchContext(urlMatch[1]);

            const response = await this.curlFetch(watchUrl);
            const html = response.data;
            const $ = load(html);
            const title = $('h1.text-3xl').first().text().trim() || 'Live Match';

            if (html.includes('self.__next_f.push')) {
                const nextPatterns = [
                    /\\"sources\\":(\[.*?\])(?=\,"matches\\"|\,\"teams\\"|\]})/,
                    /\\"sources\\":(\[.*?\])(?:,|})/,
                    /\\"sources\\":(\[.*\])/
                ];

                for (const pattern of nextPatterns) {
                    const nextMatch = html.match(pattern);
                    if (nextMatch) {
                        try {
                            const unescaped = nextMatch[1].replace(/\\"/g, '"');
                            let jsonStr = unescaped;
                            const lastBracket = jsonStr.lastIndexOf(']');
                            if (lastBracket !== -1) jsonStr = jsonStr.substring(0, lastBracket + 1);

                            const sources = JSON.parse(jsonStr);
                            if (Array.isArray(sources) && sources.length > 0) {
                                console.log(`[Extractor] Found ${sources.length} sources in Next.js data`);
                                const titleMatch = html.match(/\\"title\\":\\"([^"]+)\\"/);
                                const titleStr = titleMatch ? titleMatch[1] : title;
                                return await this.processSources(sources, titleStr, 'sports');
                            }
                        } catch (e) { }
                    }
                }
            }

            const scrapedSources = [];
            $('a[href*="/watch/"]').each((i, link) => {
                const $link = $(link);
                const href = $link.attr('href');
                if (!href) return;
                const parts = href.split('/').filter(Boolean);
                if (parts.length < 4) return;
                scrapedSources.push({
                    id: parts[1], source: parts[2], streamNo: parseInt(parts[3]) || 1,
                    hd: $link.text().includes('HD') || $link.find('span').text().includes('HD'),
                    language: $link.find('svg.lucide-globe + span').text().trim() || 'English',
                    viewers: parseInt($link.find('svg.lucide-eye + span').text().replace(/,/g, '')) || null
                });
            });

            if (scrapedSources.length > 0) {
                console.log(`[Extractor] Found ${scrapedSources.length} sources via flexible HTML scraping`);
                return await this.processSources(scrapedSources, title, 'sports');
            }
            throw new Error('Could not find sources list');
        } catch (error) {
            console.log(`[Extractor] Scraping failed: ${error.message}, falling back to Browser`);
            return await this.extractAllStreamsBrowser(watchUrl);
        }
    }

    async extractAllStreamsBrowser(watchUrl) {
        let page = null;
        try {
            console.log(`[Extractor] Scraping (Browser): ${watchUrl}`);
            page = await getNewPage({ userAgent: 'Mozilla/5.0 (iPhone; CPU iPhone OS 18_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.5 Mobile/15E148 Safari/604.1' });
            await page.goto(watchUrl, { waitUntil: 'domcontentloaded', timeout: 30000 });
            const html = await page.content();
            const nextMatch = html.match(/\\"sources\\":(\[.*?\])(?:,|})/);
            if (nextMatch) {
                const unescaped = nextMatch[1].replace(/\\"/g, '"');
                const sources = JSON.parse(unescaped);
                return await this.processSources(sources, 'Live Match', 'sports');
            }
            throw new Error('Could not find sources list in browser');
        } catch (error) {
            console.error(`[Extractor] Browser error: ${error.message}`);
            return { error: error.message };
        } finally {
            if (page) await page.close();
        }
    }

    async processSources(sources, title, category) {
        const streams = [];
        const processedIds = new Set();
        for (const source of sources) {
            const streamId = source.id;
            const key = `${source.source}-${streamId}-${source.streamNo || 1}`;
            if (processedIds.has(key)) continue;
            processedIds.add(key);
            const streamUrl = `https://livesport.su/watch/${this.matchContext}/${source.source}/${source.streamNo || 1}`;
            try {
                let m3u8Url = await this.extractM3U8FromStream(streamUrl);
                if (!m3u8Url && source.embedUrl && source.embedUrl.includes('vidbox.site')) {
                    m3u8Url = await this.extractM3U8FromVidbox(source.embedUrl);
                }
                if (m3u8Url) {
                    streams.push({
                        title: `${title} (${source.source} ${source.streamNo || 1})`,
                        category: category, quality: source.hd ? 'HD' : 'SD',
                        language: source.language || 'English', url: m3u8Url, m3u8Url: m3u8Url,
                        isCurrent: true, source: source.source, viewers: source.viewers
                    });
                }
            } catch (e) {
                console.log(`[Extractor] Extraction failed for ${key}: ${e.message}`);
            }
        }
        return { title, streams, metadata: { totalStreams: streams.length } };
    }

    async extractM3U8FromStream(streamUrl) {
        try {
            console.log(`[Extractor] Finding M3U8 for: ${streamUrl}`);
            const response = await this.curlFetch(streamUrl);
            const html = response.data;
            const $ = load(html);
            let iframeSrc = $('iframe[title*="player"]').attr('src') || $('iframe[src*="embed"]').attr('src') ||
                          $('iframe[src*="vidbox"]').attr('src') || $('iframe').first().attr('src');
            if (!iframeSrc) {
                const scriptData = $('script').map((i, el) => {
                    const script = $(el).html() || '';
                    const iframeMatch = script.match(/<iframe[^>]*src=["']([^"']+)["']/i);
                    return iframeMatch ? iframeMatch[1] : null;
                }).get();
                if (scriptData.length > 0) iframeSrc = scriptData[0];
            }
            if (!iframeSrc) return null;
            if (iframeSrc.startsWith('//')) iframeSrc = 'https:' + iframeSrc;
            else if (iframeSrc.startsWith('/')) iframeSrc = 'https://livesport.su' + iframeSrc;
            const streamId = iframeSrc.split('/').pop();
            return await this.decodeStreamId(streamId, iframeSrc);
        } catch (error) {
            return null;
        }
    }

    async extractM3U8FromVidbox(vidboxUrl) {
        return await this.extractM3U8FromEmbed(vidboxUrl);
    }

    async extractM3U8FromEmbed(embedUrl) {
        let page = null;
        try {
            console.log(`[Extractor] Capturing traffic from: ${embedUrl}`);
            page = await getNewPage({ userAgent: 'Mozilla/5.0 (iPhone; CPU iPhone OS 18_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.5 Mobile/15E148 Safari/604.1', viewport: { width: 375, height: 812 }, isMobile: true });
            let m3u8Url = null;
            await page.route('**/*', route => {
                const url = route.request().url();
                if (url.includes('.m3u8') || url.includes('secure')) {
                    console.log(`[Extractor] Intercepted M3U8: ${url}`);
                    m3u8Url = url;
                }
                const type = route.request().resourceType();
                if (['font', 'stylesheet'].includes(type)) return route.abort();
                route.continue();
            });
            await page.setExtraHTTPHeaders({ 'Referer': 'https://livesport.su/' });
            await page.goto(embedUrl, { waitUntil: 'domcontentloaded', timeout: 20000 });
            await new Promise(r => setTimeout(r, 5000));
            await page.evaluate(() => {
                const b = document.querySelector('.jw-icon-playback') || document.querySelector('video') || document.body;
                if (b) b.click();
            });
            const start = Date.now();
            while (!m3u8Url && Date.now() - start < 10000) await new Promise(r => setTimeout(r, 1000));
            return m3u8Url;
        } catch (e) {
            return null;
        } finally {
            if (page) await page.close();
        }
    }

    async verifyM3U8Content(url) {
        try {
            const { execSync } = require('child_process');
            const userAgent = 'Mozilla/5.0 (iPhone; CPU iPhone OS 18_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.5 Mobile/15E148 Safari/604.1';
            const cmd = `curl -L -s -m 5 -H "User-Agent: ${userAgent}" -H "Referer: https://a.embedsports.top/" "${url}"`;
            let body = execSync(cmd).toString();

            if (!body.includes('#EXTM3U')) return null;

            // If it's a master playlist, fetch the first media playlist
            if (body.includes('#EXT-X-STREAM-INF')) {
                const lines = body.split('\n');
                const mediaPath = lines.find(l => l.trim() && !l.startsWith('#'));
                if (mediaPath) {
                    const mediaUrl = new URL(mediaPath, url).href;
                    const mediaCmd = `curl -L -s -m 5 -H "User-Agent: ${userAgent}" -H "Referer: https://a.embedsports.top/" "${mediaUrl}"`;
                    body = execSync(mediaCmd).toString();
                }
            }

            // check for promo keywords in the final media playlist
            const lowerBody = body.toLowerCase();
            const promoKeywords = ['ppv.to', 'match end', 'unavailable', 'promo', 'subscribe'];
            
            for (const keyword of promoKeywords) {
                if (lowerBody.includes(keyword)) {
                    console.log(`[Extractor] Rejected M3U8 (found promo keyword '${keyword}'): ${url}`);
                    return null;
                }
            }

            // Ensure it has segments
            if (!body.includes('#EXTINF')) {
                console.log(`[Extractor] Rejected M3U8 (no segments found): ${url}`);
                return null;
            }

            return url;
        } catch (e) {
            return null;
        }
    }

    async decodeSecurePoocloud(slug) {
        const tokens = [
            'SMovPUhfbDNpVSuIonkeMlFErbRopSrb',
            'OwTlEqiqLsOgcQwEZjKmwPTaYTnFqreo',
            'tZzqPtoHcuFAIWvRtWZGeEMgbpzdTGSv',
            'RMNlAgdZfNwpvhTVMKCCwgwtXuxIwVJz',
            'jKMJuJiszSgdeOXLlAZMOptCMRPotZLj'
        ];
        const now = Math.floor(Date.now() / 1000);
        const startTime = (Math.floor(now / 3600) * 3600).toString();
        const endTime = (Math.floor(now / 3600) * 3600 + 25200).toString(); 
        for (const token of tokens) {
            const urls = [
                `https://strm.poocloud.in/secure/${token}/${startTime}/${endTime}/${slug}/tracks-v1a1/mono.ts.m3u8`,
                `https://strm.poocloud.in/secure/${token}/${startTime}/${endTime}/${slug}/index.m3u8`,
                `https://lb3.strmd.top/secure/${token}/rtmp/stream/${slug}/1/playlist.m3u8`,
                `https://lb3.strmd.top/secure/${token}/admin/stream/${slug}/1/playlist.m3u8`
            ];
            for (const testUrl of urls) {
                const verifiedUrl = await this.verifyM3U8Content(testUrl);
                if (verifiedUrl) {
                    console.log(`[Extractor] SUCCESS! Found verified M3U8: ${verifiedUrl}`);
                    return verifiedUrl;
                }
            }
        }
        return null;
    }

    async decodeStreamId(streamId, iframeUrl) {
        const domains = [
            'https://strm.poocloud.in', 'https://gg.poocloud.in', 'https://gg2.poocloud.in',
            'https://poocloud.in', 'https://lb3.strmd.top', 'https://lb8.strmd.top'
        ];
        const sanitize = (s) => s ? s.replace(/[^a-zA-Z0-9_-]/g, '') : null;
        const cleanedId = sanitize(streamId);
        const slugCandidates = new Set();
        if (cleanedId) slugCandidates.add(cleanedId);
        const contextSlugs = this.matchContext ? deriveSlugsFromMatchName(this.matchContext) : [];
        const prioritizedSlugs = [...contextSlugs];
        slugCandidates.forEach(s => { if (!prioritizedSlugs.includes(s)) prioritizedSlugs.push(s); });
        const fallbacks = ['pl_sun', 'pl_sat', 'pl_mon', 'pl_tue', 'pl_wed', 'pl_thu', 'pl_fri'];
        fallbacks.forEach(fb => { if (!prioritizedSlugs.includes(fb)) prioritizedSlugs.push(fb); });

        for (const slug of prioritizedSlugs.slice(0, 8)) {
            const secureUrl = await this.decodeSecurePoocloud(slug);
            if (secureUrl) return secureUrl;
        }

        const trackVariants = [
            'tracks-v1a1/mono.ts.m3u8', 'tracks-v2a1/mono.ts.m3u8', 'tracks-v3a1/mono.ts.m3u8',
            'tracks-v1b1/mono.ts.m3u8', 'rtmp/stream/1/playlist.m3u8', '1/playlist.m3u8'
        ];
        for (const domain of domains) {
            for (const slug of prioritizedSlugs) {
                for (const variant of trackVariants) {
                    const testUrl = `${domain}/${slug}/${variant}`;
                    const verifiedUrl = await this.verifyM3U8Content(testUrl);
                    if (verifiedUrl) {
                        console.log(`[Extractor] SUCCESS! Found verified M3U8: ${verifiedUrl}`);
                        return verifiedUrl;
                    }
                }
            }
        }
        return null;
    }
}

module.exports = LivesportM3U8Extractor;
