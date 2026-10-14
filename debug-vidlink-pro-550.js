const nacl = require('tweetnacl');

const KEY_HEX = 'c75136c5668bbfe65a7ecad431a745db68b5f381555b38d8f6c699449cf11fcd';
const KEY = Buffer.from(KEY_HEX, 'hex');
const NONCE = Buffer.alloc(24);
const TMDB_ID = 550;

function encryptToken(mediaId) {
  const timestamp = Math.floor(Date.now() / 1000) + 480;
  const msg = Buffer.concat([Buffer.from(mediaId, 'utf8'), Buffer.allocUnsafe(8)]);
  msg.writeBigUInt64BE(BigInt(timestamp), msg.length - 8);
  const encrypted = nacl.secretbox(msg, NONCE, KEY);
  // encrypted includes 16-byte auth tag appended by secret_box
  const payload = Buffer.concat([NONCE, encrypted]);
  return payload.toString('base64url').replace(/=+$/, '');
}

async function main() {
  const token = encryptToken(String(TMDB_ID));
  const url = `https://vidlink.pro/api/b/movie/${token}?multiLang=1`;
  console.log(' URL:', url);

  const res = await fetch(url, {
    headers: {
      'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/137.0.0.0 Safari/537.36',
      Origin: 'https://vidlink.pro',
      Referer: 'https://vidlink.pro/',
      Accept: 'application/json',
    },
  });
  console.log(' status:', res.status, 'ct:', res.headers.get('content-type'));
  const text = await res.text();
  console.log(' body length:', text.length);
  console.log(' body first 1000:', text.slice(0, 1000));

  const data = JSON.parse(text);
  const stream = data.stream || data;
  const qualities = stream.qualities || (stream.sources ? stream.sources[0] : null);
  const chosen = qualities['720'] || qualities['480'] || qualities['360'] || Object.values(qualities)[0];
  console.log(' chosen quality:', Object.keys(qualities).join(','), 'using', chosen ? Object.keys(qualities).find(k => qualities[k] === chosen) : 'none');
  if (chosen && chosen.url) {
    const head = await fetch(chosen.url, { method: 'HEAD', headers: { Referer: 'https://vidlink.pro/' } });
    console.log(' mp4 HEAD:', head.status, head.headers.get('content-type'), head.headers.get('content-length'));
  }
}

main().catch(e => { console.error(e); process.exit(1); });
