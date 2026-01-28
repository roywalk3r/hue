/**
 * Livesport M3U8 Extractor (Updated for Streamed.pk)
 * Extracts streams via API after parsing SvelteKit data
 */

const axios = require('axios');
const cheerio = require('cheerio');

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
        try {
            console.log(`[Extractor] Scraping: ${watchUrl}`);
            
            // 1. Fetch Watch Page
            const response = await this.client.get(watchUrl);
            const html = response.data;
            
            // 2. Parse SvelteKit Data to find Match ID and Sources
            // Look for kit.start call
            const kitMatch = html.match(/kit\.start\(app,\s*element,\s*(\{[\s\S]*?\})\);/);
            if (!kitMatch) {
                throw new Error('Could not find SvelteKit data');
            }

            let jsonData;
            try {
                // The regex match might need cleanup if it captured too much or little
                // We need to parse the JSON object
                // The match[1] is the config object. inside is 'data' array.
                // It's safer to find the match object directly via regex on the full HTML if json parsing is tricky
                const matchDataRegex = /match:(\{.*?\}),teams/;
                const matchDataMatch = html.match(matchDataRegex);
                
                if (matchDataMatch) {
                    // This is a partial JSON, tricky to parse.
                    // Let's try to extract the whole 'data' array from the script.
                    // Or parse the 'sources' array directly
                    const sourcesMatch = html.match(/sources:(\[\{.*?\}\])/);
                    if (sourcesMatch) {
                        const sources = JSON.parse(sourcesMatch[1]);
                        const titleMatch = html.match(/title:"([^"]+)"/);
                        const title = titleMatch ? titleMatch[1] : 'Unknown Match';
                        const categoryMatch = html.match(/category:"([^"]+)"/);
                        const category = categoryMatch ? categoryMatch[1] : 'sports';

                        return await this.processSources(sources, title, category);
                    }
                }
            } catch (e) {
                console.log('[Extractor] JSON parse error, trying fallback', e.message);
            }
            
            throw new Error('Failed to extract stream sources');

        } catch (error) {
            console.error(`[Extractor] Fatal error: ${error.message}`);
            return { error: error.message };
        }
    }

    async processSources(sources, title, category) {
        const streams = [];
        const processedIds = new Set();

        for (const source of sources) {
            if (processedIds.has(source.id)) continue;
            processedIds.add(source.id);

            // Construct API URL
            // https://streamed.pk/api/stream/{source_name}/{id}
            const apiUrl = `https://streamed.pk/api/stream/${source.source}/${source.id}`;
            
            try {
                const apiResponse = await this.client.get(apiUrl, {
                    headers: { 'Referer': 'https://streamed.pk/' }
                });
                
                const streamDataList = apiResponse.data;
                
                if (Array.isArray(streamDataList)) {
                    for (const streamData of streamDataList) {
                        const embedUrl = streamData.embedUrl;
                        const streamTitle = `${title} (${source.source} ${streamData.streamNo})`;
                        
                        // Extract M3U8 from embed
                        const m3u8Url = await this.extractM3U8FromEmbed(embedUrl);
                        
                        streams.push({
                            title: streamTitle,
                            category: category,
                            quality: streamData.hd ? 'HD' : 'SD',
                            language: streamData.language || 'English',
                            url: m3u8Url || embedUrl, // Use M3U8 if found, else embed
                            m3u8Url: m3u8Url,
                            isCurrent: true,
                            source: source.source,
                            viewers: streamData.viewers
                        });
                    }
                }
            } catch (e) {
                console.log(`[Extractor] Failed to fetch stream ${source.id}: ${e.message}`);
            }
        }

        return {
            title,
            streams,
            metadata: {
                totalStreams: streams.length
            }
        };
    }

    async extractM3U8FromEmbed(embedUrl) {
        try {
            const response = await this.client.get(embedUrl, {
                headers: { 'Referer': 'https://streamed.pk/' }
            });
            const html = response.data;
            
            // Look for m3u8 in the embed source
            const m3u8Match = html.match(/(https?:\/\/[^"']+\.m3u8[^"']*)/);
            return m3u8Match ? m3u8Match[1] : null;
        } catch (e) {
            console.log(`[Extractor] Embed scrape failed: ${e.message}`);
            return null;
        }
    }
}

module.exports = LivesportM3U8Extractor;