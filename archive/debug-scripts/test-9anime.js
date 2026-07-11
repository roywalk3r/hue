/**
 * Hue Stream API
 * Professional Streaming Aggregator
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */

const axios = require('axios');

async function test9Anime() {
    const epId = '169702';
    const baseUrl = 'https://9animetv.to';
    const headers = {
        'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36',
        'Referer': baseUrl,
        'X-Requested-With': 'XMLHttpRequest'
    };

    try {
        console.log(`Testing Servers AJAX for epId: ${epId}...`);
        const serversRes = await axios.get(`${baseUrl}/ajax/episode/servers?episodeId=${epId}`, { headers });
        console.log('Servers Response Status:', serversRes.data.status);
        
        if (serversRes.data.html) {
            const dataIdMatch = serversRes.data.html.match(/data-id="(\d+)"/);
            if (dataIdMatch) {
                const dataId = dataIdMatch[1];
                console.log(`Found Data ID: ${dataId}. Testing Sources AJAX...`);
                
                const sourcesRes = await axios.get(`${baseUrl}/ajax/episode/sources?id=${dataId}`, { headers });
                console.log('Sources Data:', JSON.stringify(sourcesRes.data, null, 2));
                
                if (sourcesRes.data.link) {
                    const embedUrl = sourcesRes.data.link;
                    console.log('Embed URL:', embedUrl);
                    
                    if (embedUrl.includes('rapid-cloud.co')) {
                        const embedId = embedUrl.split('/').pop().split('?')[0];
                        console.log(`Testing RapidCloud getSources for embedId: ${embedId}...`);
                        
                        const rapidHeaders = {
                            ...headers,
                            'Referer': embedUrl
                        };
                        
                        const rapidRes = await axios.get(`https://rapid-cloud.co/embed-2/v2/e-1/getSources?id=${embedId}`, { headers: rapidHeaders });
                        console.log('RapidCloud Results:', JSON.stringify(rapidRes.data, null, 2));
                    }
                }
            }
        }
    } catch (e) {
        console.error('Test Failed:', e.response?.status || e.message);
        if (e.response?.data) console.log('Error Data:', e.response.data);
    }
}

test9Anime();
