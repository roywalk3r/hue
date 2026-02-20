const axios = require('axios');

function encodeVarint(value) {
    const out = [];
    let n = Number(value) >>> 0;
    while (n >= 0x80) {
        out.push((n & 0x7f) | 0x80);
        n >>>= 7;
    }
    out.push(n);
    return Buffer.from(out);
}

function buildPayload(uri) {
    const body = Buffer.from(uri, 'utf8');
    return Buffer.concat([Buffer.from([0x0a]), encodeVarint(body.length), body]);
}

function extractSlug(raw) {
    const text = raw.toString('utf8');
    // Look for patterns like streamed-willow
    const matches = text.match(/[a-z0-9_-]{5,}/ig) || [];
    console.log('Matches:', matches);
    return matches;
}

async function test() {
    const uri = 'streamed-willow';
    const payload = buildPayload(uri);

    try {
        const res = await axios.post('https://a.embedsports.top/fetch', payload, {
            headers: {
                'Content-Type': 'application/octet-stream',
                'User-Agent': 'Mozilla/5.0 (iPhone; CPU iPhone OS 18_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.5 Mobile/15E148 Safari/604.1',
                'Referer': 'https://a.embedsports.top/embed-noads/streamed-willow',
                'Origin': 'https://a.embedsports.top'
            },
            responseType: 'arraybuffer'
        });
        
        const raw = Buffer.from(res.data);
        console.log('Raw Length:', raw.length);
        extractSlug(raw);
    } catch (e) {
        console.error('Fetch failed:', e.message);
    }
}

test();
