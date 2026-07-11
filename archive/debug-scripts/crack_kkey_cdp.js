const { chromium } = require('playwright');

(async () => {
    const browser = await chromium.launch({ headless: true });
    const context = await browser.newContext({
        userAgent: 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36'
    });
    const page = await context.newPage();
    
    const client = await page.context().newCDPSession(page);
    
    await client.send('Debugger.enable');
    await client.send('Runtime.enable');
    
    // Search for 'kkey=' in all scripts
    const scripts = await client.send('Debugger.getScriptSource', { scriptId: '1' }).catch(() => null);
    
    // Better approach: get all script IDs and search each
    const knownScripts = new Map();
    
    client.on('Debugger.scriptParsed', (params) => {
        knownScripts.set(params.scriptId, params);
    });
    
    // Navigate to page first so scripts get parsed
    await page.goto('https://kisskh.ovh/Drama/Direct/Episode-14?id=12358&ep=207851&page=0&pageSize=100', { waitUntil: 'domcontentloaded' });
    await page.waitForTimeout(2000);
    
    // Search in content for 'kkey='
    const breakpoints = [];
    for (const [scriptId, scriptInfo] of knownScripts) {
        try {
            const result = await client.send('Debugger.searchInContent', {
                scriptId,
                query: '"kkey="'
            });
            if (result.result && result.result.length > 0) {
                console.log(`[SCRIPT] ${scriptInfo.url || scriptId} has ${result.result.length} 'kkey=' occurrences`);
                for (const match of result.result) {
                    try {
                        const bp = await client.send('Debugger.setBreakpointByUrl', {
                            url: scriptInfo.url,
                            lineNumber: match.lineNumber,
                            columnNumber: match.columnNumber
                        });
                        breakpoints.push(bp);
                        console.log(`[BP] Set at ${scriptInfo.url}:${match.lineNumber}:${match.columnNumber}`);
                    } catch (e) {}
                }
            }
        } catch (e) {}
    }
    
    console.log(`[INFO] Total breakpoints: ${breakpoints.length}`);
    
    let hitCount = 0;
    client.on('Debugger.paused', async (params) => {
        hitCount++;
        console.log(`\n[BREAKPOINT HIT #${hitCount}]`);
        
        for (let i = 0; i < Math.min(params.callFrames.length, 3); i++) {
            const frame = params.callFrames[i];
            console.log(`[FRAME ${i}] ${frame.functionName || '(anon)'} @ ${frame.url}:${frame.lineNumber}:${frame.columnNumber}`);
            
            try {
                for (const scope of frame.scopeChain) {
                    if (scope.type !== 'local' && scope.type !== 'closure') continue;
                    
                    const props = await client.send('Runtime.getProperties', {
                        objectId: scope.object.objectId,
                        ownProperties: true,
                        generatePreview: false
                    });
                    
                    const interesting = props.result.filter(p => {
                        const name = p.name;
                        return name === 'Me' || name === 'kkey' || name === 'V' || name === 'J' || name === 'q' || name === 'z' ||
                               name === 'episode' || name === 'ep' || name === 'id' || name === 'hash' ||
                               name.includes('key') || name.includes('secret') || name.includes('crypto') || name.includes('sha');
                    });
                    
                    if (interesting.length > 0) {
                        console.log(`  [SCOPE ${scope.type}]:`);
                        for (const prop of interesting) {
                            const val = prop.value ? (prop.value.value || prop.value.description || JSON.stringify(prop.value)) : 'undefined';
                            console.log(`    ${prop.name}: ${String(val).substring(0, 100)}`);
                        }
                    }
                }
            } catch (e) {
                console.log(`  [ERROR] ${e.message}`);
            }
        }
        
        await client.send('Debugger.resume');
    });
    
    // Reload to trigger network requests
    await page.reload({ waitUntil: 'networkidle' });
    await page.waitForTimeout(10000);
    
    await client.send('Debugger.disable');
    await browser.close();
    console.log('[DONE]');
})();
