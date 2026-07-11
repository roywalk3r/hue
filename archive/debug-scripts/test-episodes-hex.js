const fetch = require('node-fetch');
const UA = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36';

(async () => {
  const hexId = '68e5280064d200c87068dcd1';
  
  console.log('=== Episodes start=1161 ===');
  const res = await fetch('https://animedata.cfd/api/episodes/' + hexId + '?start=1161', {
    headers: { 'User-Agent': UA, 'X-Requested-With': 'XMLHttpRequest' }
  });
  const data = await res.json();
  console.log('Status:', res.status);
  console.log('Has episodes:', !!data.episodes);
  if (data.episodes) {
    const target = data.episodes.find(e => e.episodeNumber == 1161);
    if (target) {
      console.log('Episode keys:', Object.keys(target));
      console.log('Episode _id:', target._id);
      console.log('Episode slugs:', target.slugs);
      console.log('Episode link:', JSON.stringify(target.link));
      
      // Now try episode endpoint with hex ID
      if (target._id) {
        console.log('\n=== Episode by hex ID ===');
        const epRes = await fetch('https://animedata.cfd/api/episode/id/' + target._id, {
          headers: { 'User-Agent': UA, 'X-Requested-With': 'XMLHttpRequest' }
        });
        const epData = await epRes.json();
        console.log('Status:', epRes.status);
        if (epData.episode?.link) {
          console.log('Link:', JSON.stringify(epData.episode.link));
        }
      }
      
      // Also try episode endpoint with first slug
      if (target.slugs?.[0]) {
        console.log('\n=== Episode by slug ===');
        const epRes2 = await fetch('https://animedata.cfd/api/episode/' + target.slugs[0], {
          headers: { 'User-Agent': UA, 'X-Requested-With': 'XMLHttpRequest' }
        });
        const epData2 = await epRes2.json();
        console.log('Status:', epRes2.status);
        if (epData2.episode?.link) {
          console.log('Link:', JSON.stringify(epData2.episode.link));
        }
      }
    } else {
      console.log('Episode 1161 not found in list');
      console.log('Available episodes:', data.episodes.map(e => e.episodeNumber).slice(0, 10));
    }
  }
})();
