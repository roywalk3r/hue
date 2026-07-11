const fetch = require('node-fetch');
const UA = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36';

(async () => {
  // First get the correct episode 1 slug
  const epRes = await fetch('https://animedata.cfd/api/episodes/68e5280064d200c87068dcd1?start=1', {
    headers: { 'User-Agent': UA, 'X-Requested-With': 'XMLHttpRequest' }
  });
  const epData = await epRes.json();
  const ep1 = epData.episodes?.find(e => e.episodeNumber == 1);
  const slug = ep1?.slugs?.[0];
  console.log('Episode 1 slug:', slug);
  
  if (slug) {
    // Check if hianimes.se has a page for it
    const url = 'https://hianimes.se/watch/' + slug;
    console.log('Checking:', url);
    const res = await fetch(url, { headers: { 'User-Agent': UA } });
    const text = await res.text();
    console.log('Status:', res.status, 'Length:', text.length);
    
    // Check for vidmoly iframe
    const iframeMatch = text.match(/src=\"(https:\/\/[^\"]+)\"/);
    if (iframeMatch) {
      console.log('Iframe src:', iframeMatch[1]);
    }
  }
})();
