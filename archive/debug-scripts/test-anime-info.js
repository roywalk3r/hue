const fetch = require('node-fetch');
const UA = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36';

(async () => {
  // We know the episode endpoint works with full slug
  const epRes = await fetch('https://animedata.cfd/api/episode/one-piece-episode-1161-hef5mk', {
    headers: { 'User-Agent': UA, 'X-Requested-With': 'XMLHttpRequest' }
  });
  const epData = await epRes.json();
  console.log('Anime from episode:', JSON.stringify(epData.anime, null, 2).substring(0, 800));
  
  // Try different anime endpoints
  const slugs = ['one-piece', 'one-piece-gahocg', epData.anime?.slug, epData.anime?._id];
  for (const slug of slugs.filter(Boolean)) {
    console.log('\n=== Testing anime slug:', slug, '===');
    const res = await fetch('https://animedata.cfd/api/anime/' + slug, {
      headers: { 'User-Agent': UA, 'X-Requested-With': 'XMLHttpRequest' }
    });
    const text = await res.text();
    console.log('Status:', res.status, 'Length:', text.length);
    if (text.length < 500) console.log('Body:', text);
    else {
      try {
        const json = JSON.parse(text);
        console.log('Keys:', Object.keys(json));
      } catch { console.log('Preview:', text.substring(0, 200)); }
    }
  }
  
  // Try search
  console.log('\n=== Search for one piece ===');
  const searchRes = await fetch('https://animedata.cfd/api/search', {
    method: 'POST',
    headers: { 'User-Agent': UA, 'X-Requested-With': 'XMLHttpRequest', 'Content-Type': 'application/json' },
    body: JSON.stringify({ title: 'one piece' })
  });
  const searchData = await searchRes.json();
  console.log('Results:', searchData.length);
  if (searchData[0]) {
    console.log('First result _id:', searchData[0]._id);
    console.log('First result slugs:', searchData[0].slugs);
    console.log('First result slug:', searchData[0].slug);
    
    // Try episodes with _id
    if (searchData[0]._id) {
      const epListRes = await fetch('https://animedata.cfd/api/episodes/' + searchData[0]._id + '?start=1161', {
        headers: { 'User-Agent': UA, 'X-Requested-With': 'XMLHttpRequest' }
      });
      const epList = await epListRes.json();
      console.log('\nEpisodes status:', epListRes.status);
      if (epList.episodes) {
        const target = epList.episodes.find(e => e.episodeNumber == 1161);
        if (target) {
          console.log('Found episode 1161 _id:', target._id);
          console.log('Found episode 1161 slugs:', target.slugs);
        }
      }
    }
  }
})();
