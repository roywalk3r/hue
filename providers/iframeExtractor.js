/**
 * Hue Stream API
 * Iframe-to-native stream extractor.
 *
 * Converts anime/donghua iframe embed URLs into direct HLS/MP4 links where
 * the host exposes them in page source or a public API.
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @license MIT
 */

const axios = require('axios');
const http = require('http');
const https = require('https');
const { URL } = require('url');

const USER_AGENT = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/137.0.0.0 Safari/537.36';

const httpAgent = new http.Agent({ family: 4, keepAlive: false });
const httpsAgent = new https.Agent({ family: 4, keepAlive: false });

const AD_HOSTS = [
  'doubleclick.net',
  'googleadservices.com',
  'googlesyndication.com',
  'adsystem',
  'adnxs.com',
  'adsrvr.org',
  'popads.net',
  ' PropellerAds',
  'exosrv.com',
  'juicyads.com',
  'onclickads.net',
  'revcontent.com',
  'taboola.com',
  'outbrain.com',
  'advertising.com',
  'amazon-adsystem.com',
  'facebook.com/tr',
  'scorecardresearch.com',
  'googletagmanager.com',
  'google-analytics.com',
];

function looksLikeAd(url) {
  const lower = url.toLowerCase();
  return AD_HOSTS.some(h => lower.includes(h));
}

async function httpGet(url, headers = {}, opts = {}) {
  return axios.get(url, {
    timeout: 20000,
    maxRedirects: 10,
    httpAgent,
    httpsAgent,
    headers: {
      'User-Agent': USER_AGENT,
      'Accept': 'text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,*/*;q=0.8',
      'Accept-Language': 'en-US,en;q=0.9',
      ...headers,
    },
    ...opts,
  });
}

function inferStreamType(url) {
  const lower = url.split('?')[0].toLowerCase();
  if (lower.includes('.m3u8')) return 'hls';
  if (lower.includes('.mpd')) return 'dash';
  if (lower.includes('.mp4') || lower.includes('.mkv') || lower.includes('.webm')) return 'mp4';
  return 'mp4';
}

/**
 * Extract a direct stream URL from an iframe embed page.
 * @param {string} url - The iframe embed URL.
 * @param {string} [referer] - The referer of the embedding page.
 * @returns {Promise<{url: string, streamType: string, referer: string}|null>}
 */
async function extractIframe(url, referer) {
  if (!url || typeof url !== 'string') return null;

  const lower = url.toLowerCase();

  try {
    if (isDailymotionHost(lower)) return await dailymotionExtract(url, referer);
    if (isOkruHost(lower)) return await okruExtract(url, referer);
    if (isStreamtapeHost(lower)) return await streamtapeExtract(url, referer);
    if (isMp4UploadHost(lower)) return await mp4uploadExtract(url, referer);
    if (isMixdropHost(lower)) return await mixdropExtract(url, referer);
    if (isStreamwishHost(lower)) return await streamwishExtract(url, referer);

    return await genericExtract(url, referer);
  } catch (e) {
    console.error(`[iframeExtractor] ${url} -> ${e.message}`);
    return null;
  }
}

function isDailymotionHost(url) {
  return url.includes('dailymotion.com');
}

function isOkruHost(url) {
  return url.includes('ok.ru') || url.includes('odnoklassniki.ru');
}

function isStreamtapeHost(url) {
  return url.includes('streamtape.') || url.includes('tapecontent.') || url.includes('streamta.pe');
}

function isMp4UploadHost(url) {
  return url.includes('mp4upload.');
}

function isMixdropHost(url) {
  return /mixdrop|m1xdrop|mixdrp|mixdroop/i.test(url);
}

function isStreamwishHost(url) {
  return /streamwish|seekplayer|wishplayer|embedwish|wishembed|filelions|vidhide|lionplayer/i.test(url);
}

// ═════════════════════════════════════════════════════════════════════════════
// Dailymotion
// ═════════════════════════════════════════════════════════════════════════════

async function dailymotionExtract(url, sourceReferer) {
  const videoId = extractDailymotionId(url);
  if (!videoId) throw new Error('dailymotion: video id not found');

  const metaUrl = `https://www.dailymotion.com/player/metadata/video/${videoId}`;
  const resp = await httpGet(metaUrl, {
    'Referer': sourceReferer || 'https://www.dailymotion.com/',
    'Accept': 'application/json',
  }, {
    headers: {
      'Cookie': 'family_filter=off; ff=off',
    },
  });

  const data = resp.data;
  const auto = data?.qualities?.auto;
  if (Array.isArray(auto)) {
    const hls = auto.find(q => q.type === 'application/x-mpegURL' || q.url?.includes('.m3u8'));
    if (hls?.url) {
      return {
        url: hls.url,
        streamType: 'hls',
        referer: 'https://www.dailymotion.com/',
      };
    }
  }

  throw new Error('dailymotion: no hls manifest found');
}

function extractDailymotionId(url) {
  const m = url.match(/(?:\/embed\/video\/|\/video\/)([a-zA-Z0-9]+)/);
  return m ? m[1] : null;
}

// ═════════════════════════════════════════════════════════════════════════════
// OK.ru (Odnoklassniki)
// ═════════════════════════════════════════════════════════════════════════════

async function okruExtract(url, sourceReferer) {
  const referer = sourceReferer || 'https://ok.ru/';
  const resp = await httpGet(url, { 'Referer': referer });
  const html = resp.data;

  // Try the newer "metadata":"..." escaped JSON first.
  const meta = extractOkruMetadata(html);
  if (meta) {
    const m3u8 = meta.hlsManifestUrl || meta.ondemandHls || meta.hlsMasterPlaylistUrl;
    if (m3u8) {
      return {
        url: m3u8,
        streamType: 'hls',
        referer: 'https://ok.ru/',
      };
    }
  }

  // Fallback: data-options player JSON.
  const playerMatch = html.match(/data-options=(['"])(?<player>\{.+?\})\1/s);
  if (playerMatch?.groups?.player) {
    const player = JSON.parse(decodeHtmlEntities(playerMatch.groups.player));
    const flashvars = player.flashvars;

    // Inline metadata JSON is the common case on embed pages.
    if (flashvars?.metadata) {
      const metadata = JSON.parse(flashvars.metadata);
      const m3u8 = metadata.hlsManifestUrl || metadata.ondemandHls || metadata.hlsMasterPlaylistUrl;
      if (m3u8) {
        return {
          url: m3u8,
          streamType: 'hls',
          referer: 'https://ok.ru/',
        };
      }
    }

    // Some pages reference a separate metadataUrl endpoint.
    const metadataUrl = flashvars?.metadataUrl;
    if (metadataUrl) {
      const metaResp = await httpGet(decodeURIComponent(metadataUrl), { 'Referer': referer });
      const metadata = metaResp.data;
      const m3u8 = metadata.hlsManifestUrl || metadata.ondemandHls || metadata.hlsMasterPlaylistUrl;
      if (m3u8) {
        return {
          url: m3u8,
          streamType: 'hls',
          referer: 'https://ok.ru/',
        };
      }
    }
  }

  throw new Error('ok.ru: no hls manifest found');
}

function extractOkruMetadata(html) {
  const marker = '"metadata":"';
  const idx = html.indexOf(marker);
  if (idx === -1) return null;

  let raw = '';
  let escaped = false;
  for (let i = idx + marker.length; i < html.length; i++) {
    const ch = html[i];
    if (escaped) {
      raw += ch;
      escaped = false;
      continue;
    }
    if (ch === '\\') {
      escaped = true;
      continue;
    }
    if (ch === '"') break;
    raw += ch;
  }

  try {
    const json = raw.replace(/\\"/g, '"').replace(/\\n/g, '\n').replace(/\\\//g, '/').replace(/\\\\/g, '\\');
    return JSON.parse(json);
  } catch (e) {
    return null;
  }
}

function decodeHtmlEntities(str) {
  return str
    .replace(/&quot;/g, '"')
    .replace(/&amp;/g, '&')
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .replace(/&#(\d+);/g, (_, code) => String.fromCharCode(code));
}

// ═════════════════════════════════════════════════════════════════════════════
// Streamtape
// ═════════════════════════════════════════════════════════════════════════════

async function streamtapeExtract(url, sourceReferer) {
  const u = new URL(url);
  if (u.pathname.startsWith('/e/')) {
    u.pathname = u.pathname.replace('/e/', '/v/');
  }
  const viewUrl = u.toString();
  const referer = sourceReferer || viewUrl;

  const resp = await httpGet(viewUrl, { 'Referer': referer });
  const html = resp.data;

  const robotMatch = html.match(/document\.getElementById\(['"]norobotlink['"]\)\.innerHTML\s*=\s*["']([^"']+)["']/);
  if (!robotMatch) throw new Error('streamtape: norobotlink not found');

  const tokenParts = robotMatch[1].split(' ').filter(Boolean);
  if (tokenParts.length < 2) throw new Error('streamtape: token malformed');

  const ideoMatch = html.match(/id=["']ideoooolink["'][^>]*>([^<]+)/);
  const ideoPath = ideoMatch ? ideoMatch[1] : '';

  // Reconstruct the signed path. The page builds something like:
  //   https:/{ideoPath}&token={tokenParts[1]}&token={tokenParts[0]}
  const gateUrl = `https://${ideoPath}&token=${tokenParts[1]}&token=${tokenParts[0]}`;

  const gateResp = await httpGet(gateUrl, { 'Referer': viewUrl }, { maxRedirects: 0, validateStatus: s => s >= 200 && s < 400 });
  if (gateResp.status >= 300 && gateResp.headers.location) {
    return {
      url: gateResp.headers.location,
      streamType: 'mp4',
      referer: viewUrl,
    };
  }

  throw new Error('streamtape: gate did not redirect');
}

// ═════════════════════════════════════════════════════════════════════════════
// MP4Upload
// ═════════════════════════════════════════════════════════════════════════════

async function mp4uploadExtract(url, sourceReferer) {
  const referer = sourceReferer || url;
  const resp = await httpGet(url, { 'Referer': referer });
  const html = resp.data;

  const scripts = html.match(/<script[^>]*>([\s\S]*?)<\/script>/gi) || [];
  const lastScript = scripts[scripts.length - 1];
  if (!lastScript) throw new Error('mp4upload: no script tags');

  const scriptBody = lastScript.replace(/<script[^>]*>|<\/script>/gi, '');
  const parts = scriptBody.split('|').filter(Boolean);

  // Find video id, domain, and port by scanning the pipe array.
  const videoId = parts.find(p => /^[a-z0-9]{10,}$/i.test(p));
  const port = parts.find(p => /^\d{2,5}$/.test(p));
  const domain = parts.find(p => p.endsWith('.mp4upload.com') || p.endsWith('mp4upload.com'));

  if (!videoId || !domain || !port) throw new Error('mp4upload: failed to parse pipe config');

  const cleanDomain = domain.replace(/^https?:\/\//, '').replace(/\/.*$/, '');
  const directUrl = `https://${cleanDomain}:${port}/d/${videoId}/video.mp4`;

  return {
    url: directUrl,
    streamType: 'mp4',
    referer: url,
  };
}

// ═════════════════════════════════════════════════════════════════════════════
// Mixdrop
// ═════════════════════════════════════════════════════════════════════════════

async function mixdropExtract(url, sourceReferer) {
  const referer = sourceReferer || url;
  const resp = await httpGet(url, { 'Referer': referer });
  const html = resp.data;

  const packed = extractPackedJs(html);
  if (!packed) throw new Error('mixdrop: no packed js found');

  const unpacked = unpackJs(packed);

  const wurlMatch = unpacked.match(/wurl\s*=\s*["']([^"']+)["']/);
  const mdcoreMatches = unpacked.matchAll(/MDCore\.\w+\s*=\s*["']([^"']+)["']/g);

  let videoUrl = null;
  if (wurlMatch) {
    videoUrl = wurlMatch[1];
  } else {
    for (const m of mdcoreMatches) {
      if (m[1].includes('.mp4') || m[1].includes('http')) {
        videoUrl = m[1];
        break;
      }
    }
  }

  if (!videoUrl) throw new Error('mixdrop: no video url in unpacked js');

  const finalUrl = videoUrl.startsWith('//') ? `https:${videoUrl}` : videoUrl;

  return {
    url: finalUrl,
    streamType: inferStreamType(finalUrl),
    referer: url,
  };
}

function extractPackedJs(html) {
  const m = html.match(/eval\(function\(p,a,c,k,e,?d\)\{[\s\S]*?\}\s*\(['"](.*?)['"]\s*,\s*\d+\s*,\s*\d+\s*,\s*['"]([\s\S]*?)['"]\)\)/);
  return m ? m[0] : null;
}

function unpackJs(packed) {
  const payloadMatch = packed.match(/\(['"](.*?)['"]\s*,\s*(\d+)\s*,\s*(\d+)\s*,\s*['"]([\s\S]*?)['"]\.split\(['"]\|['"]\)/);
  if (!payloadMatch) throw new Error('mixdrop: failed to parse packed payload');

  const p = payloadMatch[1];
  const a = parseInt(payloadMatch[2], 10);
  const c = parseInt(payloadMatch[3], 10);
  const symString = payloadMatch[4];
  const symbols = symString.split('|');

  const base = a;
  const dict = {};
  while (c--) {
    if (symbols[c]) {
      dict[toBase(c, base)] = symbols[c];
    }
  }

  return p.replace(/\b\w+\b/g, word => dict[word] || word);
}

function toBase(num, base) {
  const digits = '0123456789abcdefghijklmnopqrstuvwxyz';
  let result = '';
  do {
    result = digits[num % base] + result;
    num = Math.floor(num / base);
  } while (num > 0);
  return result || '0';
}

// ═════════════════════════════════════════════════════════════════════════════
// Streamwish / Filelions / VidHide
// ═════════════════════════════════════════════════════════════════════════════

async function streamwishExtract(url, sourceReferer) {
  // Streamwish uses an encrypted /api/v1/video?id=<hash> response plus a
  // JS unpacker. Until we port the unpacker or use a headless helper, fall
  // back to generic regex scan of the embed page for any inlined sources.
  return genericExtract(url, sourceReferer);
}

// ═════════════════════════════════════════════════════════════════════════════
// Generic fallback: regex scan for m3u8/mp4
// ═════════════════════════════════════════════════════════════════════════════

async function genericExtract(url, sourceReferer) {
  const referer = sourceReferer || url;
  const resp = await httpGet(url, { 'Referer': referer });
  const html = resp.data;

  const regex = /https?:\/\/[^\s"'<>{}|\[\]\^`]+\.(?:m3u8|mp4|mkv|webm)(?:\?[^\s"'<>{}|\[\]\^`]*)?/gi;
  const matches = [...html.matchAll(regex)].map(m => m[0]);

  for (const candidate of matches) {
    if (looksLikeAd(candidate)) continue;
    return {
      url: candidate,
      streamType: inferStreamType(candidate),
      referer: url,
    };
  }

  throw new Error('generic: no direct stream url found');
}

module.exports = {
  extractIframe,
  isDailymotionHost,
  isOkruHost,
  isStreamtapeHost,
  isMp4UploadHost,
  isMixdropHost,
  isStreamwishHost,
};
