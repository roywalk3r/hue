/**
 * HUE ENGINE ❄
 * System Designation: The Architecture of Aggregation
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */

const fetch = require('node-fetch');

const DOMAINS = ['https://hianimes.se', 'https://hianime.ph', 'https://hianime.lc'];
const UA = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/123.0.0.0 Safari/537.36';

async function fetchText(url, opts = {}) {
  try {
    const res = await fetch(url, {
      ...opts,
      headers: { 'User-Agent': UA, Referer: DOMAINS[0] + '/', ...opts.headers },
      timeout: 15000,
    });
    if (!res.ok) throw new Error(`HTTP ${res.status}`);
    return await res.text();
  } catch (e) {
    return null;
  }
}

async function fetchJSON(url, opts = {}) {
  const text = await fetchText(url, opts);
  if (!text) return null;
  try { return JSON.parse(text); } catch { return null; }
}

async function findWorkingDomain() {
  for (const d of DOMAINS) {
    try {
      const res = await fetch(d + '/home', { headers: { 'User-Agent': UA }, timeout: 8000 });
      if (res.ok) {
        console.log(`[OK] ${d}`);
        return d;
      }
    } catch {}
  }
  return DOMAINS[0];
}

async function main() {
  const base = await findWorkingDomain();
  const watchUrl = `${base}/watch/one-piece-episode-1161-hef5mk`;

  console.log(`\n=== Fetching ${watchUrl} ===`);
  const html = await fetchText(watchUrl);
  if (!html) {
    console.log('Failed to fetch watch page');
    return;
  }

  // Look for data embedded in the page
  console.log('\n--- Looking for server data ---');
  const serverMatch = html.match(/data-id="(\d+)"/g);
  if (serverMatch) {
    console.log('Server IDs found:', [...new Set(serverMatch)].slice(0, 10));
  }

  // Look for any encrypted/obfuscated data
  const encryptedMatch = html.match(/"sources"\s*:\s*"([^"]+)"/);
  if (encryptedMatch) {
    console.log('Encrypted sources found:', encryptedMatch[1].substring(0, 100));
  }

  // Look for AJAX endpoints
  const ajaxMatch = html.match(/\/ajax\/[^"']+/g);
  if (ajaxMatch) {
    console.log('AJAX endpoints:', [...new Set(ajaxMatch)].slice(0, 10));
  }

  // Look for episode ID
  const epIdMatch = html.match(/episodeId["']?\s*[:=]\s*["']?([a-f0-9]{24})["']?/);
  if (epIdMatch) {
    console.log('Episode ID:', epIdMatch[1]);
  }

  // Look for any script that might contain decryption logic
  const scriptMatch = html.match(/<script[^>]*>([\s\S]*?)<\/script>/g);
  if (scriptMatch) {
    for (const script of scriptMatch) {
      if (script.includes('getSources') || script.includes('sources') || script.includes('encrypt') || script.includes('decrypt')) {
        const content = script.replace(/<script[^>]*>/, '').replace(/<\/script>/, '');
        if (content.length > 50 && content.length < 2000) {
          console.log('\n--- Relevant script snippet ---');
          console.log(content.substring(0, 500));
        }
      }
    }
  }

  // Try the old-style AJAX servers endpoint
  const epId = 'hef5mk'; // from the URL
  const ajaxServers = `${base}/ajax/v2/episode/servers?episodeId=${epId}`;
  console.log(`\n=== Trying ${ajaxServers} ===`);
  const serversData = await fetchJSON(ajaxServers, { headers: { 'X-Requested-With': 'XMLHttpRequest' } });
  if (serversData) {
    console.log('Servers response keys:', Object.keys(serversData));
    if (serversData.html) {
      const ids = serversData.html.match(/data-id="(\d+)"/g);
      console.log('Server IDs from AJAX:', ids ? [...new Set(ids)] : 'none');
    }
  } else {
    console.log('AJAX servers endpoint failed');
  }

  // Try the newer hianime API pattern
  console.log('\n=== Checking for hianime API in page ===');
  const apiMatch = html.match(/https?:\/\/[^"'\s]+\/api\/[^"'\s]+/g);
  if (apiMatch) {
    console.log('API URLs found:', [...new Set(apiMatch)].slice(0, 10));
  }

  // Check for script sources that might have the player logic
  const srcMatch = html.match(/src="([^"]+\.js)"/g);
  if (srcMatch) {
    console.log('\nJS chunks:', srcMatch.slice(0, 5));
  }

  // Look for NEXT_DATA
  const nextData = html.match(/<script id="__NEXT_DATA__"[^>]*>([\s\S]*?)<\/script>/);
  if (nextData) {
    console.log('\n--- __NEXT_DATA__ found ---');
    try {
      const data = JSON.parse(nextData[1]);
      console.log('Keys:', Object.keys(data));
      if (data.props?.pageProps) {
        console.log('pageProps keys:', Object.keys(data.props.pageProps));
      }
    } catch (e) {
      console.log('Failed to parse __NEXT_DATA__');
    }
  }

  // Look for any JSON that might contain server info
  const jsonMatches = html.match(/{[^{}]*"sources"[^{}]*}/g);
  if (jsonMatches) {
    console.log('\n--- Inline JSON with sources ---');
    for (const j of jsonMatches.slice(0, 3)) {
      console.log(j.substring(0, 200));
    }
  }
}

main().catch(console.error).finally(() => process.exit());
