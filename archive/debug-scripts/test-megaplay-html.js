const fetch = require('node-fetch');
const UA = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36';

(async () => {
  const res = await fetch('https://megaplay.buzz/stream/s-2/2142/sub', {
    headers: { 'User-Agent': UA, Referer: 'https://hianimes.se/' }
  });
  const text = await res.text();
  console.log('Full HTML length:', text.length);
  
  // Look for video source, m3u8, mp4, or any stream data
  const patterns = [
    ['m3u8', /https?:\/\/[^\s\"'\)]+\.m3u8/g],
    ['mp4', /https?:\/\/[^\s\"'\)]+\.mp4/g],
    ['file', /file\s*[:=]\s*['\"]([^\"']+)['\"]/g],
    ['sources', /sources\s*[:=]\s*['\"\[]/g],
    ['video src', /<video[^>]*src=\"([^\"]+)\"/g],
    ['source src', /<source[^>]*src=\"([^\"]+)\"/g],
    ['data-file', /data-file=\"([^\"]+)\"/g],
    ['data-id', /data-id=\"([^\"]+)\"/g],
    ['iframe', /<iframe[^>]*src=\"([^\"]+)\"/g],
    ['script src', /<script[^>]*src=\"([^\"]+\.js)\"/g],
  ];
  
  for (const [name, regex] of patterns) {
    const matches = [...text.matchAll(regex)];
    if (matches.length > 0) {
      console.log('\n---', name, '(' + matches.length + ') ---');
      [...new Set(matches.map(m => m[1] || m[0]))].slice(0, 5).forEach(m => console.log(m.substring(0, 200)));
    }
  }
  
  // Save full HTML
  require('fs').writeFileSync('megaplay-iframe.html', text);
  console.log('\nSaved to megaplay-iframe.html');
})();
