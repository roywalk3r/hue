const fetch = require('node-fetch');
const UA = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36';

(async () => {
  const hexId = '68e5280064d200c87068dcd1';
  
  // 1. Get episodes for One Piece starting at 1
  console.log('=== Episodes start=1 ===');
  const epRes = await fetch('https://animedata.cfd/api/episodes/' + hexId + '?start=1', {
    headers: { 'User-Agent': UA, 'X-Requested-With': 'XMLHttpRequest' }
  });
  const epData = await epRes.json();
  console.log('Status:', epRes.status, 'Episodes count:', epData.episodes?.length);
  
  if (epData.episodes) {
    const ep1 = epData.episodes.find(e => e.episodeNumber == 1);
    if (ep1) {
      console.log('Episode 1 _id:', ep1._id);
      console.log('Episode 1 slugs:', ep1.slugs?.slice(0, 3));
      console.log('Episode 1 link:', JSON.stringify(ep1.link));
      
      if (ep1.slugs?.[0]) {
        // 2. Get servers for episode 1 slug
        console.log('\n=== Servers for episode 1 slug ===');
        const srvRes = await fetch('https://animedata.cfd/api/episode/' + ep1.slugs[0], {
          headers: { 'User-Agent': UA, 'X-Requested-With': 'XMLHttpRequest' }
        });
        const srvData = await srvRes.json();
        console.log('Status:', srvRes.status);
        if (srvData.episode?.link) {
          console.log('Link.sub:', JSON.stringify(srvData.episode.link.sub));
        }
      }
    } else {
      console.log('Episode 1 not found. First few episodes:', epData.episodes.slice(0, 3).map(e => ({ num: e.episodeNumber, _id: e._id, slugs: e.slugs?.slice(0, 2) })));
    }
  }
})();
