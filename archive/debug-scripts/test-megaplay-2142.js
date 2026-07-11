const fetch = require('node-fetch');
const UA = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36';

(async () => {
  const domains = ['https://megaplay.buzz', 'https://animeplay.video', 'https://embed.anicdn.to'];
  
  for (const domain of domains) {
    console.log('\n===', domain, '===');
    try {
      const res = await fetch(domain + '/stream/getSources?id=2142', {
        headers: { 
          'User-Agent': UA, 
          'X-Requested-With': 'XMLHttpRequest',
          'Referer': domain + '/'
        }
      });
      const data = await res.json().catch(() => ({}));
      console.log('Status:', res.status);
      if (data.sources?.file) {
        console.log('Source file:', data.sources.file);
        
        // Fetch the M3U8 to check what it contains
        const m3u8Res = await fetch(data.sources.file, {
          headers: { 'User-Agent': UA, 'Referer': domain + '/' }
        });
        const m3u8 = await m3u8Res.text();
        console.log('\nM3U8 first 10 lines:');
        console.log(m3u8.split('\n').slice(0, 10).join('\n'));
      } else {
        console.log('Response:', JSON.stringify(data).substring(0, 300));
      }
    } catch (e) {
      console.log('Error:', e.message);
    }
  }
})();
