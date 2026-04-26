const fetch = require('node-fetch');

/**
 * HiAnime Provider (Advanced 2026 Edition)
 * Uses direct myani.cfd API with correct POST methods and category mapping.
 */

const HIANIME_DOMAINS = [
  'https://hianimes.se',
  'https://hianime.ph',
  'https://hianime.lc'
];

const API_BASE = 'https://myani.cfd/api';
const STREAM_BASE = 'https://megaplay.buzz/stream/getSources';
const USER_AGENT = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/123.0.0.0 Safari/537.36';

async function fetchJSON(url, options = {}) {
  try {
    const res = await fetch(url, {
        ...options,
        headers: {
            'User-Agent': USER_AGENT,
            'X-Requested-With': 'XMLHttpRequest',
            'Content-Type': 'application/json',
            ...options.headers
        },
        timeout: 10000
    });
    if (!res.ok) return null;
    return await res.json();
  } catch (e) {
    return null;
  }
}

async function tryFetchWithFallback(path, options = {}) {
    for (const domain of HIANIME_DOMAINS) {
        try {
            const url = domain + path;
            const res = await fetchJSON(url, options);
            if (res) return res;
        } catch (err) {}
    }
    return null;
}

/**
 * Discovery Functions
 */

async function search(keyword, page = 1) {
    // Verified: Search is a POST request
    return await fetchJSON(`${API_BASE}/search`, {
        method: 'POST',
        body: JSON.stringify({ title: keyword })
    });
}

async function getByCategory(type = 'tv', page = 1, limit = 20) {
    return await fetchJSON(`${API_BASE}/category?type=${type}&page=${page}&limit=${limit}`);
}

async function getLatest() {
    return await fetchJSON(`${API_BASE}/latest/anime`);
}

async function getEpisodes(animeId) {
    // animeId is the hash id like '68e5280064d200c87068dcd1'
    return await fetchJSON(`${API_BASE}/episodes/${animeId}`);
}

async function getHome() {
    return await fetchJSON(`${API_BASE}/home?slugNth=12&includeSlugs=false`);
}

async function getAnimeInfo(slug) {
    // Fetches full details including the _id needed for episodes
    return await fetchJSON(`${API_BASE}/anime/${slug}`);
}
async function getServers(id) {
  console.log(`[HiAnime] Fetching servers for ID/Slug: ${id}`);

  // If it's a hex ID (24 chars), we use it directly, otherwise we treat it as a slug
  const isHexId = /^[0-9a-fA-F]{24}$/.test(id);
  const endpoint = isHexId ? `${API_BASE}/episode/id/${id}` : `${API_BASE}/episode/${id}`;

  const data = await fetchJSON(endpoint);

  if (!data || !data.episode || !data.episode.link) {
      // Fallback: try the other endpoint format just in case
      const fallbackEndpoint = isHexId ? `${API_BASE}/episode/${id}` : `${API_BASE}/episode/id/${id}`;
      const fallbackData = await fetchJSON(fallbackEndpoint);
      if (!fallbackData || !fallbackData.episode) return { sub: [], dub: [] };
      return parseServersFromApi(fallbackData);
  }

  return parseServersFromApi(data);
}

function parseServersFromApi(data) {
  const mapLinks = (links, type) => {
      return (links || []).map(link => {
          // Case 1: Internal JSON Server (megaplay/animeplay)
          const match = link.match(/\/s-2\/(\d+)/);
          if (match) {
            return {
                id: match[1],
                name: link.includes('megaplay') ? 'HD-1' : 'HD-2',
                type,
                format: 'json'
            };
          }

          // Case 2: Direct Embed (Vidmoly, Vidwish, etc)
          try {
              const url = new URL(link);
              const name = url.hostname.replace('www.', '').split('.')[0].toUpperCase();
              return {
                  id: link, // The link itself is the ID
                  name: name,
                  type,
                  format: 'embed'
              };
          } catch (e) {
              return null;
          }
      }).filter(s => s && s.id);
  };

  return {
    sub: mapLinks(data.episode.link.sub, 'sub'),
    dub: mapLinks(data.episode.link.dub, 'dub')
  };
}

async function getAnimeStream({ id, server, type, episode = 1 }) {
  let targetId = id;
  let preResolvedServers = null;

  // 1. Resolve Anime ID to Episode
  const isHexId = /^[0-9a-fA-F]{24}$/.test(id);
  if (isHexId) {
      const epData = await getEpisodes(id);
      if (epData && epData.episodes?.length > 0) {
          const ep = epData.episodes.find(e => e.episodeNumber == episode) || epData.episodes[0];
          targetId = ep.slugs?.[0] || ep._id;

          // Optimization: If the episode list already has links, use them!
          if (ep.link && (ep.link.sub?.length > 0 || ep.link.dub?.length > 0)) {
              preResolvedServers = parseServersFromApi({ episode: ep });
          }
      }
  }

  // 2. Get Servers
  const servers = preResolvedServers || await getServers(targetId);
  const list = servers[type || 'sub'];

  if (!list?.length) return { error: 'SERVERS_NOT_FOUND' };  // Select target (Prefer HD-1 or provided server)
  const target = server ? list.find(s => s.name === server) : (list.find(s => s.format === 'json') || list[0]);
  if (!target) return { error: 'SERVER_NOT_FOUND' };

  // 3. Handle Embed Format
  if (target.format === 'embed') {
      return {
          stream: {
              link: { file: target.id, type: 'iframe' },
              server: target.name
          }
      };
  }

  // 4. Handle JSON Format (Internal Extraction)
  for (const streamDom of STREAM_DOMAINS) {
      console.log(`[HiAnime] Extracting from ${streamDom} ID: ${target.id}`);
      const sources = await fetchJSON(`${streamDom}/stream/getSources?id=${target.id}`, {
          headers: { 'Referer': streamDom + '/' }
      });

      if (sources?.sources?.file) {
          return {
              stream: {
                  link: { file: sources.sources.file, type: 'hls' },
                  tracks: sources.tracks || [],
                  intro: sources.intro,
                  outro: sources.outro,
                  server: target.name
              }
          };
      }
  }

  return { error: 'EXTRACTION_FAILED' };
}module.exports = {
  getServers,
  getAnimeStream,
  getEpisodes,
  getAnimeInfo,
  search,
  getLatest,
  getHome,
  getByCategory
};
