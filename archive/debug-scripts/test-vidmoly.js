const fetch = require('node-fetch');
const UA = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36';

(async () => {
  const url = 'https://vidmoly.biz/embed-fqyz11yb8t0f.html';
  console.log('Fetching:', url);
  const res = await fetch(url, {
    headers: { 'User-Agent': UA, Referer: 'https://hianimes.se/' }
  });
  const text = await res.text();
  console.log('Status:', res.status, 'Length:', text.length);
  
  // Look for video source, m3u8, mp4, or any stream URL
  const patterns = [
    ['m3u8', /https?:\/\/[^\s\"'\)]+\.m3u8/g],
    ['mp4', /https?:\/\/[^\s\"'\)]+\.mp4/g],
    ['source', /src\s*=\s*\"([^\"]+)\"/g],
    ['video', /<video[^>]*src=\"([^\"]+)\"/g],
    ['file', /file\s*[:=]\s*['\"]([^\"']+)['\"]/g],
    ['sources', /sources\s*[:=]\s*\[?\s*['\"]([^\"']+)['\"]/g],
    ['eval', /eval\(function\(p,a,c,k,e,d\)/g],
    ['jwplayer', /jwplayer/g],
    ['plyr', /plyr/g],
  ];
  
  for (const [name, regex] of patterns) {
    const matches = [...text.matchAll(regex)];
    if (matches.length > 0) {
      console.log('\n---', name, '(' + matches.length + ' matches) ---');
      [...new Set(matches.map(m => m[0] || m[1]))].slice(0, 5).forEach(m => console.log(m.substring(0, 200)));
    }
  }
  
  // Look for packed JS
  const packed = text.match(/eval\(function\(p,a,c,k,e,d\)[^<]+/);
  if (packed) {
    console.log('\n--- Packed JS found ---');
    console.log(packed[0].substring(0, 300));
  }
  
  // Look for any script with video data
  const scripts = [...text.matchAll(/<script>([\s\S]*?)<\/script>/g)];
  for (const s of scripts) {
    const content = s[1];
    if (content.includes('video') || content.includes('source') || content.includes('file') || content.includes('m3u8') || content.includes('mp4')) {
      if (content.length < 500) {
        console.log('\n--- Relevant script ---');
        console.log(content.substring(0, 300));
      }
    }
  }
})();
