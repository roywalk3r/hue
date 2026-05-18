const fetch = require('node-fetch');
const UA = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36';

(async () => {
  // Test both streams to see which one is correct
  const urls = [
    { name: 'data-id=36396', url: 'https://s1.streamzone1.site/anime/f899139df5e1059396431415e770c6dd/61b87186ab260d05003427e16ccf5657/master.m3u8' },
    { name: 'server-id=2142', url: 'https://cdn.mewstream.buzz/anime/45694f27d75ef49ace2d62206a820a2a/1c872396b32f04b30bffd9d92768de21/master.m3u8' },
  ];
  
  for (const { name, url } of urls) {
    console.log('\n===', name, '===');
    try {
      const res = await fetch(url, {
        headers: { 'User-Agent': UA, Referer: 'https://megaplay.buzz/' }
      });
      const text = await res.text();
      console.log('Status:', res.status);
      console.log('First 5 lines:');
      console.log(text.split('\n').slice(0, 5).join('\n'));
    } catch (e) {
      console.log('Error:', e.message);
    }
  }
})();
