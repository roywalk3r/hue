const { getNewContext } = require('./browserManager');
const { config, saveConfigPatch } = require('./config');

const BACKUP_DOMAINS = ['moviebox.ph', 'moviebox.id', 'moviebox.pk', 'movieboxapp.in'];

async function refreshMovieboxCookie() {
    console.log('[MovieBox Refresh] Starting ultra-fast refresh...');
    const primaryHost = config.movieboxHost || 'moviebox.ph';
    const hostsToTry = [primaryHost, ...BACKUP_DOMAINS.filter(d => d !== primaryHost)];

    for (const host of hostsToTry) {
        let context;
        try {
            const url = `https://${host}`;
            console.log(`[MovieBox Refresh] Attempting ${url}...`);
            context = await getNewContext();
            const page = await context.newPage();

            // BLOCK EVERYTHING except the document
            await page.route('**/*', (route) => {
                const type = route.request().resourceType();
                return type === 'document' ? route.continue() : route.abort();
            });

            await page.goto(url, { waitUntil: 'commit', timeout: 60000 });
            await page.waitForTimeout(5000);

            const cookies = await context.cookies();
            const tokenCookie = cookies.find(c => c.name === 'mb_token');
            const uuidCookie = cookies.find(c => c.name === 'uuid');
            
            if (uuidCookie || tokenCookie) {
                let cookieArr = [];
                if (tokenCookie) cookieArr.push(`mb_token=${tokenCookie.value}`);
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
