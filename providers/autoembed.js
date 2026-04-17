/**
 * AutoEmbed Provider (codename: Blaze)
 * Fast API-based provider using the vixsrc.to API chain.
 * No browser/Playwright required — pure HTTP extraction.
 *
 * Flow:
 *  1. GET /api/{type}/{tmdbId} → returns { src: "/embed/...?token=..." }
 *  2. GET the embed URL → HTML with window.masterPlaylist + window.streams
 *  3. Extract playlist URLs + auth params → construct m3u8 URLs
 */

const axios = require('axios');

const BASE_URL = 'https://vixsrc.to';
const USER_AGENT = 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36';

const http = axios.create({
  timeout: 12000,
  headers: { 'User-Agent': USER_AGENT },
  validateStatus: () => true,
});

/**
 * Step 1: Call the vixsrc API to get the embed path with token.
 */
async function getEmbedPath(tmdbId, mediaType, season, episode) {
  const typePath = mediaType === 'movie' ? 'movie' : 'tv';
  let apiUrl = `${BASE_URL}/api/${typePath}/${tmdbId}`;
  if (typePath === 'tv') {
    apiUrl += `/${season || 1}/${episode || 1}`;
  }

  console.log(`[Blaze] API call: ${apiUrl}`);

  const resp = await http.get(apiUrl, {
    headers: { Referer: `${BASE_URL}/`, Accept: 'application/json' },
  });

  if (resp.status !== 200 || !resp.data || !resp.data.src) {
    console.log(`[Blaze] API returned no src (status ${resp.status})`);
    return null;
  }

  const embedPath = resp.data.src;
  const fullUrl = embedPath.startsWith('/') ? `${BASE_URL}${embedPath}` : embedPath;
  console.log(`[Blaze] Embed URL: ${fullUrl.substring(0, 80)}...`);
  return fullUrl;
}

/**
 * Step 2: Fetch the embed page and extract window.masterPlaylist / window.streams.
 */
async function extractFromEmbedPage(embedUrl) {
  const resp = await http.get(embedUrl, {
    headers: {
      Referer: `${BASE_URL}/`,
      Accept: 'text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8',
    },
  });

  if (resp.status !== 200) {
    console.log(`[Blaze] Embed page returned ${resp.status}`);
    return [];
  }

  const html = typeof resp.data === 'string' ? resp.data : '';
  if (!html) return [];

  const streams = [];

  // Extract auth params from masterPlaylist first — all playlist URLs need them
  let authToken = null;
  let authExpires = null;
  if (html.includes('window.masterPlaylist')) {
    const masterMatch = html.match(/window\.masterPlaylist\s*=\s*(\{[\s\S]*?\n\s*\})\s*[;\n]/);
    if (masterMatch) {
      const block = masterMatch[1];
      const tokenMatch = block.match(/['"]?token['"]?\s*:\s*['"]([^'"]+)['"]/);
      const expiresMatch = block.match(/['"]?expires['"]?\s*:\s*['"]([^'"]+)['"]/);
      const urlMatch = block.match(/url:\s*['"]([^'"]+)['"]/);
      if (tokenMatch) authToken = tokenMatch[1];
      if (expiresMatch) authExpires = expiresMatch[1];

      if (urlMatch) {
        let masterUrl = urlMatch[1];
        if (authToken && authExpires) {
          const sep = masterUrl.includes('?') ? '&' : '?';
          masterUrl += `${sep}token=${authToken}&expires=${authExpires}&h=1&lang=en`;
        }
        streams.push({
          url: masterUrl,
          quality: '1080p',
          format: 'hls',
          serverName: 'Master',
        });
      }
    }
  }

  // Helper to append auth params to a playlist URL
  function withAuth(url) {
    if (!authToken || !authExpires) return url;
    const sep = url.includes('?') ? '&' : '?';
    return `${url}${sep}token=${authToken}&expires=${authExpires}&h=1&lang=en`;
  }

  // Extract window.streams — array of server objects with playlist URLs
  const streamsMatch = html.match(/window\.streams\s*=\s*(\[[\s\S]*?\]);/);
  if (streamsMatch) {
    try {
      const servers = JSON.parse(streamsMatch[1]);
      for (const server of servers) {
        if (server.url) {
          const authedUrl = withAuth(server.url);
          if (!streams.some(s => s.url === authedUrl)) {
            streams.push({
              url: authedUrl,
              quality: 'Auto',
              format: 'hls',
              serverName: server.name || 'Server',
            });
          }
        }
      }
    } catch (e) {
      console.log(`[Blaze] Failed to parse window.streams: ${e.message}`);
    }
  }

  // Fallback: scan for any m3u8 URLs in the page
  if (streams.length === 0) {
    const m3u8Matches = html.match(/(https?:\/\/[^'"\s]+\.m3u8[^'"\s]*)/g);
    if (m3u8Matches) {
      const seen = new Set();
      m3u8Matches.forEach(url => {
        if (!seen.has(url)) {
          seen.add(url);
          streams.push({ url, quality: 'Auto', format: 'hls', serverName: 'Direct' });
        }
      });
    }
  }

  return streams;
}

/**
 * Subtitle extraction from the embed page.
 */
function extractSubtitles(html) {
  // Look for subtitle URLs in the page
  const subtitlePatterns = [
    /window\.subtitleUrl\s*=\s*['"]([^'"]+)['"]/,
    /subtitles:\s*\[([^\]]*)\]/,
    /subtitle[s]?\s*[:=]\s*['"]([^'"]+)['"]/,
  ];

  for (const pattern of subtitlePatterns) {
    const match = html.match(pattern);
    if (match && match[1]) {
      return [{ url: match[1], lang: 'en', label: 'English' }];
    }
  }

  return [];
}

/**
 * Main provider function — compatible with the registry's standard call signature.
 */
async function getStreams(tmdbId, mediaType, season, episode) {
  console.log(`[Blaze] Fetching TMDB ${tmdbId} (${mediaType}) S${season || '-'}E${episode || '-'}`);

  try {
    // Step 1: Get embed path from API
    const embedUrl = await getEmbedPath(tmdbId, mediaType, season, episode);
    if (!embedUrl) return [];

    // Step 2: Extract streams from embed page
    const rawStreams = await extractFromEmbedPage(embedUrl);
    if (rawStreams.length === 0) {
      console.log(`[Blaze] No streams found on embed page`);
      return [];
    }

    // Step 3: Format for registry compatibility
    const formatted = rawStreams.map(s => ({
      title: `Blaze - ${s.serverName} - ${s.quality}`,
      url: s.url,
      quality: s.quality,
      provider: 'Blaze',
      format: s.format || 'hls',
      headers: {
        Referer: `${BASE_URL}/`,
        Origin: BASE_URL,
        'User-Agent': USER_AGENT,
      },
    }));

    console.log(`[Blaze] Returning ${formatted.length} stream(s) for TMDB ${tmdbId}`);
    return formatted;
  } catch (e) {
    console.error(`[Blaze] Error: ${e.message}`);
    return [];
  }
}

module.exports = { getStreams };