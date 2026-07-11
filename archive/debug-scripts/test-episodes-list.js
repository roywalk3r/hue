const fetch = require('node-fetch');
const UA = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36';

(async () => {
  // First get anime info to find hex ID
  const infoRes = await fetch('https://animedata.cfd/api/anime/one-piece', {
    headers: { 'User-Agent': UA, 'X-Requested-With': 'XMLHttpRequest' }
  });
  const info = await infoRes.json().catch(() => ({}));
  console.log('Info status:', infoRes.status);
  console.log('Anime _id:', info.anime?._id);
  
  // Test episodes endpoint
  if (info.anime?._id) {
    const epRes = await fetch('https://animedata.cfd/api/episodes/' + info.anime._id + '?start=1161', {
      headers: { 'User-Agent': UA, 'X-Requested-With': 'XMLHttpRequest' }
    });
    const epData = await epRes.json().catch(() => ({}));
    console.log('\nEpisodes status:', epRes.status);
    if (epData.episodes) {
      const targetEp = epData.episodes.find(e => e.episodeNumber == 1161);
      if (targetEp) {
        console.log('Target episode:', JSON.stringify(targetEp, null, 2).substring(0, 500));
      }
    }
  }
  
  // Also try the old API approach: episode by hex ID
  const epRes2 = await fetch('https://animedata.cfd/api/episode/id/6a009dbae933e5b762aa62be', {
    headers: { 'User-Agent': UA, 'X-Requested-With': 'XMLHttpRequest' }
  });
  const epData2 = await epRes2.json().catch(() => ({}));
  console.log('\nEpisode by hex ID status:', epRes2.status);
  if (epData2.episode) {
    console.log('Link.sub:', JSON.stringify(epData2.episode.link?.sub));
  }
})();
