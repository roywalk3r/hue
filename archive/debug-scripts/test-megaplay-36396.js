const fetch = require('node-fetch');
const UA = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36';

(async () => {
  // Test getSources with data-id=36396
  console.log('=== getSources?id=36396 ===');
  const res = await fetch('https://megaplay.buzz/stream/getSources?id=36396', {
    headers: { 
      'User-Agent': UA, 
      'X-Requested-With': 'XMLHttpRequest',
      'Referer': 'https://megaplay.buzz/stream/s-2/2142/sub'
    }
  });
  const data = await res.json().catch(() => ({}));
  console.log('Status:', res.status);
  console.log('Response:', JSON.stringify(data, null, 2).substring(0, 500));
  
  if (data.sources?.file) {
    console.log('\nSource file:', data.sources.file);
    
    // Compare with 2142
    console.log('\n=== getSources?id=2142 ===');
    const res2 = await fetch('https://megaplay.buzz/stream/getSources?id=2142', {
      headers: { 
        'User-Agent': UA, 
        'X-Requested-With': 'XMLHttpRequest',
        'Referer': 'https://megaplay.buzz/'
      }
    });
    const data2 = await res2.json().catch(() => ({}));
    console.log('Status:', res2.status);
    console.log('Source file:', data2.sources?.file);
    
    console.log('\nAre they different?', data.sources.file !== data2.sources?.file);
  }
})();
