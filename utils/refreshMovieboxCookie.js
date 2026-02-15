const { getNewContext } = require('./browserManager');
const { config, saveConfigPatch } = require('./config');

const BACKUP_DOMAINS = ['moviebox.ph', 'moviebox.id', 'moviebox.pk', 'movieboxapp.in'];

async function refreshMovieboxCookie() {
    console.log('[MovieBox Refresh] Starting cookie refresh sequence...');
    
    // Try the configured host first, then fall back to others
    const primaryHost = config.movieboxHost || 'moviebox.ph';
    const hostsToTry = [primaryHost, ...BACKUP_DOMAINS.filter(d => d !== primaryHost)];

    for (const host of hostsToTry) {
        let context;
        try {
            const url = `https://${host}`;
            console.log(`[MovieBox Refresh] Attempting ${url}...`);
            
            context = await getNewContext();
            const page = await context.newPage();

            await page.goto(url, { waitUntil: 'domcontentloaded', timeout: 45000 });
            await page.waitForTimeout(5000);

            const cookies = await context.cookies();
            const tokenCookie = cookies.find(c => c.name === 'mb_token');
            const accountCookie = cookies.find(c => c.name === 'account');
            const uuidCookie = cookies.find(c => c.name === 'uuid');
            
            // Prefer uuid if mb_token is not available, but capture both
            if (uuidCookie || tokenCookie) {
                let cookieArr = [];
                if (tokenCookie) cookieArr.push(`mb_token=${tokenCookie.value}`);
                if (uuidCookie) cookieArr.push(`uuid=${uuidCookie.value}`);
                if (accountCookie) cookieArr.push(`account=${accountCookie.value}`);

                const cookieString = cookieArr.join('; ');
                const userAgent = await page.evaluate(() => navigator.userAgent);
                console.log(`[MovieBox Refresh] Success with ${host}. Captured UA: ${userAgent}`);

                saveConfigPatch({ 
                    movieboxCookie: cookieString,
                    movieboxUserAgent: userAgent,
                    movieboxHost: host
                });
                
                await context.close();
                return true;
            } else {
                console.warn(`[MovieBox Refresh] Essential cookies not found on ${host}`);
            }
        } catch (error) {
            console.error(`[MovieBox Refresh] Error with ${host}: ${error.message}`);
        } finally {
            if (context) await context.close();
        }
    }

    console.error('[MovieBox Refresh] All domains failed.');
    return false;
}

if (require.main === module) {
    refreshMovieboxCookie().then(success => {
        if (!success) process.exit(1);
    });
}

module.exports = { refreshMovieboxCookie };
