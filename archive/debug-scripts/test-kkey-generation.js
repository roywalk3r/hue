const https = require('https');
const vm = require('vm');

const KISSKH_CONFIG = {
    baseUrl: 'https://kisskh.ovh',
    viGuid: '62f176f3bb1b5b8e70e39932ad34a0c7',
    appVer: '2.8.10',
    platformVer: 4830201,
    appName: 'kisskh',
};

async function fetchText(url) {
    return new Promise((resolve, reject) => {
        https.get(url, { headers: { 'User-Agent': 'Mozilla/5.0' } }, (res) => {
            let data = '';
            res.on('data', chunk => data += chunk);
            res.on('end', () => resolve(data));
        }).on('error', reject);
    });
}

(async () => {
    try {
        const jsCode = await fetchText('https://kisskh.ovh/common.js');
        console.log('Fetched common.js, length:', jsCode.length);

        const ctx = vm.createContext({});
        vm.runInContext(jsCode, ctx);

        const hasFunc = vm.runInContext('typeof _0x54b991', ctx);
        console.log('_0x54b991 type:', hasFunc);

        if (hasFunc === 'function') {
            const episodeId = 207851;
            const token = vm.runInContext(
                `_0x54b991(${episodeId}, null, "${KISSKH_CONFIG.appVer}", "${KISSKH_CONFIG.viGuid}", ${KISSKH_CONFIG.platformVer}, "${KISSKH_CONFIG.appName}", "${KISSKH_CONFIG.appName}", "${KISSKH_CONFIG.appName}", "${KISSKH_CONFIG.appName}", "${KISSKH_CONFIG.appName}", "${KISSKH_CONFIG.appName}")`,
                ctx
            );
            console.log('Generated kkey length:', token.length);
            console.log('Generated kkey:', token.substring(0, 64) + '...');

            const url = `${KISSKH_CONFIG.baseUrl}/api/DramaList/Episode/${episodeId}.png?err=false&ts=0&time=0&kkey=${token}`;
            console.log('Fetching API...');

            const resp = await fetchText(url);
            console.log('Response length:', resp.length);
            console.log('Response preview:', resp.substring(0, 200));

            try {
                const data = JSON.parse(resp);
                console.log('Parsed JSON keys:', Object.keys(data));
                console.log('Video:', data.Video ? data.Video.substring(0, 80) + '...' : 'none');
            } catch (e) {
                try {
                    const decoded = Buffer.from(resp, 'base64').toString('utf8');
                    const data = JSON.parse(decoded);
                    console.log('Base64 decoded keys:', Object.keys(data));
                    console.log('Video:', data.Video ? data.Video.substring(0, 80) + '...' : 'none');
                } catch (e2) {
                    console.log('Not valid JSON or base64');
                }
            }
        }
    } catch (e) {
        console.error('Error:', e.message);
    }
})();
