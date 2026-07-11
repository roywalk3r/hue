const { getNewPage } = require('./utils/browserManager');
const { closeBrowser } = require('./utils/browserManager');

(async () => {
    let page;
    try {
        page = await getNewPage();
        const context = page.context();

        let kkey = null;
        let apiUrl = null;

        page.on('request', req => {
            const url = req.url();
            if (url.includes('/api/DramaList/Episode/') && url.includes('kkey=')) {
                apiUrl = url;
                const u = new URL(url);
                kkey = u.searchParams.get('kkey');
                console.log('[CAPTURED] kkey:', kkey);
                console.log('[CAPTURED] ts:', u.searchParams.get('ts'));
                console.log('[CAPTURED] time:', u.searchParams.get('time'));
                console.log('[CAPTURED] episode:', url.match(/Episode\/(\d+)/)?.[1]);
            }
        });

        await page.goto('https://kisskh.ovh/Drama/Direct/Episode-14?id=12358&ep=207851&page=0&pageSize=100', { waitUntil: 'networkidle' });
        await page.waitForTimeout(5000);

        if (!kkey) {
            console.log('[WARN] No kkey captured. Reloading...');
            await page.reload({ waitUntil: 'networkidle' });
            await page.waitForTimeout(5000);
        }

        // Search page scripts for kkey generation
        const scriptSources = await page.evaluate(() => {
            const scripts = Array.from(document.querySelectorAll('script'));
            return scripts.map((s, i) => {
                const text = s.innerText || s.textContent || '';
                if (text.includes('kkey') || text.includes('GetEpisode')) {
                    return { index: i, text: text.substring(0, 3000) };
                }
                return null;
            }).filter(Boolean);
        });
        
        console.log(`[INFO] Found ${scriptSources.length} inline scripts mentioning kkey.`);
        scriptSources.forEach(s => {
            console.log(`--- Script ${s.index} ---`);
            console.log(s.text);
        });

    } catch (e) {
        console.error('[ERROR]', e.message);
    } finally {
        if (page) await page.context().close();
        await closeBrowser();
    }
})();
