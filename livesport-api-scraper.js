/**
 * Livesport API Scraper
 * Uses Livesport's public API to get matches and extracts M3U8 URLs
 */

const axios = require('axios');
const LivesportM3U8Extractor = require('./livesport-m3u8-extractor');
const fs = require('fs').promises;

class LivesportAPIScraper {
    constructor() {
        this.baseURL = 'https://livesport.su/api';
        this.client = axios.create({
            timeout: 10000
        });
        this.m3u8Extractor = new LivesportM3U8Extractor();
    }

    async getSports() {
        try {
            console.log('[API] Fetching sports...');
            const response = await this.client.get(`${this.baseURL}/sports`);
            // API returns { data: [...], success: true }
            if (response.data && response.data.data && Array.isArray(response.data.data)) {
                return response.data.data;
            }
            return [];
        } catch (error) {
            console.error('[API] Error fetching sports:', error.message);
            return [];
        }
    }

    async getMatches(sportId = null, popular = false) {
        try {
            let url;
            if (sportId) {
                url = popular ? `${this.baseURL}/matches/${sportId}/popular` : `${this.baseURL}/matches/${sportId}`;
            } else {
                url = popular ? `${this.baseURL}/matches/popular` : `${this.baseURL}/matches`;
            }
            
            console.log(`[API] Fetching matches from: ${url}`);
            const response = await this.client.get(url);
            // API returns { data: [...], success: true }
            if (response.data && response.data.data && Array.isArray(response.data.data)) {
                return response.data.data;
            }
            return [];
        } catch (error) {
            console.error('[API] Error fetching matches:', error.message);
            return [];
        }
    }

    async getLiveMatches(popular = false) {
        try {
            const url = popular ? `${this.baseURL}/matches/live/popular` : `${this.baseURL}/matches/live`;
            console.log(`[API] Fetching live matches from: ${url}`);
            const response = await this.client.get(url);
            
            // API returns { data: [...], success: true }
            if (response.data && response.data.data && Array.isArray(response.data.data)) {
                return response.data.data;
            }
            
            return [];
        } catch (error) {
            console.error('[API] Error fetching live matches:', error.message);
            if (error.response) {
                console.error('[API] Response status:', error.response.status);
                console.error('[API] Response data:', error.response.data);
            }
            return [];
        }
    }

    async getTodayMatches(popular = false) {
        try {
            const url = popular ? `${this.baseURL}/matches/all-today/popular` : `${this.baseURL}/matches/all-today`;
            console.log(`[API] Fetching today's matches...`);
            const response = await this.client.get(url);
            // API returns { data: [...], success: true }
            if (response.data && response.data.data && Array.isArray(response.data.data)) {
                return response.data.data;
            }
            return [];
        } catch (error) {
            console.error('[API] Error fetching today\'s matches:', error.message);
            return [];
        }
    }

    async getMatchDetails(matchId) {
        try {
            console.log(`[API] Fetching details for match: ${matchId}`);
            const response = await this.client.get(`${this.baseURL}/matches/${matchId}/detail`);
            // API returns { data: {...}, success: true }
            if (response.data && response.data.data) {
                return response.data.data;
            }
            return null;
        } catch (error) {
            console.error('[API] Error fetching match details:', error.message);
            return null;
        }
    }

    async extractM3U8FromEmbed(embedUrl) {
        try {
            console.log(`[API] Extracting M3U8 from: ${embedUrl}`);
            
            // If it's already a vidbox stream URL, extract directly
            if (embedUrl.includes('vidbox.site/v2/stream/')) {
                console.log('[API] Direct vidbox URL detected, extracting M3U8...');
                
                // Use the stream ID to extract M3U8
                const streamId = embedUrl.split('/').pop();
                return await this.m3u8Extractor.decodeStreamId(streamId, embedUrl);
            }
            
            // Convert embed URL to watch URL
            // Example: embed might be https://livesport.su/embed/xxx
            // We need to convert to https://livesport.su/watch/xxx
            
            let watchUrl = embedUrl;
            
            // If it's an embed URL, convert to watch URL
            if (embedUrl.includes('/embed/')) {
                watchUrl = embedUrl.replace('/embed/', '/watch/');
                
                // Add default stream if not present
                if (!embedUrl.match(/\/\d+$/)) {
                    watchUrl += '/admin/1';
                }
            }
            
            // Extract M3U8 using our extractor
            const m3u8Url = await this.m3u8Extractor.extractM3U8FromStream(watchUrl);
            return m3u8Url;
        } catch (error) {
            console.error('[API] Error extracting M3U8:', error.message);
            return null;
        }
    }

    async getMatchWithStreams(matchId) {
        try {
            // Get match details
            const details = await this.getMatchDetails(matchId);
            if (!details) {
                return null;
            }

            // Extract M3U8 URLs for each stream
            const streams = [];
            
            for (const source of details.sources) {
                console.log(`\n[API] Processing stream #${source.streamNo} (${source.language})`);
                
                const m3u8Url = await this.extractM3U8FromEmbed(source.embedUrl);
                
                streams.push({
                    id: source.id,
                    streamNo: source.streamNo,
                    language: source.language,
                    hd: source.hd,
                    embedUrl: source.embedUrl,
                    watchUrl: source.embedUrl.replace('/embed/', '/watch/').replace(/(\/\d+)?$/, '/admin/1'),
                    m3u8Url: m3u8Url,
                    source: source.source
                });
                
                // Add delay to avoid rate limiting
                await new Promise(resolve => setTimeout(resolve, 1000));
            }

            return {
                match: details.match,
                streams: streams
            };
        } catch (error) {
            console.error('[API] Error getting match with streams:', error.message);
            return null;
        }
    }

    async getMultipleMatchesWithStreams(matchIds) {
        const results = [];
        
        for (let i = 0; i < matchIds.length; i++) {
            console.log(`\n[API] Processing match ${i + 1}/${matchIds.length}`);
            const result = await this.getMatchWithStreams(matchIds[i]);
            if (result) {
                results.push(result);
            }
        }
        
        return results;
    }

    async saveResults(results, filename = 'livesport-api-results.json') {
        try {
            await fs.writeFile(filename, JSON.stringify(results, null, 2));
            console.log(`\n[API] Results saved to ${filename}`);
        } catch (error) {
            console.error(`[API] Save error: ${error.message}`);
        }
    }
}

// Example usage functions
async function showLiveMatchesWithM3U8() {
    const scraper = new LivesportAPIScraper();
    
    try {
        // Get live matches
        const liveMatches = await scraper.getLiveMatches(true); // Popular only
        
        // Check if we got valid data
        if (!liveMatches || !Array.isArray(liveMatches)) {
            console.log('\n[API] No live matches found or invalid response');
            console.log('Response:', liveMatches);
            return;
        }
        
        console.log(`\n=== Found ${liveMatches.length} live matches ===\n`);
        
        // Show first 5 matches
        const matchesToProcess = liveMatches.slice(0, 5);
        
        for (const match of matchesToProcess) {
            console.log(`\n--- ${match.title} ---`);
            console.log(`Category: ${match.category}`);
            console.log(`Date: ${new Date(match.date).toLocaleString()}`);
            
            // Get streams for this match
            const matchWithStreams = await scraper.getMatchWithStreams(match.id);
            
            if (matchWithStreams && matchWithStreams.streams) {
                matchWithStreams.streams.forEach(stream => {
                    console.log(`\n  Stream #${stream.streamNo}:`);
                    console.log(`    Language: ${stream.language}`);
                    console.log(`    Quality: ${stream.hd ? 'HD' : 'SD'}`);
                    console.log(`    Source: ${stream.source}`);
                    if (stream.m3u8Url) {
                        console.log(`    ✅ M3U8: ${stream.m3u8Url}`);
                    } else {
                        console.log(`    ❌ No M3U8 found`);
                    }
                });
            }
        }
        
        // Save all results
        const allResults = await scraper.getMultipleMatchesWithStreams(
            matchesToProcess.map(m => m.id)
        );
        await scraper.saveResults(allResults, 'live-matches-with-m3u8.json');
        
    } catch (error) {
        console.error('Error:', error);
    }
}

async function showSportsAndMatches() {
    const scraper = new LivesportAPIScraper();
    
    try {
        // Get all sports
        const sports = await scraper.getSports();
        console.log('\n=== Available Sports ===\n');
        sports.forEach(sport => {
            console.log(`${sport.id}: ${sport.name}`);
        });
        
        // Get football matches
        const footballMatches = await scraper.getMatches('football');
        console.log(`\n=== Football Matches (${footballMatches.length} total) ===\n`);
        
        // Show first few
        footballMatches.slice(0, 3).forEach(match => {
            console.log(`${match.title} - ${new Date(match.date).toLocaleString()}`);
        });
        
    } catch (error) {
        console.error('Error:', error);
    }
}

// Main execution
async function main() {
    const args = process.argv.slice(2);
    
    if (args.includes('--live')) {
        await showLiveMatchesWithM3U8();
    } else if (args.includes('--sports')) {
        await showSportsAndMatches();
    } else {
        console.log('Usage:');
        console.log('  node livesport-api-scraper.js --live    # Show live matches with M3U8 URLs');
        console.log('  node livesport-api-scraper.js --sports  # Show available sports');
    }
}

// Export for use as module
module.exports = LivesportAPIScraper;

// Run if called directly
if (require.main === module) {
    main().catch(console.error);
}
