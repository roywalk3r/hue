const axios = require('axios');
const { execSync } = require('child_process');
const fs = require('fs').promises;
const path = require('path');
const tmdb = require('./tmdb');

const BASE_URL = 'https://h5-api.aoneroom.com/wefeed-h5api-bff';
const DEFAULT_HEADERS = {
    'Content-Type': 'application/json',
    'User-Agent': 'Mozilla/5.0 (Linux; Android 10; K) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/114.0.0.0 Mobile Safari/537.36',
    'Origin': 'https://www.gohitv.com',
    'Referer': 'https://www.gohitv.com/'
};

// In-memory cache for Library
let libraryCache = [];
let lastLibraryFetch = 0;
let isBuilding = false;
const CACHE_TTL = 12 * 3600 * 1000; // 12 hours
const CACHE_FILE = path.join(__dirname, '../hitv_library_cache.json');

// Hero cache
let heroCache = null;
let lastHeroFetch = 0;
const HERO_CACHE_TTL = 3600 * 1000; // 1 hour

// Load cache from disk on startup
(async () => {
    try {
        const data = await fs.readFile(CACHE_FILE, 'utf8');
        libraryCache = JSON.parse(data);
        lastLibraryFetch = Date.now(); 
        console.log(`[HiTV] Loaded ${libraryCache.length} items from disk cache.`);
    } catch (e) {
        console.log('[HiTV] No disk cache found, library empty.');
    }
})();

const sleep = (ms) => new Promise(resolve => setTimeout(resolve, ms));

function cleanTitle(title) {
    if (!title) return "";
    return title
        .replace(/\s*\(\d{4}\)/g, '')
        .replace(/\s*Season\s*\d+/gi, '')
        .replace(/\s*S\d+/gi, '')
        .replace(/\s*-\s*Ep.*/gi, '')
        .trim();
}

async function fetchWithRetry(fn, retries = 3, delay = 1000) {
    try {
        return await fn();
    } catch (e) {
        if (retries > 0) {
            await sleep(delay);
            return fetchWithRetry(fn, retries - 1, delay * 2);
        }
        return { items: [] }; 
    }
}

async function runBatch(tasks, limit, delayMs = 1500) {
    const results = [];
    for (let i = 0; i < tasks.length; i += limit) {
        const batch = tasks.slice(i, i + limit);
        const batchResults = await Promise.all(batch.map(fn => fn()));
        results.push(...batchResults);
        if (i + limit < tasks.length) {
            await sleep(delayMs);
        }
    }
    return results;
}

async function buildLibrary() {
    if (isBuilding) return;
    isBuilding = true;
    console.log('[HiTV] Starting background library build...');

    const categories = ["Drama", "Movie", "Variety"];
    const countries = ["Korea", "China", "Thailand", "Japan", "Vietnam", "Others"];
    const sorts = ["Hottest", "Newest"];
    const maxPage = 10; 
    
    const taskFactories = [];

    for (const cat of categories) {
        for (const country of countries) {
            for (const sort of sorts) {
                for (let p = 1; p <= maxPage; p++) {
                    taskFactories.push(async () => {
                        try {
                            const res = await browseHiTv(p, 24, country, cat, "All", sort);
                            if (res && res.items) {
                                res.items.forEach(item => item.category = cat);
                                return res.items;
                            }
                            return [];
                        } catch (e) {
                            return [];
                        }
                    });
                }
            }
        }
    }

    try {
        const allItemsNested = await runBatch(taskFactories, 2, 2000);
        const uniqueItems = new Map();
        libraryCache.forEach(item => uniqueItems.set(item.id, item));
        allItemsNested.forEach(items => {
            if (items) {
                items.forEach(item => {
                    uniqueItems.set(item.id, item);
                });
            }
        });
        const library = Array.from(uniqueItems.values());
        library.sort((a, b) => (b.releaseDate || "").localeCompare(a.releaseDate || ""));
        console.log(`[HiTV] Library build complete. Total Unique: ${library.length}`);
        if (library.length > 0) {
            libraryCache = library;
            lastLibraryFetch = Date.now();
            await fs.writeFile(CACHE_FILE, JSON.stringify(library));
        }
    } catch (e) {
        console.error(`[HiTV] Library build failed: ${e.message}`);
    } finally {
        isBuilding = false;
    }
}

async function getLibrary() {
    if (libraryCache.length === 0 || (Date.now() - lastLibraryFetch > CACHE_TTL)) {
        buildLibrary();
    }
    return libraryCache;
}

async function getHero() {
    if (heroCache && (Date.now() - lastHeroFetch < HERO_CACHE_TTL)) {
        return heroCache;
    }

    console.log('[HiTV] Building Smart Hero (2024-2026)...');
    const tasks = [
        () => browseHiTv(1, 24, "Korea", "Drama", "All", "Newest", "2026"),
        () => browseHiTv(1, 24, "Korea", "Drama", "All", "Newest", "2025"),
        () => browseHiTv(1, 24, "Korea", "Drama", "All", "Recommend")
    ];
    
    const results = await Promise.all(tasks.map(t => t()));
    let items = [];
    results.forEach(r => { if (r.items) items.push(...r.items); });

    const uniqueIds = new Set();
    items = items.filter(i => {
        if (uniqueIds.has(i.id)) return false;
        uniqueIds.add(i.id);
        return true;
    });

    console.log(`[HiTV] Processing ${items.length} candidates for Hero...`);

    const enriched = [];
    for (const item of items) {
        if (enriched.length >= 12) break;

        try {
            const searchTitle = cleanTitle(item.title);
            const year = item.year || (item.releaseDate ? item.releaseDate.split('-')[0] : null);
            const isVariety = item.genre && (item.genre.toLowerCase().includes('reality') || item.genre.toLowerCase().includes('variety'));
            
            const tmdbResults = await tmdb.search(searchTitle, 'tv');
            
            if (tmdbResults && tmdbResults.length > 0) {
                const match = tmdbResults[0];
                const tmdbYear = (match.first_air_date || "").split('-')[0];
                
                let isMatch = false;
                if (isVariety) {
                    isMatch = true; 
                } else if (year && tmdbYear && Math.abs(parseInt(year) - parseInt(tmdbYear)) <= 1) {
                    isMatch = true;
                }

                if (isMatch) {
                    if (match.backdrop_path) {
                        item.cover = `https://image.tmdb.org/t/p/original${match.backdrop_path}`;
                        item.isBackdrop = true;
                    } else if (match.poster_path) {
                        item.cover = `https://image.tmdb.org/t/p/w780${match.poster_path}`;
                        item.isBackdrop = false;
                    }
                    item.description = match.overview || item.description;
                    item.rating = match.vote_average ? match.vote_average.toString() : item.rating;
                    item.tmdbId = match.id;
                    enriched.push(item);
                    console.log(`[HiTV-Enrich] Hero Success: "${item.title}" (Backdrop: ${item.isBackdrop})`);
                }
            }
        } catch (e) {}
    }

    enriched.sort((a, b) => (b.isBackdrop ? 1 : 0) - (a.isBackdrop ? 1 : 0));
    const finalHero = enriched.slice(0, 10);
    console.log(`[HiTV] Smart Hero complete with ${finalHero.length} items.`);
    heroCache = finalHero;
    lastHeroFetch = Date.now();
    return finalHero;
}

const HITV_COUNTRIES = [
    "All", "United States", "United Kingdom", "Korea", "Japan", "Bangladesh", "China", 
    "Egypt", "France", "Germany", "India", "Indonesia", "Iraq", "Italy", "Ivory Coast", 
    "Kenya", "Lebanon", "Mexico", "Morocco", "Nigeria", "Pakistan", "Philippines", 
    "Russia", "Saudi Arabia", "South Africa", "Spain", "Syria", "Thailand", "Malaysia", 
    "Turkey", "Vietnam", "Others"
];

const HITV_CATEGORIES = [
    { name: "Drama", id: 2 },
    { name: "Movie", id: 1 },
    { name: "Variety", id: 3 },
    { name: "Anime", id: 4 }
];

const HITV_GENRES = [
    "All", "Action", "Adventure", "Animation", "Biography", "Comedy", "Crime", 
    "Documentary", "Drama", "Family", "Fantasy", "Film-Noir", "Game-Show", "History", 
    "Horror", "Music", "Musical", "Mystery", "News", "Reality-TV", "Romance", 
    "Sci-Fi", "Short", "Sport", "Talk-Show", "Thriller", "War", "Western", "Wuxia", "Xianxia"
];

const HITV_SORTS = ["ForYou", "Hottest", "Newest", "Latest", "Recommend", "General"];

async function browseHiTv(page = 1, pageSize = 28, country = "Korea", category = "Drama", genre = "All", sort = "Hottest", year = null) {

    const url = `${BASE_URL}/subject/filter`;

    

    const catObj = HITV_CATEGORIES.find(c => c.name.toLowerCase() === category.toLowerCase()) || HITV_CATEGORIES[0];

    

    const payload = {

        page: parseInt(page),

        perPage: parseInt(pageSize),

        channelId: catObj.id, 

        country: (country === "All" || !country) ? undefined : country, 

        sort: sort

    };
    if (genre && genre.toLowerCase() !== "all") payload.genre = genre;
    if (year) payload.year = year.toString();

    try {
        const response = await axios.post(url, payload, { headers: DEFAULT_HEADERS });
        if (response.data && response.data.code === 0 && response.data.data) {
            return {
                page: response.data.data.pager.page,
                hasNext: response.data.data.pager.hasMore,
                items: response.data.data.items.map(item => ({
                    id: item.subjectId,
                    title: item.title,
                    cover: item.cover ? item.cover.url : null,
                    rating: item.imdbRatingValue,
                    releaseDate: item.releaseDate,
                    genre: item.genre,
                    year: item.releaseDate ? item.releaseDate.split('-')[0] : null
                }))
            };
        }
        return { items: [] };
    } catch (error) {
        console.error('[HiTV] Browse error:', error.message);
        return { items: [] };
    }
}

async function browseDrama({ page = 1, country = "Korea", genre = "All", sort = "Hottest", year = null }) {
    console.log(`[HiTV] Browsing Drama: Page ${page}, ${country}, ${genre}, ${sort}, ${year || 'All Years'}`);
    const res = await browseHiTv(page, 28, country, "Drama", genre, sort, year);
    if (!res.items || res.items.length === 0) return res;

    const enrichedItems = await Promise.all(res.items.map(async (item) => {
        try {
            if (item.title) {
                const results = await tmdb.search(cleanTitle(item.title), 'tv', item.year);
                if (results && results.length > 0) {
                    const match = results[0];
                    if (match.backdrop_path) {
                        item.backdrop = `https://image.tmdb.org/t/p/original${match.backdrop_path}`;
                        item.cover = `https://image.tmdb.org/t/p/w780${match.poster_path}`; 
                    }
                    item.tmdbId = match.id;
                    item.overview = match.overview;
                    item.rating = match.vote_average ? match.vote_average.toString() : item.rating;
                }
            }
        } catch (e) {}
        return item;
    }));
    return { ...res, items: enrichedItems };
}

function getHiTvCountries() { return HITV_COUNTRIES; }
function getHiTvCategories() { return HITV_CATEGORIES.map(c => c.name); }
function getHiTvGenres() { return HITV_GENRES; }
function getHiTvSorts() { return HITV_SORTS; }

async function getHiTvDetail(id) {
    if (!/^\d+$/.test(id)) {
        console.log(`[HiTV] Resolving Web ID: ${id}`);
        let title = null;
        if (libraryCache) {
            const item = libraryCache.find(i => i.id === id);
            if (item) title = item.title;
        }
        if (!title) {
            const html = fetchWithCurl(`https://home.hitv.vip/series/${id}/1`);
            if (html) {
                const titleMatch = html.match(/<title>(.*?)<\/title>/);
                if (titleMatch) {
                    let rawTitle = titleMatch[1];
                    rawTitle = rawTitle.split('(Ep')[0].split('-Ep')[0].split(' - ')[0];
                    title = rawTitle.trim();
                }
            }
        }
        if (title) {
            const searchRes = await searchHiTv(title);
            if (searchRes.items && searchRes.items.length > 0) {
                const numericId = searchRes.items[0].id;
                return getHiTvDetail(numericId);
            }
        }
        return { id, title: title || "Unknown", episodes: [] };
    }

    const url = `${BASE_URL}/detail?subjectId=${id}&lang=en`;
    try {
        const response = await axios.get(url, { headers: DEFAULT_HEADERS });
        if (response.data && response.data.code === 0 && response.data.data && response.data.data.subject) {
            const d = response.data.data.subject;
            return {
                id: d.subjectId,
                title: d.title,
                description: d.description,
                cover: d.cover ? d.cover.url : null,
                detailPath: d.detailPath,
                episodes: (response.data.data.resource?.seasons || []).flatMap(s => {
                    const eps = [];
                    const max = s.maxEp || 0;
                    for (let i = 1; i <= max; i++) {
                        eps.push({
                            number: i,
                            title: `Episode ${i}`,
                            id: `${d.subjectId}-${i}`
                        });
                    }
                    return eps;
                })
            };
        }
        return null;
    } catch (error) {
        return { id, title: "Unknown", episodes: [] };
    }
}

async function searchHiTv(query, type = 2, page = 1) {
    const url = `${BASE_URL}/subject/search`;
    const payload = { keyword: query, page: parseInt(page), perPage: 24, subjectType: type };
    try {
        const response = await axios.post(url, payload, { headers: DEFAULT_HEADERS });
        if (response.data && response.data.code === 0 && response.data.data) {
             const list = response.data.data.items || [];
             return {
                page: response.data.data.pager ? response.data.data.pager.page : page,
                hasNext: response.data.data.pager ? response.data.data.pager.hasMore : false,
                items: list.map(item => ({
                    id: item.subjectId,
                    title: item.title,
                    cover: item.cover ? item.cover.url : null,
                    rating: item.imdbRatingValue,
                    releaseDate: item.releaseDate,
                    genre: item.genre,
                    type: item.subjectType === 1 ? "movie" : "tv"
                }))
            };
        }
        return { items: [], page, hasNext: false };
    } catch (error) {
        return { items: [], page, hasNext: false };
    }
}

function fetchWithCurl(url) {
    try {
        const cmd = `curl -s -L -A "Mozilla/5.0 (Windows NT 10.0; Win64; x64)" "${url}"`;
        const output = execSync(cmd, { encoding: 'utf8', maxBuffer: 10 * 1024 * 1024 }); 
        return output;
    } catch (e) { return null; }
}

module.exports = { browseHiTv, searchHiTv, getHiTvDetail, getHiTvCountries, getHiTvCategories, getHiTvGenres, getHiTvSorts, getLibrary, getHero, browseDrama };
