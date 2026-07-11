const fetch = require('node-fetch');
const UA = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36';

(async () => {
  const hexId = '68e5280064d200c87068dcd1';
  
  // Get episodes around 1161
  console.log('=== Episodes start=1161 ===');
  const epRes = await fetch('https://animedata.cfd/api/episodes/' + hexId + '?start=1161', {
    headers: { 'User-Agent': UA, 'X-Requested-With': 'XMLHttpRequest' }
  });
  const epData = await epRes.json();
  
  if (epData.episodes) {
    const ep1161 = epData.episodes.find(e => e.episodeNumber == 1161);
    if (ep1161) {
      console.log('Episode 1161 link:', JSON.stringify(ep1161.link));
      
      // Extract server ID
      const match = ep1161.link?.sub?.[0]?.match(/\/s-2\/(\d+)/);
      if (match) {
        console.log('Server ID:', match[1]);
        
        // Get stream
        const streamRes = await fetch('https://megaplay.buzz/stream/getSources?id=' + match[1], {
          headers: { 'User-Agent': UA, 'X-Requested-With': 'XMLHttpRequest', 'Referer': 'https://megaplay.buzz/' }
        });
        const streamData = await streamRes.json().catch(() => ({}));
        console.log('Stream file:', streamData.sources?.file);
      }
    }
  }
  
  // Check if episode 1 and 1161 have the same server ID
  console.log('\n=== Episodes start=1 ===');
  const epRes2 = await fetch('https://animedata.cfd/api/episodes/' + hexId + '?start=1', {
    headers: { 'User-Agent': UA, 'X-Requested-With': 'XMLHttpRequest' }
  });
  const epData2 = await epRes2.json();
  const ep1 = epData2.episodes?.find(e => e.episodeNumber == 1);
  if (ep1) {
    console.log('Episode 1 link:', JSON.stringify(ep1.link));
    const match1 = ep1.link?.sub?.[0]?.match(/\/s-2\/(\d+)/);
    console.log('Episode 1 server ID:', match1?.[1]);
  }
})();
