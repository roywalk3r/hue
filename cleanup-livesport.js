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
 * Cleanup and organize Livesport scraper files
 */

const fs = require('fs');
const path = require('path');

// Files to keep
const keepFiles = [
    'livesport-watch-scraper.js',
    'livesport-m3u8-extractor.js',
    'livesport-api-scraper.js',
    'm3u8-proxy-v2.js',
    'cloudflare-worker-proxy.js',
    'test-m3u8.js',
    'test-livesport-api.js',
    'LIVESPORT-README.md',
    'package.json',
    'cleanup-livesport.js'
];

// Files to remove (old versions)
const removeFiles = [
    'livesport-scraper.js',
    'livesport-scraper-puppeteer.js',
    'livesport-scraper-v2.js',
    'livesport-scraper-final.js',
    'livesport-scraper.py',
    'requirements.txt',
    'test-scraper.js',
    'extract-m3u8.js',
    'm3u8-proxy.js',
    'livesport-m3u8-results.json',
    'test-results-v2.json',
    'test-results-final.json',
    'ppv-world-darts-championship-streams-*.json',
    'ppv-world-darts-championship-m3u8-*.txt'
];

console.log('🧹 Cleaning up Livesport scraper files...\n');

// List files to remove
console.log('Files to remove:');
removeFiles.forEach(file => {
    console.log(`  - ${file}`);
});

console.log('\nFiles to keep:');
keepFiles.forEach(file => {
    if (fs.existsSync(file)) {
        console.log(`  ✓ ${file}`);
    }
});

// Ask for confirmation
process.stdout.write('\nRemove old files? (y/N): ');
process.stdin.resume();
process.stdin.setEncoding('utf8');
process.stdin.on('data', async (input) => {
    if (input.trim().toLowerCase() === 'y') {
        console.log('\nRemoving old files...');
        
        for (const file of removeFiles) {
            // Handle wildcards
            if (file.includes('*')) {
                const pattern = file.replace(/\*/g, '.*');
                const files = fs.readdirSync('.').filter(f => new RegExp(pattern).test(f));
                for (const f of files) {
                    try {
                        fs.unlinkSync(f);
                        console.log(`  Removed: ${f}`);
                    } catch (e) {
                        console.log(`  Skipped: ${f} (${e.message})`);
                    }
                }
            } else {
                try {
                    if (fs.existsSync(file)) {
                        fs.unlinkSync(file);
                        console.log(`  Removed: ${file}`);
                    }
                } catch (e) {
                    console.log(`  Skipped: ${file} (${e.message})`);
                }
            }
        }
        
        console.log('\n✅ Cleanup complete!');
        console.log('\nProject is now organized. See LIVESPORT-README.md for usage.');
    } else {
        console.log('\n❌ Cleanup cancelled.');
    }
    
    process.exit(0);
});
