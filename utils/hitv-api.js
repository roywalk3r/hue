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
    // Explicitly use subjectId as required by the API error message
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
                    // Generate episodes based on maxEp if specific list is missing
                    const eps = [];
                    const max = s.maxEp || 0;
                    for (let i = 1; i <= max; i++) {
                        eps.push({
                            number: i,
                            title: `Episode ${i}`,
                            id: `${d.subjectId}-${i}` // Dummy ID as HiTV doesn't expose unique EP IDs in this view usually
                        });
                    }
                    return eps;
                })
            };
        }
        return null;
    } catch (error) {
        console.error(`[HiTV] Detail error for ${id}:`, error.message);
        if (error.response) {
             console.error(`[HiTV] Detail Response: ${error.response.status}`, JSON.stringify(error.response.data));
        }
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
