const { EventSource } = require('eventsource');
const { exec } = require('child_process');
const fs = require('fs');

const API_KEY = process.env.API_KEY || '';
const LOG = '/mnt/devdrive/projects/Misc/hue/vidrock-vixsrc-expiry.log';

async function getUrl(endpoint, providerFilter) {
    return new Promise((resolve) => {
        const es = new EventSource(`http://localhost:24609${endpoint}`, {
            headers: { 'x-api-key': API_KEY }
        });
        let found = false;

        es.onmessage = (e) => {
            try {
                const data = JSON.parse(e.data);
                if (data.streams) {
                    for (const s of data.streams) {
                        if (!providerFilter || s.provider?.toLowerCase().includes(providerFilter)) {
                            found = true;
                            resolve(s.url);
                            es.close();
                            return;
                        }
                    }
                }
                if (e.lastEventId === 'done') {
                    if (!found) resolve(null);
                    es.close();
                }
            } catch (_) {}
        };

        es.onerror = () => {
            if (!found) resolve(null);
            es.close();
        };

        setTimeout(() => {
            if (!found) {
                resolve(null);
                es.close();
            }
        }, 20000);
    });
}

async function main() {
    fs.appendFileSync(LOG, `[${new Date().toISOString()}] Fetching fresh URLs...\n`);

    const vidrockUrl = await getUrl('/api/streams/movie/533535/live', 'luna');
    const vixsrcUrl = await getUrl('/api/streams/movie/533535/live', 'aura');

    fs.appendFileSync(LOG, `VidRock URL: ${vidrockUrl || 'NOT FOUND'}\n`);
    fs.appendFileSync(LOG, `Vixsrc URL: ${vixsrcUrl || 'NOT FOUND'}\n`);

    if (!vidrockUrl && !vixsrcUrl) {
        fs.appendFileSync(LOG, `[${new Date().toISOString()}] No URLs found, aborting probe\n`);
        process.exit(1);
    }

    fs.appendFileSync(LOG, `Probing every 15 min for up to 6 hours\n---\n`);

    for (let i = 0; i <= 24; i++) {
        const elapsed = i * 15;
        const ts = new Date().toISOString();

        if (vidrockUrl) {
            const cmd = `curl -s -o /dev/null -w "%{http_code}" -H "User-Agent: Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36" -H "Referer: https://vidrock.net/" -H "Origin: https://vidrock.net" -I "${vidrockUrl}" 2>/dev/null`;
            const status = await new Promise(r => exec(cmd, (e, out) => r(out?.trim() || 'ERR')));
            fs.appendFileSync(LOG, `[${ts}] +${elapsed}min vidrock_status=${status}\n`);
            if (status !== '200' && status !== '206') {
                fs.appendFileSync(LOG, `[${ts}] VidRock EXPIRED at +${elapsed} minutes\n`);
            }
        }

        if (vixsrcUrl) {
            const cmd = `curl -s -o /dev/null -w "%{http_code}" -H "User-Agent: Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36" -H "Referer: https://vixsrc.to/" -I "${vixsrcUrl}" 2>/dev/null`;
            const status = await new Promise(r => exec(cmd, (e, out) => r(out?.trim() || 'ERR')));
            fs.appendFileSync(LOG, `[${ts}] +${elapsed}min vixsrc_status=${status}\n`);
            if (status !== '200' && status !== '206') {
                fs.appendFileSync(LOG, `[${ts}] Vixsrc EXPIRED at +${elapsed} minutes\n`);
            }
        }

        if (i < 24) {
            await new Promise(r => setTimeout(r, 900000));
        }
    }

    fs.appendFileSync(LOG, `[${new Date().toISOString()}] Probe complete\n`);
}

main().catch(e => {
    fs.appendFileSync(LOG, `[${new Date().toISOString()}] ERROR: ${e.message}\n`);
    process.exit(1);
});
