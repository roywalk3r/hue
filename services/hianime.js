const fetch = require('node-fetch');
const cheerio = require('cheerio');
const CryptoJS = require('crypto-js');

const BASE_URL = 'https://hianime.to';
const USER_AGENT =
  'Mozilla/5.0 (X11; Linux x86_64; rv:122.0) Gecko/20100101 Firefox/122.0';
const REFERER = 'https://megacloud.tv';

const KEY_URL =
  'https://raw.githubusercontent.com/ryanwtf88/megacloud-keys/refs/heads/master/key.txt';

const FALLBACK_PROVIDERS = [
  { name: 'megaplay', domain: 'megaplay.buzz' },
  { name: 'vidwish', domain: 'vidwish.live' },
];

const TIMEOUT = 15000;
const MAX_RETRIES = 2;
const KEY_CACHE_DURATION = 60 * 60 * 1000;

let cachedKey = null;
let keyLastFetched = 0;

async function fetchJSON(url, { headers = {}, timeout = TIMEOUT } = {}) {
  const controller = new AbortController();
  const id = setTimeout(() => controller.abort(), timeout);
  try {
    const res = await fetch(url, { headers, signal: controller.signal });
    if (!res.ok) throw new Error(`HTTP ${res.status}`);
    return await res.json();
  } finally {
    clearTimeout(id);
  }
}

async function fetchText(url, { headers = {}, timeout = TIMEOUT } = {}) {
  const controller = new AbortController();
  const id = setTimeout(() => controller.abort(), timeout);
  try {
    const res = await fetch(url, { headers, signal: controller.signal });
    if (!res.ok) throw new Error(`HTTP ${res.status}`);
    return await res.text();
  } finally {
    clearTimeout(id);
  }
}

async function getServers(id) {
  const episode = id.split('ep=').at(-1);
  const ajaxUrl = `/ajax/v2/episode/servers?episodeId=${episode}`;
  const referer = `/watch/${id.replace('::', '?')}`;
  const res = await fetchJSON(BASE_URL + ajaxUrl, {
    headers: {
      Referer: BASE_URL + referer,
      'User-Agent': USER_AGENT,
    },
  });
  return extractServers(res.html);
}

function extractServers(html) {
  const $ = cheerio.load(html);
  const episode = $('.server-notice strong b').text().trim().split(' ').at(-1);

  const extractServerList = (block) => {
    const servers = [];
    $(block)
      .find('.server-item')
      .each((_, element) => {
        const serverType = $(element).attr('data-type');
        const serverId = Number($(element).attr('data-id'));
        const serverName = $(element).find('a').text().trim().toLowerCase();
        const serverIndex = $(element).attr('data-server-id');
        servers.push({
          index: Number(serverIndex),
          type: serverType,
          id: serverId,
          name: serverName,
        });
      });

    ['megaplay', 'vidwish'].forEach((name) => {
      servers.push({
        index: null,
        type: block.includes('sub') ? 'sub' : 'dub',
        id: null,
        name: name,
      });
    });
    return servers;
  };

  return {
    episode: Number(episode),
    sub: extractServerList('.servers-sub .ps__-list'),
    dub: extractServerList('.servers-dub .ps__-list'),
  };
}

async function extractToken(url, retry = 0) {
  try {
    const html = await fetchText(url, {
      headers: {
        Referer: `${BASE_URL}/`,
        'User-Agent': USER_AGENT,
        Accept: 'text/html',
      },
      timeout: 10000,
    });
    const $ = cheerio.load(html);
    return (
      extractFromMeta($) ||
      extractFromDataAttr($) ||
      extractFromNonce($) ||
      extractFromWindowStrings(html) ||
      extractFromWindowObjects(html) ||
      extractFromComments($) ||
      null
    );
  } catch (err) {
    if (retry < 2) {
      await new Promise((res) => setTimeout(res, 1000 * (retry + 1)));
      return extractToken(url, retry + 1);
    }
    return null;
  }
}

const extractFromMeta = ($) => validateToken($('meta[name="_gg_fb"]').attr('content'));
const extractFromDataAttr = ($) => validateToken($('[data-dpi]').attr('data-dpi'));
const extractFromNonce = ($) =>
  validateToken(
    $('script[nonce]')
      .map((_, el) => $(el).attr('nonce'))
      .get()
      .find(Boolean)
  );

const extractFromWindowStrings = (html) => {
  const regex = /window\.\w+\s*=\s*["']([a-zA-Z0-9_-]{10,})["']/g;
  for (const match of html.matchAll(regex)) {
    const token = validateToken(match[1]);
    if (token) return token;
  }
};

const extractFromWindowObjects = (html) => {
  const regex = /window\.\w+\s*=\s*(\{[\s\S]*?\});/g;
  for (const match of html.matchAll(regex)) {
    try {
      const obj = new Function(`return ${match[1]}`)();
      if (!obj || typeof obj !== 'object') continue;
      const joined = Object.values(obj)
        .filter((v) => typeof v === 'string')
        .join('');
      const token = validateToken(joined, 20);
      if (token) return token;
    } catch {
      continue;
    }
  }
};

const extractFromComments = ($) => {
  let token = null;
  $('*')
    .contents()
    .each((_, node) => {
      if (node.type !== 'comment') return;
      const match = node.data.trim().match(/(?:_is_th|token|key):([a-zA-Z0-9_-]{10,})/);
      if (match) {
        token = match[1];
        return false;
      }
    });
  return validateToken(token);
};

const validateToken = (value, min = 10) =>
  typeof value === 'string' && value.length >= min ? value : null;

async function getDecryptionKey() {
  const now = Date.now();
  if (cachedKey && now - keyLastFetched < KEY_CACHE_DURATION) return cachedKey;
  const data = await fetchText(KEY_URL);
  cachedKey = data.trim();
  keyLastFetched = now;
  return cachedKey;
}

function decryptAES(encrypted, key) {
  const tryDecrypt = (k) => CryptoJS.AES.decrypt(encrypted, k).toString(CryptoJS.enc.Utf8);
  const decrypted = tryDecrypt(key) || tryDecrypt(CryptoJS.enc.Hex.parse(key));
  if (!decrypted) throw new Error('AES decryption failed');
  return JSON.parse(decrypted);
}

async function megacloud({ selectedServer, id }, retry = 0) {
  const epID = id.split('ep=').pop();
  try {
    const [sourcesResponse, key] = await Promise.all([
      fetchJSON(`${BASE_URL}/ajax/v2/episode/sources?id=${selectedServer.id}`),
      getDecryptionKey(),
    ]);

    const sourceId = /\/([^/?]+)\?/.exec(sourcesResponse.link)?.[1];
    const baseUrl = sourcesResponse.link.match(/^(https?:\/\/[^/]+(?:\/[^/]+){3})/)?.[1];
    if (!sourceId || !baseUrl) throw new Error('Invalid ajax link format');

    let decrypted;
    let rawData;
    let usedFallback = false;
    try {
      const token = await extractToken(`${baseUrl}/${sourceId}?k=1&autoPlay=0&oa=0&asi=1`);
      if (!token) throw new Error('Token extraction failed');
      const data = await fetchJSON(`${baseUrl}/getSources?id=${sourceId}&_k=${token}`, {
        headers: {
          'X-Requested-With': 'XMLHttpRequest',
          Referer: `${baseUrl}/${sourceId}`,
        },
      });
      const encrypted = data?.sources;
      if (!encrypted) throw new Error('Missing encrypted sources');
      decrypted = typeof encrypted === 'string' ? decryptAES(encrypted, key) : encrypted;
      rawData = data;
    } catch {
      const fallback = await getFallbackSource(epID, selectedServer.type, selectedServer.name);
      decrypted = fallback.sources;
      rawData = fallback.rawData;
      usedFallback = true;
    }

    if (!decrypted?.[0]?.file) throw new Error('Invalid decrypted sources');
    return {
      id,
      type: selectedServer.type,
      link: { file: decrypted[0].file, type: 'hls' },
      tracks: rawData?.tracks ?? [],
      intro: rawData?.intro ?? null,
      outro: rawData?.outro ?? null,
      server: selectedServer.name,
      usedFallback,
    };
  } catch (err) {
    if (retry < MAX_RETRIES) {
      await new Promise((res) => setTimeout(res, 2000 * (retry + 1)));
      return megacloud({ selectedServer, id }, retry + 1);
    }
    return null;
  }
}

async function getFallbackSource(epID, type, serverName) {
  const providers =
    serverName.toLowerCase() === 'hd-1'
      ? [FALLBACK_PROVIDERS[0], FALLBACK_PROVIDERS[1]]
      : [FALLBACK_PROVIDERS[1], FALLBACK_PROVIDERS[0]];

  for (const provider of providers) {
    try {
      const html = await fetchText(`https://${provider.domain}/stream/s-2/${epID}/${type}`, {
        headers: {
          Referer: `https://${provider.domain}/`,
          'User-Agent': USER_AGENT,
        },
      });
      const realId = html.match(/data-id=["'](\d+)["']/)?.[1];
      if (!realId) continue;
      const data = await fetchJSON(`https://${provider.domain}/stream/getSources?id=${realId}`, {
        headers: {
          'X-Requested-With': 'XMLHttpRequest',
          Referer: `https://${provider.domain}/`,
        },
      });
      if (data?.sources?.file) {
        return { sources: [{ file: data.sources.file }], rawData: data };
      }
    } catch {
      continue;
    }
  }

  throw new Error('All fallbacks failed');
}

async function attachSubtitlesFromSub(stream, server, id) {
  try {
    const allServers = await getServers(id);
    const subServer = allServers?.sub?.find(
      (s) => s.name === server.name || s.index === server.index
    );
    if (!subServer?.id) return;
    const subStream = await megacloud({ selectedServer: subServer, id });
    const subtitles =
      subStream?.tracks?.filter((t) => t.kind === 'captions' || t.kind === 'subtitles') ?? [];
    if (subtitles.length === 0) return;
    stream.tracks = [...(stream.tracks ?? []), ...subtitles];
  } catch {
    // ignore subtitle fallback errors
  }
}

async function streamExtract({ selectedServer, id }) {
  const isEmbedded = selectedServer.name === 'megaplay' || selectedServer.name === 'vidwish';
  if (isEmbedded) {
    const episodeId = id.split('ep=').pop();
    const endPath = `${episodeId}/${selectedServer.type}`;
    const endUrl =
      selectedServer.name === 'megaplay'
        ? `https://megaplay.buzz/stream/s-2/${endPath}`
        : `https://vidwish.live/stream/s-2/${endPath}`;
    return { streamingLink: endUrl, servers: selectedServer.name };
  }

  const stream = await megacloud({ selectedServer, id });
  if (!stream?.link?.file) return stream;

  if (selectedServer.type === 'dub') {
    const captions =
      stream.tracks?.filter((t) => t.kind === 'captions' || t.kind === 'subtitles') ?? [];
    if (captions.length === 0) {
      await attachSubtitlesFromSub(stream, selectedServer, id);
    }
  }

  return { ...stream, referer: REFERER };
}

async function getAnimeStream({ id, server, type }) {
  const servers = await getServers(id);
  const serverList = servers?.[type] || [];
  const defaultServerName = 'hd-1';
  const fallbackServerName = 'hd-2';
  const resolvedServer =
    server ||
    serverList.find((el) => el.name === defaultServerName)?.name ||
    serverList[0]?.name;
  const selectedServer = serverList.find((el) => el.name === resolvedServer);
  if (!selectedServer) {
    return { servers, stream: null, error: 'SERVER_NOT_FOUND' };
  }
  const isValidStream = (candidate) =>
    !!candidate && (candidate.streamingLink || candidate?.link?.file);

  let stream = await streamExtract({ selectedServer, id });
  if (
    !isValidStream(stream) &&
    !server &&
    resolvedServer === defaultServerName
  ) {
    const fallbackServer = serverList.find((el) => el.name === fallbackServerName);
    if (fallbackServer) {
      const fallbackStream = await streamExtract({ selectedServer: fallbackServer, id });
      if (isValidStream(fallbackStream)) {
        stream = fallbackStream;
      }
    }
  }

  return { servers, stream };
}

module.exports = {
  getServers,
  getAnimeStream,
};
