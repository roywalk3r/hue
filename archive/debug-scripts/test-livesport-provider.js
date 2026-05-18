#!/usr/bin/env node

/**
 * Hue Stream API
 * Professional Streaming Aggregator
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */

/**
 * Test Livesport provider with unique M3U8 extraction
 */

const LivesportWatchScraper = require('./livesport-watch-scraper');

async function testProvider() {
    const scraper = new LivesportWatchScraper();
    const watchUrl = 'https://livesport.su/watch/ppv-world-darts-championship';
    
    console.log(`Testing: ${watchUrl}`);
    console.log('='.repeat(60));
    
    const result = await scraper.scrapeWatchUrl(watchUrl);
    
    if (!result || result.error) {
        console.error('Failed to scrape:', result?.error);
        return;
    }
    
    console.log(`\nFound ${result.streams.length} streams:\n`);
    
    const m3u8Urls = new Set();
    
    result.streams.forEach((stream, index) => {
        console.log(`${index + 1}. ${stream.title}`);
        console.log(`   Source: ${stream.category}`);
        console.log(`   Quality: ${stream.quality}`);
        console.log(`   Viewers: ${stream.viewers}`);
        console.log(`   Stream URL: ${stream.url}`);
        console.log(`   M3U8 URL: ${stream.m3u8Url || 'NOT FOUND'}`);
        
        if (stream.m3u8Url) {
            if (m3u8Urls.has(stream.m3u8Url)) {
                console.log(`   ⚠️  DUPLICATE M3U8 URL!`);
            } else {
                m3u8Urls.add(stream.m3u8Url);
                console.log(`   ✅ Unique M3U8`);
            }
        }
        console.log('');
    });
    
    console.log(`\nSummary:`);
    console.log(`- Total streams: ${result.streams.length}`);
    console.log(`- Unique M3U8 URLs: ${m3u8Urls.size}`);
    console.log(`- Duplicate ratio: ${((result.streams.length - m3u8Urls.size) / result.streams.length * 100).toFixed(1)}%`);
}

testProvider().catch(console.error);
