#!/usr/bin/env node
/**
 * Livesport Watch URL Scraper
 * Given a watch URL, extracts all streams and their M3U8 URLs
 */

const LivesportM3U8Extractor = require('./livesport-m3u8-extractor');
const fs = require('fs').promises;

class LivesportWatchScraper {
    constructor() {
        this.extractor = new LivesportM3U8Extractor();
    }

    async scrapeWatchUrl(watchUrl) {
        try {
            console.log(`\n🎯 Scraping: ${watchUrl}`);
            console.log('=' .repeat(50));
            
            // Extract all streams from the watch page
            const result = await this.extractor.extractAllStreams(watchUrl);
            
            if (result.error) {
                console.error('❌ Error:', result.error);
                return null;
            }
            
            // Display results
            console.log(`\n📺 Title: ${result.title}`);
            console.log(`🔢 Total Streams: ${result.metadata.totalStreams}\n`);
            
            console.log('📋 AVAILABLE STREAMS:');
            console.log('-'.repeat(50));
            
            let m3u8Count = 0;
            result.streams.forEach((stream, index) => {
                console.log(`\n${index + 1}. ${stream.title}`);
                console.log(`   📂 Category: ${stream.category}`);
                console.log(`   🎥 Quality: ${stream.quality}`);
                console.log(`   👥 Viewers: ${stream.viewers || 'N/A'}`);
                console.log(`   🌐 Language: ${stream.language}`);
                console.log(`   🔗 Watch URL: ${stream.url}`);
                
                if (stream.m3u8Url) {
                    console.log(`   ✅ M3U8: ${stream.m3u8Url}`);
                    m3u8Count++;
                } else {
                    console.log(`   ❌ No M3U8 found`);
                }
                
                console.log(`   ⭐ Current: ${stream.isCurrent ? 'Yes' : 'No'}`);
            });
            
            console.log('\n' + '='.repeat(50));
            console.log(`✅ Successfully extracted ${m3u8Count}/${result.metadata.totalStreams} M3U8 URLs`);
            
            return result;
            
        } catch (error) {
            console.error('❌ Fatal error:', error.message);
            return null;
        }
    }

    async saveJson(data, filename) {
        try {
            await fs.writeFile(filename, JSON.stringify(data, null, 2));
            console.log(`\n💾 Saved to: ${filename}`);
        } catch (error) {
            console.error('❌ Save error:', error.message);
        }
    }

    async saveM3U8List(streams, filename) {
        try {
            const m3u8Urls = streams
                .filter(s => s.m3u8Url)
                .map(s => `${s.title} (${stream.category}): ${s.m3u8Url}`)
                .join('\n');
            
            await fs.writeFile(filename, m3u8Urls);
            console.log(`💾 M3U8 list saved to: ${filename}`);
        } catch (error) {
            console.error('❌ Save error:', error.message);
        }
    }
}

// Command line interface
async function main() {
    const args = process.argv.slice(2);
    
    if (args.length === 0) {
        console.log(`
🎯 Livesport Watch URL Scraper

Usage:
  node livesport-watch-scraper.js <watch-url>

Example:
  node livesport-watch-scraper.js https://livesport.su/watch/ppv-world-darts-championship
  node livesport-watch-scraper.js https://livesport.su/watch/sunderland-vs-leeds-united-2267251/admin/1

Output:
  - Displays all streams with their M3U8 URLs
  - Saves JSON result to <match-id>-streams.json
  - Saves M3U8 list to <match-id>-m3u8.txt
        `);
        return;
    }
    
    const watchUrl = args[0];
    
    // Validate URL
    if (!watchUrl.includes('livesport.su/watch/')) {
        console.error('❌ Invalid URL. Please provide a valid livesport.su/watch URL');
        return;
    }
    
    const scraper = new LivesportWatchScraper();
    const result = await scraper.scrapeWatchUrl(watchUrl);
    
    if (result) {
        // Generate filename from URL
        const matchId = watchUrl.split('/watch/')[1]?.split('/')[0] || 'unknown';
        const timestamp = new Date().toISOString().replace(/[:.]/g, '-');
        
        // Save results
        await scraper.saveJson(result, `${matchId}-streams-${timestamp}.json`);
        await scraper.saveM3U8List(result.streams, `${matchId}-m3u8-${timestamp}.txt`);
    }
}

// Export for use as module
module.exports = LivesportWatchScraper;

// Run if called directly
if (require.main === module) {
    main().catch(console.error);
}
