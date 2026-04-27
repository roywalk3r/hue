#!/usr/bin/env node

/**
 * Hue Stream API
 * Professional Streaming Aggregator
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */

/**
 * Standalone Sports Proxy Server
 * Run this alongside your main server on a different port
 */

const http = require('http');
const https = require('https');
const url = require('url');

class SportsProxyServer {
    constructor(port = 6001) {
        this.port = port;
        this.targetBase = 'https://gg.poocloud.in';
        this.headers = {
            'User-Agent': 'Mozilla/5.0 (Linux; Android 6.0; Nexus 5 Build/MRA58N) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Mobile Safari/537.36',
            'Origin': 'https://embedsports.top',
            'Referer': 'https://embedsports.top/',
            'Accept': '*/*',
            'Accept-Language': 'en-US,en;q=0.9',
            'DNT': '1',
            'Sec-Ch-Ua': '"Brave";v="143", "Chromium";v="143", "Not A(Brand";v="24"',
            'Sec-Ch-Ua-Mobile': '?1',
            'Sec-Ch-Ua-Platform': '"Android"',
            'Sec-Fetch-Dest': 'empty',
            'Sec-Fetch-Mode': 'cors',
            'Sec-Fetch-Site': 'cross-site',
            'Sec-GPC': '1',
            'Connection': 'keep-alive'
        };
        
        // Connection pooling
        this.httpsAgent = new https.Agent({
            keepAlive: true,
            maxSockets: 50,
            maxFreeSockets: 10,
            timeout: 5000
        });
    }

    start() {
        const server = http.createServer((req, res) => {
            this.handleRequest(req, res);
        });

        server.listen(this.port, () => {
            console.log(`🎺 Sports Proxy Server running on http://localhost:${this.port}`);
            console.log('\n📺 Usage:');
            console.log(`   M3U8: http://localhost:${this.port}/pl_sun/tracks-v1a1/mono.ts.m3u8`);
            console.log(`   Test player: http://localhost:${this.port}/test`);
            console.log(`   Proxy endpoint: http://localhost:${this.port}/sports-proxy?url=encodedUrl`);
            console.log('\n✅ This proxy works specifically for sports streams!');
        });

        return server;
    }

    handleRequest(req, res) {
        const parsedUrl = url.parse(req.url);
        const path = parsedUrl.pathname;
        
        // Handle test endpoint
        if (path === '/') {
            res.writeHead(200, { 'Content-Type': 'text/html' });
            res.end(`
                <!DOCTYPE html>
                <html>
                <head>
                    <title>Sports Proxy Test</title>
                    <script src="https://cdn.jsdelivr.net/npm/hls.js@latest"></script>
                    <style>
                        body { font-family: Arial, sans-serif; padding: 20px; background: #000; color: #fff; }
                        video { width: 100%; max-width: 800px; }
                        input { width: 100%; padding: 10px; margin: 10px 0; }
                        button { padding: 10px 20px; background: #dc2626; color: white; border: none; cursor: pointer; }
                    </style>
                </head>
                <body>
                    <h1>🎺 Sports Proxy Test</h1>
                    <input type="text" id="m3u8Url" placeholder="Enter M3U8 URL" value="/pl_sun/tracks-v1a1/mono.ts.m3u8">
                    <button onclick="loadStream()">Load Stream</button>
                    <video id="video" controls></video>
                    <script>
                        function loadStream() {
                            const url = document.getElementById('m3u8Url').value;
                            const video = document.getElementById('video');
                            
                            if (Hls.isSupported()) {
                                const hls = new Hls();
                                hls.loadSource(url);
                                hls.attachMedia(video);
                                hls.on(Hls.Events.MANIFEST_PARSED, () => video.play());
                            } else if (video.canPlayType('application/vnd.apple.mpegurl')) {
                                video.src = url;
                                video.play();
                            }
                        }
                    </script>
                </body>
                </html>
            `);
            return;
        }
        
        // Handle sports-proxy endpoint
        if (path === '/sports-proxy') {
            const targetUrl = parsedUrl.query.url;
            if (!targetUrl) {
                res.writeHead(400, { 'Content-Type': 'text/plain' });
                res.end('URL parameter required');
                return;
            }
            
            console.log(`🔄 Proxying: ${targetUrl}`);
            this.proxyRequest(targetUrl, req, res);
            return;
        }
        
        // Direct M3U8/segment proxy
        if (path.startsWith('/pl_') || path.includes('.m3u8') || path.includes('.ts')) {
            const targetUrl = this.targetBase + path;
            console.log(`🔄 Direct proxy: ${path}`);
            this.proxyRequest(targetUrl, req, res);
            return;
        }
        
        // 404 for other paths
        res.writeHead(404, { 'Content-Type': 'text/plain' });
        res.end('Not Found');
    }
    
    proxyRequest(targetUrl, req, res) {
        const options = url.parse(targetUrl);
        options.headers = { ...this.headers };
        options.method = req.method;
        options.agent = this.httpsAgent;
        
        const proxyReq = https.request(options, (proxyRes) => {
            const headers = { ...proxyRes.headers };
            delete headers['content-encoding'];
            delete headers['content-length'];
            
            res.writeHead(proxyRes.statusCode, headers);
            proxyRes.pipe(res);
        });
        
        proxyReq.on('error', (err) => {
            console.error(`❌ Proxy error: ${err.message}`);
            res.writeHead(500, { 'Content-Type': 'text/plain' });
            res.end('Proxy error');
        });
        
        proxyReq.setTimeout(5000, () => {
            proxyReq.destroy();
        });
        
        if (req.method !== 'GET' && req.method !== 'HEAD') {
            req.pipe(proxyReq);
        } else {
            proxyReq.end();
        }
    }
}

// Start server if run directly
if (require.main === module) {
    const port = process.argv[2] || 6001;
    const proxy = new SportsProxyServer(port);
    proxy.start();
}

module.exports = SportsProxyServer;
