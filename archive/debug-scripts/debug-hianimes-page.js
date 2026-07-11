const fetch = require('node-fetch');
const fs = require('fs');

(async () => {
  const res = await fetch('https://hianimes.se/watch/one-piece-episode-1161-hef5mk', {
    headers: { 'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36' }
  });
  const text = await res.text();
  console.log('Status:', res.status, 'Length:', text.length);

  const patterns = [
    ['__NEXT_DATA__', /<script id="__NEXT_DATA__"[^>]*>([\s\S]*?)<\/script>/],
    ['data-id=', /data-id="(\d+)"/g],
    ['episodeId', /episodeId['"]?\s*[:=]\s*['"]?([^'"\s>]+)/g],
    ['/ajax/', /\/ajax\/[^\s"'<]+/g],
    ['getSources', /getSources[^\s"'<]*/g],
    ['megaplay', /megaplay[^\s"'<]*/g],
    ['myani', /myani[^\s"'<]*/g],
    ['animedata', /animedata[^\s"'<]*/g],
  ];

  for (const [name, regex] of patterns) {
    if (!regex.global) {
      const m = text.match(regex);
      if (m) {
        console.log(`\n--- ${name} ---`);
        console.log(m[0].substring(0, 300));
        if (name === '__NEXT_DATA__') {
          try {
            const data = JSON.parse(m[1]);
            console.log('Parsed keys:', Object.keys(data));
            if (data.props?.pageProps) {
              console.log('pageProps keys:', Object.keys(data.props.pageProps));
            }
          } catch(e) { console.log('Parse error:', e.message); }
        }
      }
      continue;
    }
    const matches = [...text.matchAll(regex)];
    if (matches.length > 0) {
      console.log(`\n--- ${name} (${matches.length} matches) ---`);
      const unique = [...new Set(matches.map(m => m[0]))];
      for (const m of unique.slice(0, 10)) {
        console.log(m.substring(0, 200));
      }
    }
  }

  fs.writeFileSync('hianimes-page.html', text);
  console.log('\nSaved to hianimes-page.html');
})();
