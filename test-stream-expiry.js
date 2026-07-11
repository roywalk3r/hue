const moviebox = require('./providers/moviebox');
const sflix = require('./providers/sflix');
const vixsrc = require('./providers/vixsrc');

function inspectUrl(url, provider) {
    try {
        const u = new URL(url);
        const params = {};
        for (const [k, v] of u.searchParams) {
            params[k] = v.length > 30 ? v.slice(0, 30) + '...' : v;
        }
        const hasExpiry = ['expires', 'exp', 'e', 'token', 'sign', 'ts', 't', 'auth'].some(k => u.searchParams.has(k));
        console.log(`\n[${provider}] URL: ${u.origin}${u.pathname}`);
        console.log(`[${provider}] Query params:`, JSON.stringify(params, null, 2));
        console.log(`[${provider}] Has expiry-like param: ${hasExpiry}`);
        return hasExpiry;
    } catch (e) {
        console.log(`[${provider}] Invalid URL: ${url}`);
        return false;
    }
}

async function testMovieBox() {
    console.log('\n=== MovieBox ===');
    try {
        const streams = await moviebox.getStreams('155', 'movie');
        if (!streams.length) {
            console.log('[MovieBox] No streams returned');
            return;
        }
        for (const s of streams.slice(0, 2)) {
            inspectUrl(s.url, 'MovieBox');
        }
    } catch (e) {
        console.log('[MovieBox] Error:', e.message);
    }
}

async function testSflix() {
    console.log('\n=== Sflix ===');
    try {
        const streams = await sflix.getStreams('155', 'movie');
        if (!streams.length) {
            console.log('[Sflix] No streams returned');
            return;
        }
        for (const s of streams.slice(0, 2)) {
            inspectUrl(s.url, 'Sflix');
        }
    } catch (e) {
        console.log('[Sflix] Error:', e.message);
    }
}

async function testVixsrc() {
    console.log('\n=== VidXsrc ===');
    try {
        const streams = await vixsrc.getVixsrcStreams('155', 'movie');
        if (!streams.length) {
            console.log('[VidXsrc] No streams returned');
            return;
        }
        for (const s of streams.slice(0, 2)) {
            inspectUrl(s.url, 'VidXsrc');
        }
    } catch (e) {
        console.log('[VidXsrc] Error:', e.message);
    }
}

(async () => {
    await testMovieBox();
    await testSflix();
    await testVixsrc();
    console.log('\n=== Done ===');
})();
