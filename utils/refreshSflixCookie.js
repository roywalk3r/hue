const { getNewContext } = require('./browserManager');
const { saveConfigPatch } = require('./config');

const SFLIX_URL = 'https://sflix.film';

async function refreshSflixCookie() {
    console.log('[SFlix Refresh] Starting ultra-fast refresh...');
    let context;
    try {
        context = await getNewContext();
        const page = await context.newPage();

        // BLOCK EVERYTHING except the document to make it instant
        await page.route('**/*', (route) => {
            const type = route.request().resourceType();
            return type === 'document' ? route.continue() : route.abort();
        });

        console.log(`[SFlix Refresh] Navigating to ${SFLIX_URL}...`);
        await page.goto(SFLIX_URL, { waitUntil: 'commit', timeout: 60000 });
        
        await page.waitForTimeout(5000);

        const cookies = await context.cookies();
        const tokenCookie = cookies.find(c => c.name === 'sflix_token');
        
        if (!tokenCookie) {
            console.error('[SFlix Refresh] Failed: sflix_token not found.');
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
