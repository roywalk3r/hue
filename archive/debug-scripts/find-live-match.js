/**
 * Hue Stream API
 * Professional Streaming Aggregator
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */

const { execSync } = require('child_process');

async function scan() {
    const domains = [
        'https://strm.poocloud.in',
        'https://gg.poocloud.in',
        'https://lb3.strmd.top',
        'https://lb8.strmd.top'
    ];
    
    // Slugs to test
    const slugs = [
        'pl_liv', 'pl_mci', 'pl_sun', 'pl_sat',
        'premierleague', 'liverpool', 'mancity',
        'mia-wsh', 'nba', 'nbatv',
        'event1', 'event2', 'admin', 'echo', 'alpha', 'delta', 'golf'
    ];

    const variants = [
        'tracks-v1a1/mono.ts.m3u8',
        'tracks-v2a1/mono.ts.m3u8',
        'index.m3u8',
        '1/playlist.m3u8'
    ];

    console.log('🔍 Scanning for non-promo streams...');
    const userAgent = 'Mozilla/5.0 (iPhone; CPU iPhone OS 18_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.5 Mobile/15E148 Safari/604.1';

    for (const slug of slugs) {
        for (const domain of domains) {
            for (const variant of variants) {
                const url = `${domain}/${slug}/${variant}`;
                try {
                    const cmd = `curl -L -s -m 5 -H "User-Agent: ${userAgent}" -H "Referer: https://a.embedsports.top/" "${url}"`;
                    const body = execSync(cmd).toString();
                    
                    if (body.includes('#EXTM3U')) {
                        const isPromo = body.toLowerCase().includes('ppv.to') || 
                                       body.toLowerCase().includes('promo') || 
                                       body.toLowerCase().includes('match end');
                        
                        const status = isPromo ? '🔴 PROMO' : '🟢 LIVE?';
                        console.log(`[${status}] ${url}`);
                        
                        if (!isPromo) {
                            console.log('--- MANIFEST START ---');
                            console.log(body.split('\n').slice(0, 10).join('\n'));
                            console.log('--- MANIFEST END ---\n');
                        }
                    }
                } catch (e) {
                    // ignore
                }
            }
        }
    }
}

scan();