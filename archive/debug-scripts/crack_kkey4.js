const { getNewPage, closeBrowser } = require('./utils/browserManager');

(async () => {
    let page;
    try {
        page = await getNewPage();

        await page.addInitScript(() => {
            const orig = window.XMLHttpRequest.prototype.open;
            window.XMLHttpRequest.prototype.open = function(method, url) {
                if (typeof url === 'string' && url.includes('kkey=')) {
                    const err = new Error('KKEY_BREAKPOINT');
                    // Find the first frame that contains main.js to get the caller function
                    const frames = err.stack.split('\n');
                    const callerFrame = frames.find(f => f.includes('main.c582432abad915cc.js') && !f.includes('XMLHttpRequest'));
                    console.log('[KKEY_CALLER]', callerFrame);
                    
                    // Try to extract the function name from the stack
                    for (const frame of frames) {
                        if (frame.includes('main.c582432abad915cc.js') && frame.includes('at ')) {
                            console.log('[KKEY_FRAME]', frame.trim());
                        }
                    }
                }
                return orig.apply(this, arguments);
            };
        });

        page.on('console', msg => {
            const text = msg.text();
            if (text.includes('[KKEY_')) console.log(text);
        });

        await page.goto('https://kisskh.ovh/Drama/Direct/Episode-14?id=12358&ep=207851&page=0&pageSize=100', { waitUntil: 'networkidle' });
        await page.waitForTimeout(8000);

        // Now try to find the kkey in localStorage/sessionStorage or any global variables
        const globals = await page.evaluate(() => {
            const results = [];
            for (const key of Object.keys(window)) {
                try {
                    const val = window[key];
                    if (typeof val === 'string' && val.length === 128 && /^[0-9A-F]+$/i.test(val)) {
                        results.push({ key, value: val.substring(0, 20) + '...', length: val.length });
                    }
                } catch (e) {}
            }
            return results;
        });
        console.log('[GLOBALS]', JSON.stringify(globals));

    } catch (e) {
        console.error('[ERROR]', e.message);
    } finally {
        if (page) await page.context().close();
        await closeBrowser();
    }
})();
