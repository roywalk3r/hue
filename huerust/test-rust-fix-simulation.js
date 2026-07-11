const fetch = require('node-fetch');
const UA = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36';

(async () => {
  // 1. Get episode 1 from episodes endpoint
  const epRes = await fetch('https://animedata.cfd/api/episodes/68e5280064d200c87068dcd1?start=1', {
    headers: { 'User-Agent': UA, 'X-Requested-With': 'XMLHttpRequest' }
  });
  const epData = await epRes.json();
  const ep1 = epData.episodes?.find(e => e.episodeNumber == 1);
  const iframeUrl = ep1?.link?.sub?.[0];
  console.log('Episode 1 iframe URL:', iframeUrl);
  
  // 2. Fetch iframe page to extract data-id
  const iframeRes = await fetch(iframeUrl, {
    headers: { 'User-Agent': UA, Referer: 'https://hianimes.se/' }
  });
  const html = await iframeRes.text();
  const dataIdMatch = html.match(/data-id\s*=\s*"(\d+)"/);
  const dataId = dataIdMatch?.[1];
  console.log('Extracted data-id:', dataId);
  
  // 3. Call getSources with data-id
  const sourcesRes = await fetch('https://megaplay.buzz/stream/getSources?id=' + dataId, {
    headers: { 'User-Agent': UA, 'X-Requested-With': 'XMLHttpRequest', Referer: 'https://megaplay.buzz/' }
  });
  const sourcesData = await sourcesRes.json();
  console.log('Stream URL:', sourcesData.sources?.file);
  console.log('Tracks:', sourcesData.tracks?.length);
  console.log('Intro:', sourcesData.intro);
  console.log('Outro:', sourcesData.outro);
})();
