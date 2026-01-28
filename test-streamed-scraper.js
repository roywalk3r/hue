const axios = require('axios');
const cheerio = require('cheerio');

async function testStreamedScraper() {
    const watchUrl = 'https://streamed.pk/watch/neptunas-vs-hapoel-jerusalem/echo/1';
    console.log(`Testing: ${watchUrl}`);

    try {
        // 1. Get Watch Page
        const response = await axios.get(watchUrl, {
            headers: {
                'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36'
            }
        });
        const html = response.data;
        console.log('Watch Page Fetched.');

        // 2. Extract Next.js Data
        const nextDataMatch = html.match(/<script id="__NEXT_DATA__" type="application\/json">(.*?)<\/script>/);
        if (nextDataMatch) {
            const json = JSON.parse(nextDataMatch[1]);
            console.log('Found __NEXT_DATA__');
            // console.log(JSON.stringify(json, null, 2)); // Debug: Dump all data

            // Navigate json to find stream/match info
            // Structure guess: props.pageProps.match.id or similar
            const props = json.props?.pageProps;
            if (props) {
                const matchId = props.match?.id || props.event?.id || props.stream?.id;
                console.log(`Extracted ID from props: ${matchId}`);
                
                if (matchId) {
                     const apiUrl = `https://streamed.pk/api/stream/echo/${matchId}`;
                     console.log(`Constructed API URL: ${apiUrl}`);
                     
                     // 3. Call API
                     const apiResponse = await axios.get(apiUrl, {
                        headers: {
                            'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36',
                            'Referer': watchUrl
                        }
                    });
                    console.log('API Response:', JSON.stringify(apiResponse.data, null, 2));
                }
            }
        } else {
            console.log('No __NEXT_DATA__ found.');
        }

    } catch (e) {
        console.error('Error:', e.message);
    }
}

testStreamedScraper();
