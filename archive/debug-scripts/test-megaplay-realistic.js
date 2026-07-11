const fetch = require('node-fetch');
const UA = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/123.0.0.0 Safari/537.36';

(async () => {
  // Test the megaplay iframe URL with various headers
  const url = 'https://megaplay.buzz/stream/s-2/2142/sub';
  
  const configs = [
    { headers: { 'User-Agent': UA, Referer: 'https://hianimes.se/' } },
    { headers: { 'User-Agent': UA, Referer: 'https://hianimes.se/watch/one-piece-episode-1-a7b7hv-141' } },
    { headers: { 'User-Agent': UA, 'X-Requested-With': 'XMLHttpRequest', Referer: 'https://megaplay.buzz/' } },
    { headers: { 'User-Agent': UA, Origin: 'https://hianimes.se' } },
  ];
  
  for (const [i, config] of configs.entries()) {
    console.log('\n=== Config', i, '===');
    try {
      const res = await fetch(url, config);
      const text = await res.text();
      console.log('Status:', res.status, 'Length:', text.length);
      console.log('First 300 chars:', text.substring(0, 300));
      
      // Check for video or player
      if (text.includes('video') || text.includes('player') || text.includes('m3u8') || text.includes('source')) {
        console.log('Found video-related content');
      }
    } catch (e) {
      console.log('Error:', e.message);
    }
  }
})();
