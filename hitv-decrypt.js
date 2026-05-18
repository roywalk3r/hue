#!/usr/bin/env node

/**
 * HUE ENGINE ❄
 * System Designation: The Architecture of Aggregation
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */

/**
 * HiTV API Decryptor
 *
 * Usage:
 *   node hitv-decrypt.js <url>                          # Decrypt all API responses
 *   node hitv-decrypt.js <url> --endpoint rslv          # Only video stream URLs
 *   node hitv-decrypt.js <url> --endpoint detail        # Only episode details
 *   node hitv-decrypt.js <url> --json                   # Output as JSON (for piping)
 *   node hitv-decrypt.js <url> --scroll                  # Scroll page to load more data
 *   node hitv-decrypt.js <url> --click-episode 2        # Click episode 2 before capturing
 *
 * Key derivation:
 *   key = MD5(MD5(did + ts) + salt).slice(0, 16)
 *   iv  = MD5(MD5(did + ts) + salt).slice(16)
 *   salt = "Wcb26arWkvkcAZc378eR"
 *   did = hpwa_did cookie (auto-generated per session)
 */

const { chromium } = require('playwright');
const { decrypt, parseEncryptedResponse } = require('./utils/hitv-crypto');

function getEndpointName(url) {
  // Match longest paths first to avoid false positives
  const patterns = [
    ['/seriesAlbum/detail', 'seriesAlbum/detail'],
    ['/episode/detail', 'episode/detail'],
    ['/series/rslv', 'series/rslv'],
    ['/series/stats', 'series/stats'],
    ['/series/sync', 'series/sync'],
    ['/navigation/get', 'navigation/get'],
    ['/user/mine/get', 'user/mine/get'],
    ['/search/config', 'search/config'],
    ['/common/configs', 'common/configs'],
    ['/home/', 'home'],
    ['/recommend/', 'recommend'],
    ['/category/', 'category'],
    ['/tag/', 'tag'],
  ];
  for (const [pattern, name] of patterns) {
    if (url.includes(pattern)) return name;
  }
  const match = url.match(/\/s1\/w\/(.+?)[\?]/);
  return match ? match[1] : url;
}

function parseArgs(argv) {
  const args = { url: null, endpoint: 'all', json: false, scroll: false, clickEpisode: null, headless: true, api: null, rawTs: null, rawDid: null, rawData: null };
  for (let i = 2; i < argv.length; i++) {
    if (argv[i] === '--endpoint' && argv[i + 1]) { args.endpoint = argv[++i]; continue; }
    if (argv[i] === '--json') { args.json = true; continue; }
    if (argv[i] === '--scroll') { args.scroll = true; continue; }
    if (argv[i] === '--click-episode' && argv[i + 1]) { args.clickEpisode = parseInt(argv[++i]); continue; }
    if (argv[i] === '--no-headless') { args.headless = false; continue; }
    if (argv[i] === '--api' && argv[i + 1]) { args.api = argv[++i]; continue; }
    if (argv[i] === '--ts' && argv[i + 1]) { args.rawTs = argv[++i]; continue; }
    if (argv[i] === '--did' && argv[i + 1]) { args.rawDid = argv[++i]; continue; }
    if (argv[i] === '--data' && argv[i + 1]) { args.rawData = argv[++i]; continue; }
    if (argv[i] === '--help') { return args; }
    if (!args.url && !argv[i].startsWith('--')) { args.url = argv[i]; }
  }
  return args;
}

async function main() {
  const args = parseArgs(process.argv);

  // Raw decryption mode: decrypt a captured response directly (no browser needed)
  if (args.rawData && args.rawTs && args.rawDid) {
    const decrypted = decrypt(args.rawData, args.rawTs, args.rawDid);
    if (decrypted) {
      try { console.log(JSON.stringify(JSON.parse(decrypted), null, 2)); }
      catch { console.log(decrypted); }
    } else {
      console.error('Decryption failed. Check did/ts/data.');
      process.exit(1);
    }
    return;
  }

  // API fetch mode: use browser to fetch a specific API URL and decrypt
  if (args.api && !args.url) {
    args.url = 'https://home.hitv.vip/';
  }

  if (!args.url) {
    console.log(`HiTV API Decryptor - Decrypt HiTV encrypted API responses

Usage: node hitv-decrypt.js <url> [options]
       node hitv-decrypt.js --api <api-path> [options]   # Fetch & decrypt a specific API
       node hitv-decrypt.js --did <did> --ts <ts> --data <base64>  # Decrypt raw data

Options:
  --endpoint <name>   Filter by endpoint: all, rslv, detail, album, stats, sync (default: all)
  --api <path>        Fetch a specific API path, e.g. /s1/w/routine/api/seriesAlbum/detail?aid=...
  --json              Output as JSON for piping
  --scroll            Scroll page to trigger lazy-loaded API calls
  --click-episode <n> Click episode number N before capturing
  --no-headless       Show browser window (for debugging)
  --did <did>         Device ID for raw decryption
  --ts <timestamp>    Timestamp for raw decryption
  --data <base64>     Encrypted data for raw decryption

Examples:
  node hitv-decrypt.js https://home.hitv.vip/series/burning-desire
  node hitv-decrypt.js https://home.hitv.vip/series/burning-desire --endpoint rslv --json
  node hitv-decrypt.js https://home.hitv.vip/series/burning-desire --click-episode 2
  node hitv-decrypt.js --api /s1/w/routine/api/seriesAlbum/detail?aid=a_sSaCsLFo1e_412f0cCSn
  node hitv-decrypt.js --did abc123 --ts 1778231552189 --data GCTumNn7...
  node hitv-decrypt.js https://home.hitv.vip/ --scroll --json > home.json`);
    process.exit(args.url === undefined ? 1 : 0);
  }

  const browser = await chromium.launch({ headless: args.headless });
  const context = await browser.newContext();
  const page = await context.newPage();

  const results = [];

  await page.route('**/s1/w/**', async (route) => {
    const response = await route.fetch();
    try {
      const body = await response.text();
      const json = JSON.parse(body);
      if (json.ts && json.data) {
        const did = await page.evaluate(() => {
          const m = document.cookie.match(/hpwa_did=([^;]+)/);
          return m ? m[1] : '';
        });
        const endpointName = getEndpointName(route.request().url());

        if (args.endpoint === 'all' || endpointName.includes(args.endpoint)) {
          const decrypted = decrypt(json.data, json.ts.toString(), did);
          if (decrypted) {
            try {
              const parsed = JSON.parse(decrypted);
              results.push({ endpoint: endpointName, did, ts: json.ts, data: parsed });
            } catch {
              results.push({ endpoint: endpointName, did, ts: json.ts, data: decrypted });
            }
          }
        }
      }
    } catch {}
    await route.fulfill({ response });
  });

  // Inject did cookie if provided (needed for auth-protected endpoints)
  if (args.rawDid) {
    const domain = new URL(args.url).hostname;
    await context.addCookies([{ name: 'hpwa_did', value: args.rawDid, domain, path: '/' }]);
    if (!args.json) console.log(`Injected hpwa_did cookie: ${args.rawDid}`);
  }

  // If --api flag provided, fetch that specific API endpoint after page loads
  if (args.api) {
    if (!args.json) console.log(`Fetching API: ${args.api}`);
    const apiBase = new URL(args.url).origin;
    const apiFull = args.api.startsWith('http') ? args.api : apiBase + args.api;
    // Use context.request to bypass CORS and include cookies
    const apiResp = await context.request.get(apiFull, {
      headers: { 'Cookie': `hpwa_did=${args.rawDid || ''}` }
    });
    const apiResult = await apiResp.text();
    try {
      const json = JSON.parse(apiResult);
      if (json.ts && json.data) {
        const did = args.rawDid || await page.evaluate(() => {
          const m = document.cookie.match(/hpwa_did=([^;]+)/);
          return m ? m[1] : '';
        });
        const decrypted = decrypt(json.data, json.ts.toString(), did);
        if (decrypted) {
          try {
            results.push({ endpoint: getEndpointName(args.api), did, ts: json.ts, data: JSON.parse(decrypted) });
          } catch {
            results.push({ endpoint: getEndpointName(args.api), did, ts: json.ts, data: decrypted });
          }
        }
      }
    } catch {}
    await browser.close();
    if (results.length === 0) {
      if (args.json) console.log(JSON.stringify({ error: 'No encrypted data in API response', raw: apiResult.substring(0, 200) }));
      else { console.log('No encrypted data in API response.'); console.log('Raw:', apiResult.substring(0, 200)); }
      return;
    }
    if (args.json) { console.log(JSON.stringify(results, null, 2)); return; }
    for (const r of results) {
      console.log(`=== ${r.endpoint} ===`);
      console.log(`  did: ${r.did}`);
      console.log(JSON.stringify(r.data, null, 2));
      console.log();
    }
    return;
  }

  if (!args.json) console.log(`Loading: ${args.url}`);
  await page.goto(args.url, { waitUntil: 'networkidle', timeout: 30000 });

  // Click specific episode if requested
  if (args.clickEpisode) {
    if (!args.json) console.log(`Clicking episode ${args.clickEpisode}...`);
    try {
      await page.getByRole('heading', { name: String(args.clickEpisode), exact: true }).click();
      await page.waitForTimeout(2000);
    } catch {
      // fallback: try link
      try {
        await page.locator(`text="${args.clickEpisode}"`).first().click();
        await page.waitForTimeout(2000);
      } catch {}
    }
  }

  // Scroll to load more content
  if (args.scroll) {
    if (!args.json) console.log('Scrolling to load more data...');
    for (let i = 0; i < 5; i++) {
      await page.evaluate(() => window.scrollTo(0, document.body.scrollHeight));
      await page.waitForTimeout(1500);
    }
  }

  await page.waitForTimeout(1000);
  await browser.close();

  if (results.length === 0) {
    if (args.json) {
      console.log(JSON.stringify({ error: 'No encrypted API responses found' }));
    } else {
      console.log('No encrypted API responses found.');
    }
    return;
  }

  if (args.json) {
    console.log(JSON.stringify(results, null, 2));
    return;
  }

  console.log(`\nDecrypted ${results.length} response(s):\n`);
  for (const r of results) {
    console.log(`=== ${r.endpoint} ===`);
    console.log(`  did: ${r.did}`);
    console.log(`  ts:  ${r.ts}`);
    console.log(JSON.stringify(r.data, null, 2));
    console.log();
  }
}

main().catch(e => {
  console.error('Error:', e.message);
  process.exit(1);
});