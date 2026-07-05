/**
 * Hue Stream API
 * Professional Streaming Aggregator
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */

const { getNewContext } = require('./browserManager');
const { config, saveConfigPatch } = require('./config');

const BACKUP_DOMAINS = ['moviebox.ph', 'moviebox.id', 'moviebox.pk', 'movieboxapp.in'];

async function refreshMovieboxCookie() {
    console.log('[MovieBox Refresh] Starting reliable refresh...');
    const primaryHost = config.movieboxHost || 'moviebox.ph';
    const hostsToTry = [primaryHost, ...BACKUP_DOMAINS.filter(d => d !== primaryHost)];

    for (const host of hostsToTry) {
        let context;
        try {
            const url = `https://${host}`;
            console.log(`[MovieBox Refresh] Attempting ${url}...`);
            context = await getNewContext();
            const page = await context.newPage();

            // Allow essential types for cookie setting
            await page.route('**/*', (route) => {
                const type = route.request().resourceType();
                if (['document', 'script', 'xhr', 'fetch'].includes(type)) {
                    return route.continue();
                }
                return route.abort();
            });

            await page.goto(url, { waitUntil: 'networkidle', timeout: 60000 });
            await page.waitForTimeout(10000);

            const cookies = await context.cookies();
            const mbTokenCookie = cookies.find(c => c.name === 'mb_token');
            const uuidCookie = cookies.find(c => c.name === 'uuid');
            // The `token` cookie is the account/session grant. It used to be the
            // same value as mb_token, but MovieBox now issues a distinct token
            // and it is what actually unlocks `hasResource` on the play endpoint
            // (a days-old mb_token goes stale server-side even though its JWT
            // exp is ~90 days). Capture it so the Rust client can send it.
            const authTokenCookie = cookies.find(c => c.name === 'token');

            if (uuidCookie || mbTokenCookie || authTokenCookie) {
                let cookieArr = [];
                if (authTokenCookie) cookieArr.push(`token=${authTokenCookie.value}`);
                if (mbTokenCookie) cookieArr.push(`mb_token=${mbTokenCookie.value}`);
                if (uuidCookie) cookieArr.push(`uuid=${uuidCookie.value}`);
                const cookieString = cookieArr.join('; ');
                const userAgent = await page.evaluate(() => navigator.userAgent);

                saveConfigPatch({ 
                    movieboxCookie: cookieString,
                    movieboxUserAgent: userAgent,
                    movieboxHost: host
                });
                console.log(`[MovieBox Refresh] Success with ${host}.`);
                return true;
            }
        } catch (error) {
            console.error(`[MovieBox Refresh] Error with ${host}: ${error.message}`);
        } finally {
            if (context) await context.close();
        }
    }
    return false;
}

if (require.main === module) {
    refreshMovieboxCookie().then(success => process.exit(success ? 0 : 1));
}

module.exports = { refreshMovieboxCookie };
