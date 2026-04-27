/**
 * Hue Stream API
 * Professional Streaming Aggregator
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */

const { getNewPage } = require('./browserManager');

/**
 * Resolves the best m3u8 URL by surfing the page.
 * Uses shared browser manager for efficiency and concurrency control.
 */
async function resolveWithPlaywright(targetUrl, referer = null) {
    let page = null;
    try {
        console.log(`[Playwright] Starting resolution for: ${targetUrl}`);
        
        // getNewPage already handles stealth, UA, and resource blocking
        page = await getNewPage({
            extraHTTPHeaders: {
                'Referer': referer || 'https://fbstreams.pm/'
            }
        });

        const foundStreams = [];
        let resolveStreamPromise; 
        const streamFoundPromise = new Promise(resolve => { resolveStreamPromise = resolve; });

        // Passive listener for m3u8 requests
        page.on('request', request => {
            const url = request.url();
            if (url.includes('.m3u8')) {
                foundStreams.push({
                    url: url,
                    headers: request.headers(),
                    priority: url.includes('4k') ? 4 : url.includes('1080') ? 3 : url.includes('720') ? 2 : 1
                });
                
                // Early exit on master/index/playlist
                if (url.includes('index.m3u8') || url.includes('master.m3u8') || url.includes('playlist.m3u8')) {
                    resolveStreamPromise(true);
                }
            }
        });

        // Navigation
        await page.goto(targetUrl, { 
            waitUntil: 'domcontentloaded', 
            timeout: 20000
        });

        // Aggressive interaction loop to trigger stream start
        const interactionSelectors = ['#player', 'video', '.play-button', 'button', 'canvas', '.ratio-16x9', 'body', 'iframe'];
        const startTime = Date.now();
        const MAX_WAIT = 10000; // 10 seconds max

        const interactionLoop = async () => {
            for (let i = 0; i < 5; i++) {
                if (foundStreams.length > 0) return;
                if (Date.now() - startTime > MAX_WAIT) return;

                const frames = page.frames();
                for (const frame of frames) {
                    if (foundStreams.length > 0) break;
                    for (const selector of interactionSelectors) {
                        try {
                            const el = await frame.$(selector);
                            if (el) {
                                // Try to click or dispatch event to start playback
                                await el.click({ timeout: 500, force: true }).catch(() => {});
                            }
                        } catch (e) {}
                    }
                }
                await page.waitForTimeout(1500);
            }
        };

        // Wait for either the loop to finish OR a stream to be found
        await Promise.race([
            interactionLoop(),
            streamFoundPromise,
            new Promise(r => setTimeout(r, MAX_WAIT + 2000))
        ]);

        if (foundStreams.length > 0) {
            // Sort by priority (quality hints)
            foundStreams.sort((a, b) => b.priority - a.priority);
            
            // Deduplicate
            const uniqueStreams = [];
            const seenUrls = new Set();
            for (const s of foundStreams) {
                if (!seenUrls.has(s.url)) {
                    seenUrls.add(s.url);
                    uniqueStreams.push({
                        url: s.url,
                        headers: s.headers,
                        quality: s.url.includes('1080') ? '1080p' : s.url.includes('720') ? '720p' : 'Auto'
                    });
                }
            }
            
            console.log(`[Playwright] Successfully captured ${uniqueStreams.length} streams for ${targetUrl}`);
            return { type: 'stream_list', streams: uniqueStreams };
        }

        console.log(`[Playwright] No m3u8 found for ${targetUrl}`);
        return { type: 'page', error: 'No m3u8 found after surfing' };

    } catch (error) {
        console.error(`[Playwright] Error resolving ${targetUrl}:`, error.message);
        return { error: error.message };
    } finally {
        if (page) {
            try { 
                // Context close releases the slot in browserManager
                await page.context().close(); 
            } catch(e) {}
        }
    }
}

module.exports = { resolveWithPlaywright };
