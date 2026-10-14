const crypto = require('crypto');

const EM = 'c4a8f1d7e2b9a6c3d0f5e8a1b7c4d9e2';
const TMDB_ID = 533535;

const HEADERS = {
  accept: 'application/json, text/plain, */*',
  'accept-language': 'en-US,en;q=0.9',
  'user-agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36',
  referer: `https://player.vidzee.wtf/embed/movie/${TMDB_ID}`,
};

async function httpGet(url, extraHeaders = {}) {
  const res = await fetch(url, { headers: { ...HEADERS, ...extraHeaders } });
  const text = await res.text();
  if (!res.ok) throw new Error(`GET ${url} => ${res.status}: ${text.slice(0, 200)}`);
  return { res, text };
}

async function decryptApiKey(b64) {
  const enc = new TextEncoder();
  const keyMaterial = await crypto.webcrypto.subtle.digest('SHA-256', enc.encode(EM));
  const key = await crypto.webcrypto.subtle.importKey('raw', keyMaterial, { name: 'AES-GCM' }, false, ['decrypt']);
  const raw = Buffer.from(b64.replace(/\s+/g, ''), 'base64');
  if (raw.length <= 28) throw new Error('api-key payload too short');
  const iv = raw.slice(0, 12);
  const tag = raw.slice(12, 28);
  const cipher = raw.slice(28);
  const payload = Buffer.concat([cipher, tag]);
  const plain = await crypto.webcrypto.subtle.decrypt({ name: 'AES-GCM', iv, tagLength: 128 }, key, payload);
  return new TextDecoder().decode(plain);
}

function decryptLink(linkB64, keyPlain) {
  const inner = Buffer.from(linkB64, 'base64').toString('utf8');
  const [ivB64, cipherB64] = inner.split(':');
  if (!ivB64 || !cipherB64) throw new Error('link payload missing iv:cipher');
  const iv = Buffer.from(ivB64, 'base64');
  const key = Buffer.from(keyPlain.padEnd(32, '\0'), 'utf8');
  const decipher = crypto.createDecipheriv('aes-256-cbc', key, iv);
  let plain = decipher.update(cipherB64, 'base64', 'utf8');
  plain += decipher.final('utf8');
  return plain;
}

async function headCheck(url) {
  const res = await fetch(url, { method: 'HEAD', headers: HEADERS, redirect: 'follow' });
  return { ok: res.ok, status: res.status, ct: res.headers.get('content-type'), len: res.headers.get('content-length') };
}

async function main() {
  const { text: apiKeyCipher } = await httpGet('https://core.vidzee.wtf/api-key');
  const apiKey = await decryptApiKey(apiKeyCipher);

  const { text: serverJson } = await httpGet(`https://player.vidzee.wtf/api/server?id=${TMDB_ID}&sr=0`);
  console.log(' server raw:', serverJson.slice(0, 500));
  const server = JSON.parse(serverJson);
  const entry = Array.isArray(server.url) ? server.url[0] : server;
  if (!entry?.link) throw new Error('server response missing link: ' + Object.keys(server).join(', '));

  const link = decryptLink(entry.link, apiKey);
  console.log(' decrypted link:', link);

  const m3u8Info = await headCheck(link);
  console.log(' m3u8 HEAD:', m3u8Info);

  const { text: m3u8 } = await httpGet(link);
  console.log(' m3u8 lines:\n' + m3u8.slice(0, 1200));
  const segmentUrl = m3u8.split('\n').find(line => line.trim() && !line.startsWith('#'));
  const segmentUrls = m3u8.split('\n').filter(line => line.trim() && !line.startsWith('#')).slice(0, 5);
  for (const segUrl of segmentUrls) {
    const segRes = await fetch(segUrl, { headers: HEADERS });
    const segBuf = Buffer.from(await segRes.arrayBuffer());
    const hex = segBuf.slice(0, 16).toString('hex');
    const sig = hex.startsWith('89504e47') ? 'PNG' : hex.startsWith('47400010') || hex.startsWith('000000') ? 'MP4' : 'OTHER';
    console.log(' segment', sig, 'size', segBuf.length, 'url', segUrl.slice(0, 80) + '...');
  }
}

main().catch(e => { console.error(e); process.exit(1); });
