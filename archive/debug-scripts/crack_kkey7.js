const { getNewPage, closeBrowser } = require('./utils/browserManager');

(async () => {
    const kkeys = [];
    for (let i = 0; i < 3; i++) {
        const page = await getNewPage();
        let kkey = null;
        
        page.on('request', req => {
            const url = req.url();
            if (url.includes('/api/DramaList/Episode/207851') && url.includes('kkey=')) {
                kkey = new URL(url).searchParams.get('kkey');
            }
        });
        
        await page.goto('https://kisskh.ovh/Drama/Direct/Episode-14?id=12358&ep=207851&page=0&pageSize=100', { waitUntil: 'networkidle' });
        await page.waitForTimeout(3000);
        await page.context().close();
        
        kkeys.push(kkey);
        console.log(`Run ${i + 1}: ${kkey ? kkey.substring(0, 40) : 'null'}...`);
    }
    
    await closeBrowser();
    
    console.log('\nSame kkey across runs?', kkeys[0] === kkeys[1] && kkeys[1] === kkeys[2]);
    
    // Also try a different episode
    const page2 = await getNewPage();
    let kkey2 = null;
    page2.on('request', req => {
        const url = req.url();
        if (url.includes('/api/DramaList/Episode/207852') && url.includes('kkey=')) {
            kkey2 = new URL(url).searchParams.get('kkey');
        }
    });
    await page2.goto('https://kisskh.ovh/Drama/Direct/Episode-15?id=12358&ep=207852&page=0&pageSize=100', { waitUntil: 'networkidle' });
    await page2.waitForTimeout(3000);
    await page2.context().close();
    
    console.log('Ep 207851 vs 207852 different?', kkeys[0] !== kkey2);
})();
