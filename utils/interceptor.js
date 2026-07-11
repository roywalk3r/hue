/**
 * Hue Stream API
 * Professional Streaming Aggregator
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */

/**
 * Shared Request Interceptor & Resource Blocker
 * Standardizes ad-blocking and stream capturing across all browser providers.
 */

// Resources to always block to save CPU/Bandwidth
const BLOCKED_TYPES = ['image', 'font', 'stylesheet', 'other'];
const AD_DOMAINS = [
    'analytics', 'doubleclick', 'google-analytics', 'adsystem', 
    'carbon', 'popads', 'popcash', 'histats', 'acscdn', 
    'googletagmanager', 'facebook', 'twitter'
];

/**
 * Attaches standard blocking rules to a Playwright page.
 * @param {import('playwright').Page} page 
 */
async function attachBlocker(page) {
    await page.route('**/*', (route) => {
        const type = route.request().resourceType();
        const url = route.request().url();
        
        // Allow streams/media
        if (url.includes('.m3u8') || url.includes('.mp4') || url.includes('.ts') || url.includes('workers.dev')) {
            return route.continue();
        }

        // Block by resource type
        if (BLOCKED_TYPES.includes(type)) {
            return route.abort();
        }

        // Block by domain pattern
        if (AD_DOMAINS.some(d => url.includes(d))) {
            return route.abort();
        }

        route.continue();
    });
}

/**
 * Standard stream capture filter.
 * @param {string} url 
 * @returns {boolean} True if the URL looks like a valid stream.
 */
function isStreamUrl(url) {
    const u = url.toLowerCase();
    // Exclude subtitles
    if (u.includes('.vtt') || u.includes('.srt') || u.includes('/subtitle/')) return false;
    
    // Check for video extensions
    return u.includes('.m3u8') || u.includes('.mp4') || u.includes('master.m3u8') || u.includes('index.m3u8');
}

module.exports = {
    attachBlocker,
    isStreamUrl,
    AD_DOMAINS,
    BLOCKED_TYPES
};
