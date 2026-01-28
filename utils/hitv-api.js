const axios = require('axios');

const BASE_URL = 'https://h5-api.aoneroom.com/wefeed-h5api-bff';
const DEFAULT_HEADERS = {
    'Content-Type': 'application/json',
    'User-Agent': 'Mozilla/5.0 (Linux; Android 10; K) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/114.0.0.0 Mobile Safari/537.36',
    'Origin': 'https://www.gohitv.com',
    'Referer': 'https://www.gohitv.com/'
};

const HITV_COUNTRIES = ["Korea", "China", "Thailand", "Japan", "Vietnam", "Others"];
const HITV_CATEGORIES = [
    { name: "Drama", id: 2 },
    { name: "Movie", id: 1 },
    { name: "Variety", id: 3 }
];
const HITV_GENRES = [
    "All", "Romance", "Comedy", "Action", "Thriller", "Horror", "Crime", "Mystery", 
    "Fantasy", "Family", "History", "Youth", "Medical", "Law", "Wuxia", "Xianxia"
];
const HITV_SORTS = ["Hottest", "Newest", "Latest", "Recommend", "General"];

async function browseHiTv(page = 1, pageSize = 20, country = "Korea", category = "Drama", genre = "All", sort = "Hottest") {
    const url = `${BASE_URL}/subject/filter`;
    
    // Resolve category name to ID
    const catObj = HITV_CATEGORIES.find(c => c.name.toLowerCase() === category.toLowerCase()) || HITV_CATEGORIES[0];
    
    const payload = {
        page: parseInt(page),
        perPage: parseInt(pageSize),
        channelId: catObj.id, 
        country: country,
        sort: sort
    };

    // Add genre if specified and not "All"
    if (genre && genre.toLowerCase() !== "all") {
        payload.genre = genre;
    }

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
                    genre: item.genre
                }))
            };
        }
        return { items: [] };
    } catch (error) {
        console.error('[HiTV] Browse error:', error.message);
        return { items: [] };
    }
}

function getHiTvCountries() {
    return HITV_COUNTRIES;
}

function getHiTvCategories() {
    return HITV_CATEGORIES.map(c => c.name);
}

function getHiTvGenres() {
    return HITV_GENRES;
}

function getHiTvSorts() {
    return HITV_SORTS;
}

async function getHiTvDetail(id) {
    // Guessing the endpoint based on standard HiTV/Aoneroom patterns
    // Usually it's POST /subject/detail or GET /subject/{id}
    const url = `${BASE_URL}/subject/detail`;
    const payload = {
        subjectId: id
    };

    try {
        const response = await axios.post(url, payload, { headers: DEFAULT_HEADERS });
        if (response.data && response.data.code === 0 && response.data.data) {
            const d = response.data.data;
            return {
                id: d.subjectId,
                title: d.title,
                description: d.description,
                cover: d.cover ? d.cover.url : null,
                detailPath: d.detailPath, // Added slug
                episodes: (d.seasonList || []).flatMap(s => s.episodeList || []).map(e => ({
                    number: e.episodeName || e.episode,
                    title: e.title || `Episode ${e.episode}`,
                    id: e.id
                }))
            };
        }
        return null;
    } catch (error) {
        // Fallback: If detail fails, return basic info so we can still try to search
        return { id, title: "Unknown", episodes: [] };
    }
}

async function searchHiTv(query, type = 2, page = 1) {
    const url = `${BASE_URL}/subject/search`;
    const payload = {
        keyword: query,
        page: parseInt(page),
        perPage: 24,
        subjectType: type // 1 for Movies, 2 for Series
    };

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
        console.error('[HiTV] Search error:', error.message);
        return { items: [], page, hasNext: false };
    }
}

module.exports = { browseHiTv, searchHiTv, getHiTvDetail, getHiTvCountries, getHiTvCategories, getHiTvGenres, getHiTvSorts };
