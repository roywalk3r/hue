import CryptoJS from 'crypto-js';
import config from '#config/config';
import extractToken from './token.js';

const { baseurl } = config;

/* =======================
   CONSTANTS
======================= */
const MAX_RETRIES = 2;
const TIMEOUT = 15000;
const KEY_CACHE_DURATION = 60 * 60 * 1000; // 1 hour
const HARDCODED_MEGACLOUD_KEY =
  '3709ad8892f413166b796a10c7fb86018bd1be1c7ae6f4d2cfc3fdc299cb3205';

const KEY_TEXT_URLS = [
  process.env.MEGACLOUD_KEY_URL,
  'https://raw.githubusercontent.com/itzzzme/megacloud-keys/refs/heads/main/key.txt',
  'https://raw.githubusercontent.com/ryanwtf88/megacloud-keys/master/key.txt',
  'https://raw.githubusercontent.com/ryanwtf88/megacloud-keys/main/key.txt',
  'https://raw.githubusercontent.com/ryanwtf88/megacloud-keys/refs/heads/master/key.txt',
]
  .filter(Boolean);
const KEY_JSON_URLS = [
  'https://raw.githubusercontent.com/yogesh-hacker/MegacloudKeys/refs/heads/main/keys.json',
];

const FALLBACK_PROVIDERS = [
  { name: 'megaplay', domain: 'megaplay.buzz' },
  { name: 'vidwish', domain: 'vidwish.live' },
];

/* =======================
   FETCH HELPERS (BUN)
======================= */
const fetchJSON = async (url, { headers = {}, timeout = TIMEOUT } = {}) => {
  const controller = new AbortController();
  const id = setTimeout(() => controller.abort(), timeout);

  try {
    const res = await fetch(url, {
      headers,
      signal: controller.signal,
    });

    if (!res.ok) {
      throw new Error(`HTTP ${res.status}`);
    }

    return await res.json();
  } finally {
    clearTimeout(id);
  }
};

const fetchText = async (url, { headers = {}, timeout = TIMEOUT } = {}) => {
  const controller = new AbortController();
  const id = setTimeout(() => controller.abort(), timeout);

  try {
    const res = await fetch(url, {
      headers,
      signal: controller.signal,
    });

    if (!res.ok) {
      throw new Error(`HTTP ${res.status}`);
    }

    return await res.text();
  } finally {
    clearTimeout(id);
  }
};

/* =======================
   KEY CACHE
======================= */
let cachedKey = null;
let keyLastFetched = 0;

/* =======================
   PUBLIC API
======================= */
export default async function megacloud({ selectedServer, id }, retry = 0) {
  const epID = extractEpisodeId(id);

  try {
    const sourcesResponse = await fetchAjaxSources(selectedServer.id);

    const { baseUrl, sourceId } = parseAjaxLink(sourcesResponse.link);

    let decrypted;
    let rawData;
    let usedFallback = false;

    try {
      ({ sources: decrypted, rawData } = await decryptPrimarySource(baseUrl, sourceId));
    } catch {
      ({ sources: decrypted, rawData } = await getFallbackSource(
        epID,
        selectedServer.type,
        selectedServer.name
      ));
      usedFallback = true;
    }

    validateSources(decrypted);

    return buildResult({
      id,
      server: selectedServer,
      file: decrypted[0].file,
      rawData,
      usedFallback,
    });
  } catch (err) {
    console.error(err.message);
    if (retry < MAX_RETRIES) {
      await backoff(retry);
      return megacloud({ selectedServer, id }, retry + 1);
    }
    return null;
  }
}

/* =======================
   CORE HELPERS
======================= */

const fetchAjaxSources = async (serverId) => {
  const data = await fetchJSON(`${baseurl}/ajax/v2/episode/sources?id=${serverId}`);

  if (!data?.link) throw new Error('Missing ajax link');
  return data;
};

const parseAjaxLink = (link) => {
  const sourceId = /\/([^/?]+)\?/.exec(link)?.[1];
  const baseUrl = link.match(/^(https?:\/\/[^/]+(?:\/[^/]+){3})/)?.[1];

  if (!sourceId || !baseUrl) {
    throw new Error('Invalid ajax link format');
  }

  return { sourceId, baseUrl };
};

const decryptPrimarySource = async (baseUrl, sourceId) => {
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

  const sources =
    typeof encrypted === 'string' ? await decryptAESWithFallback(encrypted) : encrypted;

  return { sources, rawData: data };
};

const decryptAES = (encrypted, key) => {
  const tryDecrypt = (k) => CryptoJS.AES.decrypt(encrypted, k).toString(CryptoJS.enc.Utf8);

  const decrypted = tryDecrypt(key) || tryDecrypt(CryptoJS.enc.Hex.parse(key));

  if (!decrypted) throw new Error('AES decryption failed');
  return JSON.parse(decrypted);
};

const isValidKey = (key) => typeof key === 'string' && /^[a-fA-F0-9]{32,64}$/.test(key.trim());

const normalizeKey = (value) => {
  const key = typeof value === 'string' ? value.trim() : '';
  if (!isValidKey(key)) {
    throw new Error('Invalid key format');
  }
  return key;
};

const getRemoteKey = async () => {
  for (const keyUrl of KEY_TEXT_URLS) {
    try {
      const data = await fetchText(keyUrl);
      const key = normalizeKey(data);
      cachedKey = key;
      keyLastFetched = Date.now();
      return key;
    } catch (err) {
      console.error(`[megacloud] key fetch failed from ${keyUrl}: ${err.message}`);
    }
  }

  for (const keyUrl of KEY_JSON_URLS) {
    try {
      const data = await fetchJSON(keyUrl);
      const key = normalizeKey(data?.mega || data?.vidstr || data?.rabbit);
      cachedKey = key;
      keyLastFetched = Date.now();
      return key;
    } catch (err) {
      console.error(`[megacloud] key fetch failed from ${keyUrl}: ${err.message}`);
    }
  }

  throw new Error('Key fetch failed');
};

const decryptAESWithFallback = async (encrypted) => {
  const keysToTry = [];

  try {
    keysToTry.push(normalizeKey(process.env.MEGACLOUD_KEY || HARDCODED_MEGACLOUD_KEY));
  } catch {
    // ignore invalid env override and continue with remote sources
  }

  if (cachedKey && Date.now() - keyLastFetched < KEY_CACHE_DURATION) {
    keysToTry.push(cachedKey);
  }

  for (const key of keysToTry) {
    try {
      return decryptAES(encrypted, key);
    } catch {
      // Try the next candidate or remote refresh below.
    }
  }

  const remoteKey = await getRemoteKey();
  return decryptAES(encrypted, remoteKey);
};

/* =======================
   FALLBACK
======================= */

const getFallbackSource = async (epID, type, serverName) => {
  const providers = prioritizeFallback(serverName);

  for (const provider of providers) {
    try {
      const html = await fetchFallbackHTML(provider, epID, type);
      const realId = extractDataId(html);
      if (!realId) continue;

      const { data } = await fetchFallbackSources(provider, realId);
      if (data?.sources?.file) {
        return {
          sources: [{ file: data.sources.file }],
          rawData: data,
        };
      }
    } catch {
      continue;
    }
  }

  throw new Error('All fallbacks failed');
};

const prioritizeFallback = (serverName) => {
  const primary =
    serverName.toLowerCase() === 'hd-1' ? FALLBACK_PROVIDERS[0] : FALLBACK_PROVIDERS[1];

  return [primary, ...FALLBACK_PROVIDERS.filter((p) => p !== primary)];
};

const fetchFallbackHTML = async (provider, epID, type) =>
  fetchText(`https://${provider.domain}/stream/s-2/${epID}/${type}`, {
    headers: {
      Referer: `https://${provider.domain}/`,
      'User-Agent': config.headers['User-Agent'],
    },
  });

const fetchFallbackSources = async (provider, id) => {
  const data = await fetchJSON(`https://${provider.domain}/stream/getSources?id=${id}`, {
    headers: {
      'X-Requested-With': 'XMLHttpRequest',
      Referer: `https://${provider.domain}/`,
    },
  });

  return { data };
};

const extractDataId = (html) => html.match(/data-id=["'](\d+)["']/)?.[1];

/* =======================
   KEY MANAGEMENT
======================= */

const getDecryptionKey = async () => {
  const now = Date.now();

  if (cachedKey && now - keyLastFetched < KEY_CACHE_DURATION) {
    return cachedKey;
  }

  try {
    return normalizeKey(process.env.MEGACLOUD_KEY || HARDCODED_MEGACLOUD_KEY);
  } catch {
    // Fall through to remote fetch if the override is invalid.
  }

  if (cachedKey) return cachedKey;
  keyLastFetched = now;
  return getRemoteKey();
};

/* =======================
   UTIL
======================= */

const extractEpisodeId = (id) => id.split('ep=').pop();

const validateSources = (sources) => {
  if (!sources?.[0]?.file) {
    throw new Error('Invalid decrypted sources');
  }
};

const buildResult = ({ id, server, file, rawData, usedFallback }) => ({
  id,
  type: server.type,
  link: {
    file,
    type: 'hls',
  },
  tracks: rawData?.tracks ?? [],
  intro: rawData?.intro ?? null,
  outro: rawData?.outro ?? null,
  server: server.name,
  usedFallback,
});

const backoff = (retry) => new Promise((res) => setTimeout(res, 2000 * (retry + 1)));
