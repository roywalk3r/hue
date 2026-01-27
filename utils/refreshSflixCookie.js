const { getNewContext } = require('./browserManager');
const { saveConfigPatch } = require('./config');

const SFLIX_URL = 'https://sflix.film';

async function refreshSflixCookie() {
    console.log('[SFlix Refresh] Starting cookie refresh sequence...');
    let context;
    let page;

    try {
        context = await getNewContext();
        page = await context.newPage();

        // Navigate to home page to generate cookies
        console.log(`[SFlix Refresh] Navigating to ${SFLIX_URL}...`);
        await page.goto(SFLIX_URL, { waitUntil: 'domcontentloaded', timeout: 60000 });
        
        await page.waitForTimeout(5000);

        // Get Cookies
        const cookies = await context.cookies();
        
        const tokenCookie = cookies.find(c => c.name === 'sflix_token');
        const accountCookie = cookies.find(c => c.name === 'account');
        const uuidCookie = cookies.find(c => c.name === 'uuid');
        
        if (!tokenCookie) {
            console.error('[SFlix Refresh] Failed: sflix_token cookie not found.');
            return false;
        }

        let cookieString = `sflix_token=${tokenCookie.value}; sflix_i18n_lang=en`;
        if (accountCookie) cookieString += `; account=${accountCookie.value}`;
        if (uuidCookie) cookieString += `; uuid=${uuidCookie.value}`;

        // Get User-Agent
        const userAgent = await page.evaluate(() => navigator.userAgent);
        console.log(`[SFlix Refresh] Captured UA: ${userAgent}`);

        // Save both Cookie AND User-Agent to config
        const success = saveConfigPatch({ 
            sflixCookie: cookieString,
            sflixUserAgent: userAgent
        });
        
        if (success) {
            console.log('[SFlix Refresh] Config updated successfully.');
            return true;
        } else {
            console.error('[SFlix Refresh] Failed to save config.');
            return false;
        }

    } catch (error) {
        console.error(`[SFlix Refresh] Error: ${error.message}`);
        return false;
    } finally {
        if (context) await context.close();
    }
}

if (require.main === module) {
    refreshSflixCookie().then(success => {
        if (!success) process.exit(1);
    });
}

module.exports = { refreshSflixCookie };
