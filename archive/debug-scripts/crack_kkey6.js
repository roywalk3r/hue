const { chromium } = require('playwright');

(async () => {
    const browser = await chromium.launch({ headless: true });
    const context = await browser.newContext({
        userAgent: 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36'
    });
    const page = await context.newPage();
    
    const client = await page.context().newCDPSession(page);
    await client.send('Debugger.enable');
    
    // Set a breakpoint on XHR open for kkey URLs
    await client.send('Runtime.evaluate', {
        expression: `
            (function() {
                const orig = XMLHttpRequest.prototype.open;
                XMLHttpRequest.prototype.open = function(method, url) {
                    if (typeof url === 'string' && url.includes('kkey=')) {
                        debugger;
                    }
                    return orig.apply(this, arguments);
                };
            })();
        `
    });
    
    client.on('Debugger.paused', async (params) => {
        console.log('[BREAKPOINT] Hit!');
        console.log('[BREAKPOINT] Call frames:', params.callFrames.length);
        
        // Get local variables from each frame
        for (let i = 0; i < Math.min(params.callFrames.length, 5); i++) {
            const frame = params.callFrames[i];
            console.log(`[FRAME ${i}] ${frame.functionName || '(anonymous)'} at ${frame.url}:${frame.lineNumber}:${frame.columnNumber}`);
            
            try {
                const scopeChain = frame.scopeChain;
                for (const scope of scopeChain.slice(0, 3)) {
                    if (scope.type === 'local' || scope.type === 'closure') {
                        const props = await client.send('Runtime.getProperties', {
                            objectId: scope.object.objectId,
                            ownProperties: true
                        });
                        
                        const interesting = props.result.filter(p => 
                            p.name.includes('kkey') || p.name.includes('episode') || p.name.includes('ep') || 
                            p.name.includes('Me') || p.name.includes('V') || p.name.includes('key') || 
                            p.name === 'err' || p.name === 'ts' || p.name === 'time'
                        );
                        
                        if (interesting.length > 0) {
                            console.log(`  [SCOPE ${scope.type}]`);
                            for (const prop of interesting) {
                                const val = prop.value ? (prop.value.value || prop.value.description || '...') : 'undefined';
                                console.log(`    ${prop.name}: ${val}`);
                            }
                        }
                    }
                }
            } catch (e) {
                console.log(`  [ERROR getting scope]: ${e.message}`);
            }
        }
        
        await client.send('Debugger.resume');
    });
    
    await page.goto('https://kisskh.ovh/Drama/Direct/Episode-14?id=12358&ep=207851&page=0&pageSize=100', { waitUntil: 'networkidle' });
    await page.waitForTimeout(10000);
    
    await client.send('Debugger.disable');
    await browser.close();
})();
