/**
 * Livesport M3U8 Extractor (Updated for Streamed.pk)
 * Extracts streams via API after parsing SvelteKit data
 */

const axios = require('axios');
const { getNewPage } = require('./utils/browserManager');

class LivesportM3U8Extractor {
    constructor() {
        this.client = axios.create({
            headers: {
                'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36',
                'Accept': 'text/html,application/xhtml+xml,application/xml;q=0.9,image/webp,*/*;q=0.8',
                'Accept-Language': 'en-US,en;q=0.5',
                'Connection': 'keep-alive'
            },
            timeout: 10000
        });
    }

    async extractAllStreams(watchUrl) {
        let page = null;
        try {
            console.log(`[Extractor] Scraping: ${watchUrl}`);
            
            page = await getNewPage({
                userAgent: 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36'
            });

            const response = await page.goto(watchUrl, { waitUntil: 'domcontentloaded', timeout: 30000 });
            if (response.status() >= 400) {
                throw new Error(`Page returned status ${response.status()}`);
            }

            const html = await page.content();
            
            // Extract sources array directly
            const sourcesMatch = html.match(/(?:sources|"sources"):\s*(\[.*?\])(?:,|,\}|, \}| \})/);
            if (!sourcesMatch) {
                // Fallback to simpler match
                const simpleSourcesMatch = html.match(/sources:(\[.*?\])/);
                if (!simpleSourcesMatch) throw new Error('Could not find sources list');
                return await this.parseAndProcess(simpleSourcesMatch[1], html);
            }

            return await this.parseAndProcess(sourcesMatch[1], html);

        } catch (error) {
            console.error(`[Extractor] Fatal error: ${error.message}`);
            return { error: error.message };
        } finally {
            if (page) await page.close();
        }
    }

    async parseAndProcess(sourcesStr, html) {
        let sources = [];
        try {
            // Attempt to clean unquoted keys for JSON.parse
            const fixedStr = sourcesStr.replace(/([{,])\s*([a-zA-Z0-9_]+):/g, '$1"$2":');
            sources = JSON.parse(fixedStr);
        } catch (e) {
            // Regex fallback for: { source: "alpha", id: "..." }
            const objectRegex = /\{.*?\}/g;
            let objMatch;
            while ((objMatch = objectRegex.exec(sourcesStr)) !== null) {
                const objStr = objMatch[0];
                const sourceMatch = objStr.match(/(?:source|"source"):\s*"([^"]+)"/);
                const idMatch = objStr.match(/(?:id|"id"):\s*"([^"]+)"/);
                if (sourceMatch && idMatch) sources.push({ source: sourceMatch[1], id: idMatch[1] });
            }
        }

        if (!sources || sources.length === 0) throw new Error('No sources parsed');

        const titleMatch = html.match(/title:"([^"]+)"/);
        const title = titleMatch ? titleMatch[1] : 'Live Match';
        const categoryMatch = html.match(/category:"([^"]+)"/);
        const category = categoryMatch ? categoryMatch[1] : 'sports';

        return await this.processSources(sources, title, category);
    }

    async processSources(sources, title, category) {
        const streams = [];
        const processedIds = new Set();

        for (const source of sources) {
            const streamId = source.id;
            if (processedIds.has(streamId)) continue;
            processedIds.add(streamId);

            const apiUrl = `https://streamed.pk/api/stream/${source.source}/${streamId}`;
            try {
                const apiResponse = await this.client.get(apiUrl, {
                    headers: { 'Referer': 'https://streamed.pk/' }
                });
                
                const streamDataList = apiResponse.data;
                if (Array.isArray(streamDataList)) {
                    for (const streamData of streamDataList) {
                        const embedUrl = streamData.embedUrl;
                        const m3u8Url = await this.extractM3U8FromEmbed(embedUrl);
                        
                        streams.push({
                            title: `${title} (${source.source} ${streamData.streamNo})`,
                            category: category,
                            quality: streamData.hd ? 'HD' : 'SD',
                            language: streamData.language || 'English',
                            url: m3u8Url || embedUrl,
                            m3u8Url: m3u8Url,
                            isCurrent: true,
                            source: source.source,
                            viewers: streamData.viewers
                        });
                    }
                }
            } catch (e) {
                console.log(`[Extractor] API fail for ${streamId}: ${e.message}`);
            }
        }

        return { title, streams, metadata: { totalStreams: streams.length } };
    }

    async extractM3U8FromEmbed(embedUrl) {
        let page = null;
        try {
            console.log(`[Extractor] Browser scraping: ${embedUrl}`);
            
            page = await getNewPage({
                userAgent: 'Mozilla/5.0 (Linux; Android 6.0; Nexus 5 Build/MRA58N) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/144.0.0.0 Mobile Safari/537.36',
                viewport: { width: 375, height: 812 },
                isMobile: true
            });
            
            // Console logging for debug
            // page.on('console', msg => console.log(`[Browser Console] ${msg.text()}`));
            
            let m3u8Url = null;
            await page.route('**/*', route => {
                const url = route.request().url();
                if ((url.includes('.m3u8') || url.includes('playlist')) && !m3u8Url) {
                    console.log(`[Extractor] Intercepted M3U8: ${url}`);
                    m3u8Url = url;
                }
                const type = route.request().resourceType();
                if (['image', 'font', 'stylesheet'].includes(type) && !url.includes('.m3u8')) return route.abort();
                route.continue();
            });

            await page.setExtraHTTPHeaders({ 'Referer': 'https://streamed.pk/' });
            const response = await page.goto(embedUrl, { waitUntil: 'domcontentloaded', timeout: 15000 });
            console.log(`[Extractor] Status: ${response.status()}`);
            
            // Wait for potential JS execution
            await new Promise(r => setTimeout(r, 2000));

            // Trigger interaction
            await page.evaluate(() => {
                const b = document.querySelector('.jw-icon-playback') || document.querySelector('video') || document.body;
                if (b) b.click();
            });

            const start = Date.now();
            while (!m3u8Url && Date.now() - start < 10000) {
                await new Promise(r => setTimeout(r, 500));
            }

            if (!m3u8Url) {
                // Broad content search
                const content = await page.content();
                const m = content.match(/(https?:\/\/[^"']+\.m3u8[^"']*)/);
                if (m) {
                    console.log(`[Extractor] Found via content search: ${m[1]}`);
                    m3u8Url = m[1];
                }
            }

            if (!m3u8Url) {
                m3u8Url = await page.evaluate(() => {
                    if (window.jwplayer) return window.jwplayer().getConfig?.().file;
                    if (window.player && window.player.options) return window.player.options.source;
                    return null;
                });
            }

            return m3u8Url;
        } catch (e) {
            console.error(`[Extractor] Browser error: ${e.message}`);
            return null;
        } finally {
            if (page) await page.close();
        }
    }
}

module.exports = LivesportM3U8Extractor;
