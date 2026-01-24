const axios = require('axios');
const http = require('http');
const https = require('https');

const USER_AGENT = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";
const axiosInstance = axios.create({
    headers: { "User-Agent": USER_AGENT, "Accept": "application/json" },
    httpAgent: new http.Agent({ family: 4 }),
    httpsAgent: new https.Agent({ family: 4 }),
    timeout: 10000
});

(async () => {
    try {
        const episodeId = "146141"; // From the logs for "Amidst a Snowstorm of Love" Ep 1
        const url = `https://kisskh.ovh/api/DramaList/Episode/${episodeId}.png?v=1.1`;
        console.log(`Fetching: ${url}`);
        const res = await axiosInstance.get(url);
        console.log("Result:", JSON.stringify(res.data, null, 2));
    } catch (e) {
        console.error("Error:", e.response ? e.response.status : e.message);
    }
})();
