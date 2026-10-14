#!/usr/bin/env node

/**
 * No-browser vidsrc.to extractor for TMDB 550.
 */

require('dotenv').config();

const UA = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36';

async function fetchText(url, headers = {}, method = 'GET') {
  const res = await fetch(url, { method, headers, redirect: 'follow' });
  if (!res.ok) throw new Error(`${url} -> ${res.status}`);
  return { text: await res.text(), ct: res.headers.get('content-type') };
}

function findIframeSrc(html, baseUrl) {
  const match = html.match(/<iframe[^>]+src=["']([^"']+)["']/i);
  if (!match) return null;
  let src = match[1];
  if (src.startsWith('//')) src = 'https:' + src;
  if (src.startsWith('/') && !src.startsWith('//')) {
    try { src = new URL(src, baseUrl).href; } catch (_) {}
  }
  return src;
}

function findProrcpHash(html) {
  // The prorcp hash appears in the rcp HTML both as a path and in data-hash
  const match = html.match(/\/prorcp\/([A-Za-z0-9_~:-]+)/);
  if (match) return match[1];
  const dataMatch = html.match(/data-hash=["']([^"']+)["']/);
  if (dataMatch) return dataMatch[1];
  return null;
}

function extractStreamUrls(html) {
  const urls = [];
  const regex = /(https?:\/\/[^\s"'<>]+\/(?:pl|cdnstr)\/[^\s"'<>]+\.(?:m3u8|list\.m3u8))\?token=(__TOKEN(?:PG)?__)/gi;
  let m;
  while ((m = regex.exec(html)) !== null) {
    urls.push({ baseUrl: m[1], tokenPlaceholder: m[2], upstream: new URL(m[1]).origin });
  }
  return urls;
}

async function getToken(upstream, refererPath) {
  const generateUrl = `${upstream}/generate.php`;
  const res = await fetch(generateUrl, {
    method: 'GET',
    headers: {
      'User-Agent': UA,
      'Referer': `https://cloudorchestranova.com${refererPath}`,
      'Accept': '*/*',
      'Origin': 'https://cloudorchestranova.com',
    },
    redirect: 'follow',
  });
  if (!res.ok) throw new Error(`generate.php ${res.status}`);
  return (await res.text()).trim();
}

async function testStream(url) {
  const headers = {
    'User-Agent': UA,
    'Referer': 'https://cloudorchestranova.com/',
    'Accept': '*/*',
  };
  try {
    const res = await fetch(url, { method: 'HEAD', headers, redirect: 'follow' });
    console.log(`  HEAD ${res.status} ct=${res.headers.get('content-type')} len=${res.headers.get('content-length') || 'n/a'}`);
    return res.ok;
  } catch (e) {
    console.log(`  HEAD failed: ${e.message}`);
    return false;
  }
}

async function main() {
  const tmdbId = '550';
  const embedUrl = `https://vidsrc.to/embed/movie/${tmdbId}`;

  console.log(`[NoBrowser] Step 1: ${embedUrl}`);
  const { text: embedHtml } = await fetchText(embedUrl, {
    'User-Agent': UA,
    'Accept': 'text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8',
    'Referer': 'https://vidsrc.to/',
  });

  const vsembedUrl = findIframeSrc(embedHtml, embedUrl);
  if (!vsembedUrl) throw new Error('No vsembed.ru iframe found');
  console.log(`[NoBrowser] Step 2: ${vsembedUrl}`);

  const { text: vsembedHtml } = await fetchText(vsembedUrl, {
    'User-Agent': UA,
    'Accept': 'text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8',
    'Referer': embedUrl,
  });

  const rcpUrl = findIframeSrc(vsembedHtml, vsembedUrl);
  if (!rcpUrl) throw new Error('No cloudorchestranova rcp iframe found');
  console.log(`[NoBrowser] Step 3: ${rcpUrl}`);

  const { text: rcpHtml } = await fetchText(rcpUrl, {
    'User-Agent': UA,
    'Accept': 'text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8',
    'Referer': vsembedUrl,
  });

  const prorcpHash = findProrcpHash(rcpHtml);
  if (!prorcpHash) throw new Error('No /prorcp hash found in RCP HTML');
  const prorcpUrl = `https://cloudorchestranova.com/prorcp/${prorcpHash}`;
  console.log(`[NoBrowser] Step 4: ${prorcpUrl}`);

  const { text: prorcpHtml } = await fetchText(prorcpUrl, {
    'User-Agent': UA,
    'Accept': 'text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8',
    'Referer': rcpUrl,
  });

  const streamUrls = extractStreamUrls(prorcpHtml);
  if (streamUrls.length === 0) throw new Error('No stream URLs found in prorcp HTML');
  console.log(`[NoBrowser] Found ${streamUrls.length} stream URL template(s)`);

  const results = [];
  for (const tpl of streamUrls) {
    const token = await getToken(tpl.upstream, `/prorcp/${prorcpHash}`);
    const fullUrl = `${tpl.baseUrl}?token=${encodeURIComponent(token)}`;
    console.log(`\n[NoBrowser] Final URL (${tpl.upstream}):`);
    console.log(`  ${fullUrl.substring(0, 140)}...`);
    const ok = await testStream(fullUrl);
    results.push({ url: fullUrl, ok, upstream: tpl.upstream });
  }

  console.log('\n[NoBrowser] Results:');
  results.forEach(r => console.log(`  ${r.ok ? 'OK' : 'FAIL'} ${r.upstream}`));

  if (results.some(r => r.ok)) {
    console.log('\n[NoBrowser] Working stream found without browser!');
  }
}

main().catch(e => {
  console.error('[NoBrowser] Error:', e.message);
  process.exit(1);
});
