/**
 * HUE ENGINE ❄
 * System Designation: The Architecture of Aggregation
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */

const axios = require('axios');
const { decrypt } = require('./videasy/decrypt');
const { getTmdbInfo, getExternalIds } = require('../utils/tmdb');

const API_BASE = 'https://api.videasy.net';
const DEFAULT_PROVIDER = 'cdn';

async function getStreams(tmdbId, mediaType, season, episode) {
  console.log(`[Videasy] Fetching TMDB ${tmdbId} (${mediaType}) S${season || '-'}E${episode || '-'}`);

  try {
    const type = (mediaType === 'tv' || mediaType === 'series') ? 'tv' : 'movie';
    const [info, extIds] = await Promise.all([
      getTmdbInfo(tmdbId, mediaType),
      getExternalIds(type, tmdbId)
    ]);

    if (!info || !info.title) {
      console.warn(`[Videasy] No title found for TMDB ${tmdbId}`);
      return [];
    }

    const title = info.title;
    const year = String(info.year || '');
    const imdbId = extIds?.imdb_id || '';

    const params = new URLSearchParams();
    params.set('title', title);
    params.set('mediaType', type);
    params.set('year', year);
    params.set('tmdbId', String(tmdbId));
    if (imdbId) params.set('imdbId', imdbId);
    if (type === 'tv') {
      params.set('seasonId', String(season || 1));
      params.set('episodeId', String(episode || 1));
    }

    const url = `${API_BASE}/${DEFAULT_PROVIDER}/sources-with-title?${params.toString()}`;
    console.log(`[Videasy] API: ${url}`);

    const response = await axios.get(url, {
      headers: {
        'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36',
        'Accept': '*/*',
        'Referer': 'https://cineby.sc/',
      },
      timeout: 20000,
      responseType: 'text',
    });

    const hexResponse = response.data;
    if (!hexResponse || typeof hexResponse !== 'string' || hexResponse.length < 100) {
      console.warn('[Videasy] Empty or invalid response');
      return [];
    }

    const decrypted = await decrypt(hexResponse.trim(), tmdbId);
    if (!decrypted || !decrypted.sources || !Array.isArray(decrypted.sources)) {
      console.warn('[Videasy] Decryption produced no sources');
      return [];
    }

    const streams = [];
    for (const source of decrypted.sources) {
      if (!source.url) continue;
      streams.push({
        url: source.url,
        title: `Vortex - ${source.quality || 'Auto'}`,
        quality: source.quality || 'Auto',
        format: 'hls',
        provider: 'Vortex',
        headers: {
          'Referer': 'https://cineby.sc/',
          'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36',
        },
      });
    }

    // Attach subtitles to the first stream if available
    if (streams.length > 0 && decrypted.subtitles && Array.isArray(decrypted.subtitles)) {
      streams[0].subtitles = decrypted.subtitles.map(s => ({
        url: s.url,
        lang: s.lang || 'en',
        label: s.language || s.lang || 'English',
      }));
    }

    console.log(`[Videasy] Found ${streams.length} streams`);
    return streams;

  } catch (e) {
    console.error(`[Videasy] Error: ${e.message}`);
    return [];
  }
}

module.exports = { getStreams };
