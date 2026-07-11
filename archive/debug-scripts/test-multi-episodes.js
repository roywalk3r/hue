const fetch = require('node-fetch');
const UA = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36';

(async () => {
  // Test different episodes and animes
  const tests = [
    'one-piece-episode-1161-hef5mk',
    'one-piece-episode-1-gahocg',  // try episode 1
    'one-piece-episode-100-8mkhmb',
    'naruto-episode-1-abc123',
  ];
  
  for (const slug of tests) {
    console.log('\n=== Testing:', slug, '===');
    try {
      const res = await fetch('https://animedata.cfd/api/episode/' + slug, {
        headers: { 'User-Agent': UA, 'X-Requested-With': 'XMLHttpRequest' }
      });
      const data = await res.json();
      if (data.episode?.link?.sub) {
        console.log('Links:', JSON.stringify(data.episode.link.sub));
        const hasMegaplay = data.episode.link.sub.some(l => l.includes('megaplay') || l.includes('animeplay') || l.includes('s-2'));
        console.log('Has megaplay:', hasMegaplay);
      } else {
        console.log('No link data');
      }
    } catch (e) {
      console.log('Error:', e.message);
    }
  }
  
  // Also try random anime
  console.log('\n=== Search for recent anime ===');
  const searchRes = await fetch('https://animedata.cfd/api/search', {
    method: 'POST',
    headers: { 'User-Agent': UA, 'X-Requested-With': 'XMLHttpRequest', 'Content-Type': 'application/json' },
    body: JSON.stringify({ title: 'solo leveling' })
  });
  const searchData = await searchRes.json();
  if (searchData[0]?.slugs?.[0]) {
    const animeSlug = searchData[0].slugs[0];
    console.log('Found anime slug:', animeSlug);
    
    // Get episodes
    const epRes = await fetch('https://animedata.cfd/api/episodes/' + searchData[0]._id + '?start=1', {
      headers: { 'User-Agent': UA, 'X-Requested-With': 'XMLHttpRequest' }
    });
    const epData = await epRes.json();
    if (epData.episodes?.[0]) {
      const firstEp = epData.episodes[0];
      console.log('First ep slugs:', firstEp.slugs?.slice(0, 3));
      
      if (firstEp.slugs?.[0]) {
        const epRes2 = await fetch('https://animedata.cfd/api/episode/' + firstEp.slugs[0], {
          headers: { 'User-Agent': UA, 'X-Requested-With': 'XMLHttpRequest' }
        });
        const epData2 = await epRes2.json();
        if (epData2.episode?.link?.sub) {
          console.log('Links:', JSON.stringify(epData2.episode.link.sub));
          const hasMegaplay = epData2.episode.link.sub.some(l => l.includes('megaplay') || l.includes('animeplay') || l.includes('s-2'));
          console.log('Has megaplay:', hasMegaplay);
        }
      }
    }
  }
})();
