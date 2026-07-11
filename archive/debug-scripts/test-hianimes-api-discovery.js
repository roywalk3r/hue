const fetch = require('node-fetch');
const UA = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36';

(async () => {
  const base = 'https://hianimes.se';
  const epId = 'hef5mk';
  const animeHexId = '68e5280064d200c87068dcd1';
  
  const endpoints = [
    '/ajax/v2/episode/servers?episodeId=' + epId,
    '/ajax/v2/episode/sources?id=1',
    '/api/episode/' + epId,
    '/api/episode/id/' + animeHexId,
    '/api/episodes/' + animeHexId + '?start=1',
    '/api/episodes/' + animeHexId + '?start=1161',
    '/api/anime/one-piece-gahocg',
    '/api/home',
    '/api/latest/anime',
    '/api/latest/episode',
    '/api/search',
    '/api/stream?episodeId=' + epId,
    '/api/server?episodeId=' + epId,
    '/api/sources?episodeId=' + epId,
    '/api/video?episodeId=' + epId,
    '/watch/' + epId + '/servers',
    '/watch/' + epId + '/sources',
    '/_next/data/static/watch/one-piece-episode-1161-' + epId + '.json',
  ];
  
  for (const endpoint of endpoints) {
    const url = base + endpoint;
    try {
      const res = await fetch(url, {
        headers: { 
          'User-Agent': UA, 
          'X-Requested-With': 'XMLHttpRequest',
          'Referer': base + '/watch/one-piece-episode-1161-' + epId
        },
        timeout: 10000
      });
      const text = await res.text();
      let type = 'unknown';
      try { JSON.parse(text); type = 'json'; } catch { type = 'html'; }
      console.log(endpoint, '=>', res.status, type, 'len:', text.length);
      if (type === 'json' && text.length < 500) {
        console.log('  ', text.substring(0, 200));
      }
    } catch (e) {
      console.log(endpoint, '=>', 'ERROR:', e.message.substring(0, 50));
    }
  }
})();
