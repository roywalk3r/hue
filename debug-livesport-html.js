/**
 * Hue Stream API
 * Professional Streaming Aggregator
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */

const { getNewPage, closeBrowser } = require('./utils/browserManager');
const fs = require('fs');

async function debugHtml() {
    const url = 'https://livesport.su/watch/brighton-and-hove-albion-vs-crystal-palace-2267315/admin/1';
    console.log(`Fetching: ${url}`);

    let page;
    try {
        page = await getNewPage({
            userAgent: 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36'
        });

        const response = await page.goto(url, { waitUntil: 'domcontentloaded', timeout: 30000 });
        console.log(`Status: ${response.status()}`);

        const html = await page.content();
        fs.writeFileSync('debug_livesport.html', html);
        console.log('Saved HTML to debug_livesport.html');

        // Quick check for keywords
        console.log('Contains "sources":', html.includes('sources'));
        console.log('Contains "__sveltekit":', html.includes('__sveltekit'));
        
        // Try to match sources
        const sourcesMatch = html.match(/(?:sources|"sources"):\s*(\[.*?\])(?:,|,\}|, \}| \})/);
        console.log('Regex 1 match:', !!sourcesMatch);
        
        const simpleMatch = html.match(/sources:(\[.*?\])/);
        console.log('Regex 2 match:', !!simpleMatch);

    } catch (e) {
        console.error('Error:', e);
    } finally {
        if (page) await page.close();
        await closeBrowser();
    }
}

debugHtml();
