const { getNewPage, closeBrowser } = require('./utils/browserManager');

(async () => {
    let page;
    try {
        page = await getNewPage();

        // Inject interceptor BEFORE page loads
        await page.addInitScript(() => {
            // Wait for Angular to bootstrap then patch the service
            const patchService = () => {
                try {
                    const ng = window.ng;
                    if (!ng) return setTimeout(patchService, 100);
                    
                    const elements = document.querySelectorAll('*');
                    for (const el of elements) {
                        try {
                            const comp = ng.getComponent(el);
                            if (!comp) continue;
                            
                            // Look for a property that has GetEpisode
                            for (const key of Object.keys(comp)) {
                                const val = comp[key];
                                if (val && typeof val === 'object') {
                                    for (const k of Object.keys(val)) {
                                        if (k === 'GetEpisode' && typeof val[k] === 'function') {
                                            const orig = val[k];
                                            val[k] = function(...args) {
                                                console.log('[PATCHED] GetEpisode called with:', JSON.stringify(args));
                                                const err = new Error('caller trace');
                                                console.log('[PATCHED] Stack:', err.stack);
                                                return orig.apply(this, args);
                                            };
                                            console.log('[PATCHED] Successfully hooked GetEpisode on', key);
                                            return true;
                                        }
                                    }
                                }
                            }
                        } catch (e) {}
                    }
                    setTimeout(patchService, 500);
                } catch (e) {
                    console.log('[PATCHED] Error:', e.message);
                    setTimeout(patchService, 500);
                }
            };
            setTimeout(patchService, 500);
        });

        page.on('console', msg => {
            const text = msg.text();
            if (text.includes('[PATCHED]')) console.log(text);
        });

        await page.goto('https://kisskh.ovh/Drama/Direct/Episode-14?id=12358&ep=207851&page=0&pageSize=100', { waitUntil: 'networkidle' });
        await page.waitForTimeout(10000);

        // Also try to find the component that renders the video
        const videoComp = await page.evaluate(() => {
            const ng = window.ng;
            if (!ng) return 'no ng';
            const elements = document.querySelectorAll('*');
            for (const el of elements) {
                try {
                    const comp = ng.getComponent(el);
                    if (comp && (comp.videoUrl || comp.episodeId || comp.currentEpisode || comp.streamUrl)) {
                        return { 
                            tag: el.tagName, 
                            keys: Object.keys(comp).filter(k => typeof comp[k] !== 'function').slice(0, 20),
                            episodeId: comp.episodeId,
                            currentEpisode: comp.currentEpisode
                        };
                    }
                } catch (e) {}
            }
            return 'no video component found';
        });
        console.log('[VIDEO_COMP]', JSON.stringify(videoComp));

    } catch (e) {
        console.error('[ERROR]', e.message);
    } finally {
        if (page) await page.context().close();
        await closeBrowser();
    }
})();
