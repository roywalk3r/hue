/**
 * Hue Stream API
 * Professional Streaming Aggregator
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */

const { getNewContext } = require('./browserManager');
const { saveConfigPatch } = require('./config');

const SFLIX_URL = 'https://sflix.film';

async function refreshSflixCookie() {
    console.log('[SFlix Refresh] Starting reliable refresh...');
    let context;
    try {
        context = await getNewContext();
        const page = await context.newPage();

        // Allow scripts and documents, block images/fonts
        await page.route('**/*', (route) => {
            const type = route.request().resourceType();
            if (['document', 'script', 'xhr', 'fetch'].includes(type)) {
                return route.continue();
            }
            return route.abort();
        });

        console.log(`[SFlix Refresh] Navigating to ${SFLIX_URL}...`);
        await page.goto(SFLIX_URL, { waitUntil: 'networkidle', timeout: 90000 });
        
        // Give scripts time to set cookies
        await page.waitForTimeout(10000);

        const cookies = await context.cookies();
        const tokenCookie = cookies.find(c => c.name === 'sflix_token');
        
        if (!tokenCookie) {
            console.error('[SFlix Refresh] Failed: sflix_token not found after waiting.');
            return false;
        }

        const cookieString = `sflix_token=${tokenCookie.value}; sflix_i18n_lang=en`;
        const userAgent = await page.evaluate(() => navigator.userAgent);

        saveConfigPatch({ sflixCookie: cookieString, sflixUserAgent: userAgent });
        console.log('[SFlix Refresh] Success.');
        return true;

    } catch (error) {
        console.error(`[SFlix Refresh] Error: ${error.message}`);
        return false;
    } finally {
        if (context) await context.close();
    }
}

if (require.main === module) {
    refreshSflixCookie().then(success => process.exit(success ? 0 : 1));
}

module.exports = { refreshSflixCookie };
