const { getNewPage } = require('./utils/browserManager');

async function test() {
    const url = 'https://embedsports.top/embed/admin/admin-willow-cricket/1';
    const mobileUA = 'Mozilla/5.0 (Linux; Android 6.0; Nexus 5 Build/MRA58N) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/145.0.0.0 Mobile Safari/537.36';
    
    console.log(`--- Debugging with Mobile UA: ${url} ---`);
    
    const page = await getNewPage({
        userAgent: mobileUA,
        viewport: { width: 412, height: 915 },
        isMobile: true
    });

    page.on('request', request => {
        const u = request.url();
        if (u.includes('fetch') || u.includes('.m3u8') || u.includes('secure')) {
            console.log(`[Request] ${u.substring(0, 100)}...`);
        }
    });

    page.on('response', async response => {
        const u = response.url();
        if (u.includes('fetch') || u.includes('.m3u8') || u.includes('secure')) {
            console.log(`[Response] ${response.status()} | ${u.substring(0, 100)}...`);
        }
    });

    try {
        await page.goto(url, { waitUntil: 'domcontentloaded', timeout: 60000 });
        console.log('Waiting for 10 seconds...');
        await page.waitForTimeout(10000);
        
        // Try to find and click play buttons in all frames
        const frames = page.frames();
        for (const frame of frames) {
            console.log(`Frame: ${frame.url()}`);
            const selectors = ['.jw-display-icon-container', '.jw-icon-playback', 'video', '#player', 'body'];
            for (const s of selectors) {
                try {
                    const el = await frame.$(s);
                    if (el) {
                        await el.click({ timeout: 500, force: true }).catch(() => {});
                        console.log(`  Clicked ${s} in frame`);
                    }
                } catch (e) {}
            }
        }

        console.log('Waiting 20 more seconds...');
        await page.waitForTimeout(20000);
    } catch (e) {
        console.error('Error:', e.message);
    } finally {
        await page.context().close();
    }
}

test();
