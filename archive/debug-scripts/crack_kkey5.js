const { getNewPage, closeBrowser } = require('./utils/browserManager');
const crypto = require('crypto');

async function getKkeyForEpisode(episodeId) {
    const page = await getNewPage();
    let kkey = null;
    
    page.on('request', req => {
        const url = req.url();
        if (url.includes(`/api/DramaList/Episode/${episodeId}`) && url.includes('kkey=')) {
            kkey = new URL(url).searchParams.get('kkey');
        }
    });

    await page.goto(`https://kisskh.ovh/Drama/Direct/Episode-1?id=12358&ep=${episodeId}&page=0&pageSize=100`, { waitUntil: 'networkidle' });
    await page.waitForTimeout(5000);
    await page.context().close();
    return kkey;
}

(async () => {
    // Capture kkeys for multiple episodes
    const episodes = [207851, 207852, 207853, 207854, 207855];
    const results = [];
    
    for (const ep of episodes) {
        const kkey = await getKkeyForEpisode(ep);
        results.push({ episode: ep, kkey });
        console.log(`Episode ${ep}: ${kkey}`);
    }
    
    await closeBrowser();
    
    // Analyze patterns
    console.log('\n--- Analysis ---');
    const secretsToTry = ['kisskh', 'kisskh.ovh', 'kisskh2024', 'kisskh2025', 'secret', 'key', 'api', 'dramalist'];
    
    for (const { episode, kkey } of results) {
        if (!kkey) continue;
        const epStr = String(episode);
        
        // Try various hash combinations
        for (const secret of secretsToTry) {
            const h1 = crypto.createHmac('sha512', secret).update(epStr).digest('hex').toUpperCase();
            const h2 = crypto.createHmac('sha512', secret).update(epStr + '.png').digest('hex').toUpperCase();
            const h3 = crypto.createHmac('sha512', secret).update(epStr + '.png?err=false&ts=null&time=null').digest('hex').toUpperCase();
            const h4 = crypto.createHash('sha512').update(epStr + secret).digest('hex').toUpperCase();
            const h5 = crypto.createHash('sha512').update(secret + epStr).digest('hex').toUpperCase();
            
            if (h1 === kkey) console.log(`MATCH! HMAC-SHA512('${secret}', '${epStr}')`);
            if (h2 === kkey) console.log(`MATCH! HMAC-SHA512('${secret}', '${epStr}.png')`);
            if (h3 === kkey) console.log(`MATCH! HMAC-SHA512('${secret}', full query)`);
            if (h4 === kkey) console.log(`MATCH! SHA512('${epStr}${secret}')`);
            if (h5 === kkey) console.log(`MATCH! SHA512('${secret}${epStr}')`);
        }
        
        // Also try with no secret - pure hash
        const pure = crypto.createHash('sha512').update(epStr).digest('hex').toUpperCase();
        if (pure === kkey) console.log(`MATCH! Pure SHA512 of episode ID`);
    }
})();
