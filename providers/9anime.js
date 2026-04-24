const axios = require('axios');
const cheerio = require('cheerio');
const { getTmdbInfo } = require('../utils/tmdb');

const BASE_URL = 'https://9animetv.to';
const RAPID_CLOUD = 'https://rapid-cloud.co';

const DEFAULT_HEADERS = {
    'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36',
    'X-Requested-With': 'XMLHttpRequest'
};

async function getStreams(tmdbId, type, season = 1, episode = 1) {
    if (type !== 'series' && type !== 'tv' && type !== 'movie') return [];

    try {
        console.log(`[9Anime] Searching for TMDB ID: ${tmdbId}, Type: ${type}`);
        
        const info = await getTmdbInfo(tmdbId, type === 'series' ? 'tv' : type);
        if (!info || !info.title) return [];
        
        const searchTitle = info.title;
        console.log(`[9Anime] Searching for: "${searchTitle}"`);

        const searchRes = await axios.get(`${BASE_URL}/search?keyword=${encodeURIComponent(searchTitle)}`, {
            headers: { ...DEFAULT_HEADERS, 'Referer': BASE_URL }
        });
        const $ = cheerio.load(searchRes.data);
        const results = [];
        
        $('.film-detail').each((i, el) => {
            const nameEl = $(el).find('.film-name a');
            const title = nameEl.text().trim();
            const href = nameEl.attr('href');
            const id = href ? href.split('-').pop() : null;
            
            const infoText = $(el).find('.fd-infor').text();
            const yearMatch = infoText.match(/(19|20)\d{2}/);
            const year = yearMatch ? yearMatch[0] : null;

            if (title && id) {
                results.push({ title, id, year, href });
            }
        });

        if (results.length === 0) {
            console.log(`[9Anime] No search results for "${searchTitle}"`);
            return [];
        }

        // IMPROVED MATCHING: Fuzzy/Scored
        const bestMatch = results.sort((a, b) => {
            const clean = (t) => t.toLowerCase().replace(/[^a-z0-9]/g, '');
            const targetClean = clean(searchTitle);
            const aClean = clean(a.title);
            const bClean = clean(b.title);

            // 1. Exact matches first
            if (aClean === targetClean && bClean !== targetClean) return -1;
            if (bClean === targetClean && aClean !== targetClean) return 1;

            // 2. Season specific matches if TV
            if (type !== 'movie') {
                const aHasSeason = aClean.includes(`season${season}`);
                const bHasSeason = bClean.includes(`season${season}`);
                if (aHasSeason && !bHasSeason) return -1;
                if (bHasSeason && !aHasSeason) return 1;
            }

            // 3. Length score (shorter is usually better)
            return a.title.length - b.title.length;
        })[0];

        console.log(`[9Anime] Selected: "${bestMatch.title}" (ID: ${bestMatch.id})`);

        let targetEpId = null;
        const epListRes = await axios.get(`${BASE_URL}/ajax/episode/list/${bestMatch.id}`, {
            headers: { ...DEFAULT_HEADERS, 'Referer': `${BASE_URL}${bestMatch.href}` }
        });
        
        if (epListRes.data.status && epListRes.data.html) {
            const $ep = cheerio.load(epListRes.data.html);
            if (type === 'movie') {
                targetEpId = $ep('.ep-item').first().data('id');
            } else {
                const epEl = $ep(`.ep-item[data-number="${episode}"]`);
                targetEpId = epEl.data('id') || $ep('.ep-item').first().data('id');
            }
        }

        if (!targetEpId) return [];

        const serversRes = await axios.get(`${BASE_URL}/ajax/episode/servers?episodeId=${targetEpId}`, {
            headers: { ...DEFAULT_HEADERS, 'Referer': `${BASE_URL}${bestMatch.href}` }
        });
        
        if (!serversRes.data.status || !serversRes.data.html) return [];
        const $sv = cheerio.load(serversRes.data.html);
        
        const serverItems = $sv('.server-item');
        let dataId = null;
        let serverName = "Saber";

        serverItems.each((i, el) => {
            const sid = $(el).data('server-id');
            if (sid == 4 || sid == 1) { // 4 = Vidstreaming, 1 = Vidcloud
                dataId = $(el).data('id');
                serverName = $(el).text().trim();
                return false;
            }
        });

        if (!dataId && serverItems.length > 0) {
            dataId = serverItems.first().data('id');
            serverName = serverItems.first().text().trim();
        }

        if (!dataId) return [];

        const sourcesRes = await axios.get(`${BASE_URL}/ajax/episode/sources?id=${dataId}`, {
            headers: { ...DEFAULT_HEADERS, 'Referer': `${BASE_URL}${bestMatch.href}` }
        });
        
        const embedUrl = sourcesRes.data.link;
        if (!embedUrl) return [];

        if (embedUrl.includes('rapid-cloud.co')) {
            const embedId = embedUrl.split('/').pop().split('?')[0];
            const rapidRes = await axios.get(`${RAPID_CLOUD}/embed-2/v2/e-1/getSources?id=${embedId}`, {
                headers: { ...DEFAULT_HEADERS, 'Referer': embedUrl }
            });
            
            if (rapidRes.data.sources && rapidRes.data.sources.length > 0) {
                const streams = [];
                const subtitles = (rapidRes.data.tracks || [])
                    .filter(t => t.kind === 'captions')
                    .map(t => ({
                        url: t.file,
                        lang: t.label.toLowerCase(),
                        label: t.label
                    }));

                rapidRes.data.sources.forEach(s => {
                    streams.push({
                        title: `${serverName} - ${s.type === 'hls' ? 'Auto' : 'MP4'}`,
                        url: s.file,
                        quality: 'Auto',
                        provider: 'Saber',
                        format: s.type === 'hls' ? 'hls' : 'mp4',
                        subtitles: subtitles.length > 0 ? subtitles : undefined,
                        headers: {
                            'Referer': embedUrl, // USE FULL EMBED URL AS REFERER
                            'User-Agent': DEFAULT_HEADERS['User-Agent'],
                            'Origin': RAPID_CLOUD
                        }
                    });
                });
                return streams;
            }
        }

        return [{
            title: `${serverName} (External)`,
            url: embedUrl,
            quality: 'Unknown',
            provider: 'Saber',
            headers: { 'Referer': BASE_URL }
        }];

    } catch (e) {
        console.error('[9Anime] Provider Error:', e.message);
        return [];
    }
}

module.exports = { getStreams };
