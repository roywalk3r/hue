const { getNewPage, closeBrowser } = require('./utils/browserManager');

(async () => {
    let page;
    try {
        page = await getNewPage();

        // Inject interceptor to trace kkey origin
        await page.addInitScript(() => {
            const origFetch = window.fetch;
            window.fetch = async function(...args) {
                const url = args[0];
                if (typeof url === 'string' && url.includes('kkey=')) {
                    const err = new Error('kkey trace');
                    console.log('[KKEY_TRACE]', url);
                    console.log('[KKEY_STACK]', err.stack);
                }
                return origFetch.apply(this, args);
            };
            
            const origXHR = window.XMLHttpRequest.prototype.open;
            window.XMLHttpRequest.prototype.open = function(method, url, ...rest) {
                if (typeof url === 'string' && url.includes('kkey=')) {
                    const err = new Error('kkey trace xhr');
                    console.log('[KKEY_TRACE_XHR]', url);
                    console.log('[KKEY_STACK_XHR]', err.stack);
                }
                return origXHR.call(this, method, url, ...rest);
            };
        });

        page.on('console', msg => {
            const text = msg.text();
            if (text.includes('[KKEY_TRACE') || text.includes('kkey')) {
                console.log(text);
            }
        });

        await page.goto('https://kisskh.ovh/Drama/Direct/Episode-14?id=12358&ep=207851&page=0&pageSize=100', { waitUntil: 'networkidle' });
        await page.waitForTimeout(8000);

        // Try to find Angular service
        const angularService = await page.evaluate(() => {
            try {
                const ng = window.ng;
                if (!ng) return null;
                const elements = document.querySelectorAll('*');
                for (const el of elements) {
                    const comp = ng.getComponent(el);
                    if (comp && comp.videoService) {
                        return { found: 'videoService', keys: Object.keys(comp.videoService) };
                    }
                    if (comp && comp.dramaService) {
                        return { found: 'dramaService', keys: Object.keys(comp.dramaService) };
                    }
                    if (comp && comp.apiService) {
                        return { found: 'apiService', keys: Object.keys(comp.apiService) };
                    }
                }
                return { found: 'none', ngVersion: ng.VERSION?.full };
            } catch (e) {
                return { error: e.message };
            }
        });
        console.log('[ANGULAR]', JSON.stringify(angularService));

    } catch (e) {
        console.error('[ERROR]', e.message);
    } finally {
        if (page) await page.context().close();
        await closeBrowser();
    }
})();
