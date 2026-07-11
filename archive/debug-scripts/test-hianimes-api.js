const fetch = require('node-fetch');
const UA = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36';

async function test(url, headers = {}) {
  try {
    const res = await fetch(url, { headers: { 'User-Agent': UA, ...headers }, timeout: 10000 });
    const text = await res.text();
    console.log('Status:', res.status, 'Length:', text.length);
    if (text.length < 500) console.log('Body:', text);
    else {
      try {
        const json = JSON.parse(text);
        console.log('JSON keys:', Object.keys(json));
        if (json.html) console.log('Has html field, length:', json.html.length);
        if (json.link) console.log('Link:', json.link);
        if (json.sources) console.log('Sources:', JSON.stringify(json.sources).substring(0, 200));
      } catch {
        console.log('Body preview:', text.substring(0, 300));
      }
    }
  } catch (e) {
    console.log('Error:', e.message);
  }
}

(async () => {
  const base = 'https://hianimes.se';
  const epId = 'hef5mk';

  console.log('\n=== Test 1: AJAX servers ===');
  await test(base + '/ajax/v2/episode/servers?episodeId=' + epId, { 'X-Requested-With': 'XMLHttpRequest', Referer: base + '/watch/one-piece-episode-1161-' + epId });

  console.log('\n=== Test 2: AJAX sources (serverId=1) ===');
  await test(base + '/ajax/v2/episode/sources?id=1', { 'X-Requested-With': 'XMLHttpRequest', Referer: base + '/watch/one-piece-episode-1161-' + epId });

  console.log('\n=== Test 3: API episode (slug) ===');
  await test(base + '/api/episode/' + epId, { 'X-Requested-With': 'XMLHttpRequest' });

  console.log('\n=== Test 4: API anime info ===');
  await test(base + '/api/anime/one-piece', { 'X-Requested-With': 'XMLHttpRequest' });
})();
