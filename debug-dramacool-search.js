const axios = require("axios");
const cheerio = require("cheerio");

const BASE_URL = "https://dramacool.sh";
const USER_AGENT = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

async function testSearch(title, episode) {
    const searchUrl = `${BASE_URL}/search?keyword=${encodeURIComponent(title)}`;
    console.log(`Searching: ${searchUrl}`);
    try {
        const { data } = await axios.get(searchUrl, { headers: { "User-Agent": USER_AGENT } });
        const $ = cheerio.load(data);
        const results = [];
        $(".list-episode-item li a").each((i, el) => {
            results.push({
                title: $(el).find(".title").text().trim(),
                href: $(el).attr("href")
            });
        });

        console.log(`Found ${results.length} results:`);
        results.forEach(r => console.log(` - ${r.title} (${r.href})`));

        const target = title.toLowerCase().replace(/[^a-z0-9]/g, "");
        const bestMatch = results.find(r => {
            const rTitle = r.title.toLowerCase().replace(/[^a-z0-9]/g, "");
            return rTitle.includes(target) && r.title.includes(`Episode ${episode}`);
        }) || results[0];

        console.log(`Best match: ${bestMatch ? bestMatch.title : 'NONE'}`);
    } catch (e) {
        console.error(`Error: ${e.message}`);
    }
}

testSearch("Squid Game", 1);
