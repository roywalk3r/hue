const express = require('express');
const axios = require('axios');
const https = require('https');
const router = express.Router();
const { COMMON_HEADERS } = require('../utils/headers');

const axiosInstance = axios.create({
    timeout: 15000,
    validateStatus: () => true,
    httpsAgent: new https.Agent({ rejectUnauthorized: false })
});

// Helper to build a proxy URL
function getProxyUrl(req, targetUrl, headers) {
    const baseUrl = `${req.protocol}://${req.get('host')}/api/proxy`;
    const params = new URLSearchParams();
    params.set('url', targetUrl);
    if (headers) params.set('headers', JSON.stringify(headers));
    return `${baseUrl}?${params.toString()}`;
}

router.get('/', async (req, res) => {
    try {
        const { url: targetUrl, headers: headersJson } = req.query;

        if (!targetUrl) return res.status(400).send('Missing url parameter');

        let customHeaders = {};
        if (headersJson) {
            try {
                customHeaders = JSON.parse(headersJson);
            } catch (e) {
                console.error('Failed to parse proxy headers:', e.message);
            }
        }

        // Normalize all headers
        const finalHeaders = {};
        // Add Common Headers First
        Object.keys(COMMON_HEADERS).forEach(k => finalHeaders[k.toLowerCase()] = COMMON_HEADERS[k]);
        
        // Add Custom Headers (Referer/Origin are critical for sports)
        Object.keys(customHeaders).forEach(k => finalHeaders[k.toLowerCase()] = customHeaders[k]);

        // Clean up headers that node/axios handles or that cause issues
        delete finalHeaders['host'];
        delete finalHeaders['connection'];
        delete finalHeaders['content-length'];
        delete finalHeaders['accept-encoding']; 
        // Ensure Referer/Origin are preserved if passed in customHeaders
        
        // Fetch as arraybuffer to handle binary segments (.ts)
        const response = await axiosInstance.get(targetUrl, {
            headers: finalHeaders,
            responseType: 'arraybuffer'
        });

        // Determine if it's a playlist
        const contentType = (response.headers['content-type'] || '').toLowerCase();
        const isM3U8 = contentType.includes('mpegurl') || 
                       contentType.includes('apple.mpegurl') || 
                       targetUrl.toLowerCase().split('?')[0].endsWith('.m3u8');

        res.status(response.status);

        // Forward vital headers
        if (response.headers['content-type']) res.setHeader('Content-Type', response.headers['content-type']);
        res.setHeader('Access-Control-Allow-Origin', '*'); // Enable CORS for the player

        if (isM3U8) {
            // Decode buffer to string for rewriting
            let data = Buffer.from(response.data).toString('utf8');
            
            console.log(`[Proxy] Rewriting M3U8: ${targetUrl}`);
            
            const lines = data.split('\n');
            const rewrittenLines = lines.map(line => {
                line = line.trim();
                if (!line) return line;

                // Handle Tags with URIs (Key, Map)
                if (line.startsWith('#')) {
                    if (line.startsWith('#EXT-X-KEY') || line.startsWith('#EXT-X-MAP')) {
                        return line.replace(/URI=["']([^"']+)["']/, (match, uri) => {
                            try {
                                const absoluteUri = new URL(uri, targetUrl).href;
                                return `URI="${getProxyUrl(req, absoluteUri, customHeaders)}"`;
                            } catch(e) { return match; }
                        });
                    }
                    return line;
                }

                // Handle Segment URLs
                try {
                    const absoluteUrl = new URL(line, targetUrl).href;
                    return getProxyUrl(req, absoluteUrl, customHeaders);
                } catch(e) { return line; }
            });
            
            const rewrittenData = rewrittenLines.join('\n');
            res.setHeader('Content-Type', 'application/vnd.apple.mpegurl');
            res.send(rewrittenData);
        } else {
            // Forward binary data (segments, keys, images)
            res.send(Buffer.from(response.data));
        }

    } catch (error) {
        console.error(`[Proxy] Error fetching ${req.query.url}:`, error.message);
        res.status(500).send(`Proxy Error: ${error.message}`);
    }
});

module.exports = router;