const fetch = require('node-fetch');
const UA = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36';

(async () => {
  const base = 'https://hianimes.se';
  const htmlRes = await fetch(base + '/watch/one-piece-episode-1161-hef5mk', { headers: { 'User-Agent': UA } });
  const html = await htmlRes.text();
  const chunks = [...html.matchAll(/src=\"(\/_next\/static\/chunks\/[^\"]+\.js)\"/g)].map(m => m[1]);
  
  console.log('Searching', chunks.length, 'chunks...');
  
  for (const chunk of chunks) {
    try {
      const res = await fetch(base + chunk, { headers: { 'User-Agent': UA } });
      const text = await res.text();
      
      // Look for stream/getSources, megaplay, s-2, or any stream resolution
      const patterns = [
        ['getSources', /getSources/g],
        ['megaplay', /megaplay/g],
        ['animeplay', /animeplay/g],
        ['s-2', /s-2/g],
        ['/stream/', /\/stream\//g],
        ['embed', /embed/g],
        ['vidmoly', /vidmoly/g],
        ['nova.upn', /nova\.upn/g],
        ['m3u8', /m3u8/g],
        ['sources', /sources/g],
        ['tracks', /tracks/g],
        ['X-Requested-With', /X-Requested-With/g],
      ];
      
      const hits = [];
      for (const [name, regex] of patterns) {
        const matches = text.match(regex);
        if (matches) hits.push(`${name}(${matches.length})`);
      }
      
      if (hits.length > 3) {
        console.log('\n---', chunk, '---');
        console.log('Size:', text.length);
        console.log('Hits:', hits.join(', '));
        
        // If it has getSources or stream, print surrounding context
        if (text.includes('getSources') || text.includes('s-2')) {
          const idx = text.indexOf('getSources') || text.indexOf('s-2');
          console.log('Context:', text.substring(Math.max(0, idx - 100), idx + 200));
        }
      }
    } catch (e) {}
  }
})();
