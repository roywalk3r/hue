const fetch = require('node-fetch');
const UA = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36';

(async () => {
  const res = await fetch('https://animedata.cfd/api/episode/one-piece-episode-1161-hef5mk', {
    headers: { 'User-Agent': UA, 'X-Requested-With': 'XMLHttpRequest' }
  });
  const data = await res.json();
  console.log('Episode link.sub:', JSON.stringify(data.episode.link.sub, null, 2));
  console.log('Episode link.dub:', JSON.stringify(data.episode.link.dub, null, 2));
  console.log('\nEpisode public_id:', data.episode.public_id);
  console.log('Episode _id:', data.episode.id);
  console.log('Episode slugs:', data.episode.slugs);

  const sub = data.episode.link.sub || [];
  for (const link of sub) {
    const match = link.match(/\/s-2\/(\d+)/);
    if (match) {
      console.log('\nFound server ID in:', link, '=>', match[1]);
      const streamRes = await fetch('https://megaplay.buzz/stream/getSources?id=' + match[1], {
        headers: { 'User-Agent': UA, 'X-Requested-With': 'XMLHttpRequest', Referer: 'https://megaplay.buzz/' }
      });
      const streamData = await streamRes.json().catch(() => ({}));
      console.log('Stream sources:', streamData.sources ? streamData.sources.file : 'none');
    } else {
      console.log('\nEmbed link:', link);
    }
  }
})();
