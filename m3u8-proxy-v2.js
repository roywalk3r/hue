#!/usr/bin/env node
/**
 * Improved M3U8 Proxy Server with connection pooling
 */

const http = require('http');
const https = require('https');
const url = require('url');

class M3U8ProxyV2 {
    constructor(port = 6000) {
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
        
        // Cache for M3U8 responses to reduce repeated requests
        this.cache = new Map();
        this.cacheTimeout = 2000; // 2 seconds cache for M3U8
        
        // Increase agent limits for better performance
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
            console.log(`🚀 M3U8 Proxy V2 running on http://localhost:${this.port}`);
            console.log('\n📺 Usage:');
            console.log(`   Main M3U8: http://localhost:${this.port}/pl_sun/tracks-v1a1/mono.ts.m3u8`);
            console.log(`   Test player: http://localhost:${this.port}/test`);
            console.log('\n✅ Improvements:');
            console.log('   - Connection pooling');
            console.log('   - Response caching');
            console.log('   - Better error handling');
            console.log('   - Reduced logging');
        });

        return server;
    }

    handleRequest(req, res) {
        const parsedUrl = url.parse(req.url);
        const path = parsedUrl.pathname;

        // Handle test endpoint
        if (path === '/test') {
            res.writeHead(200, { 'Content-Type': 'text/html' });
            res.end(`
                <!DOCTYPE html>
                <html>
                <head>
                    <title>M3U8 Test Player V2</title>
                    <script src="https://cdn.jsdelivr.net/npm/hls.js@latest"></script>
                    <style>
                        body { font-family: Arial, sans-serif; max-width: 1000px; margin: 0 auto; padding: 20px; }
                        #video { width: 100%; max-width: 800px; }
                        .stats { margin-top: 20px; padding: 10px; background: #f0f0f0; border-radius: 5px; }
                    </style>
                </head>
                <body>
                    <h1>M3U8 Live Stream Player</h1>
                    <video id="video" controls></video>
                    <div class="stats" id="stats">Loading...</div>
                    <script>
                        const video = document.getElementById('video');
                        const stats = document.getElementById('stats');
                        let bufferCount = 0;
                        
                        if (Hls.isSupported()) {
                            const hls = new Hls({
                                debug: false,
                                enableWorker: true,
                                lowLatencyMode: true,
                                backBufferLength: 90
                            });
                            
                            hls.loadSource('/pl_sun/tracks-v1a1/mono.ts.m3u8');
                            hls.attachMedia(video);
                            
                            hls.on(Hls.Events.MANIFEST_PARSED, function() {
                                video.play();
                            });
                            
                            hls.on(Hls.Events.FRAG_BUFFERED, function() {
                                bufferCount++;
                                stats.innerHTML = \`Segments buffered: \${bufferCount}<br>
                                Live latency: \${Math.round(hls.latency)}s<br>
                                Position: \${Math.round(video.currentTime)}s\`;
                            });
                            
                            setInterval(() => {
                                if (hls.latency) {
                                    stats.innerHTML = \`Segments buffered: \${bufferCount}<br>
                                    Live latency: \${Math.round(hls.latency)}s<br>
                                    Position: \${Math.round(video.currentTime)}s\`;
                                }
                            }, 1000);
                        } else if (video.canPlayType('application/vnd.apple.mpegurl')) {
                            video.src = '/pl_sun/tracks-v1a1/mono.ts.m3u8';
                            video.addEventListener('loadedmetadata', function() {
                                video.play();
                            });
                        }
                    </script>
                </body>
                </html>
            `);
            return;
        }

        // Check cache for M3U8 files
        if (path.endsWith('.m3u8')) {
            const cached = this.cache.get(path);
            if (cached && (Date.now() - cached.timestamp < this.cacheTimeout)) {
                res.writeHead(200, cached.headers);
                res.end(cached.data);
                return;
            }
        }

        // Construct target URL
        const targetUrl = this.targetBase + path;
        
        // Only log M3U8 requests to reduce noise
        if (path.endsWith('.m3u8')) {
            console.log(`🔄 M3U8 Request: ${path}`);
        }

        // Make request to target
        const options = url.parse(targetUrl);
        options.headers = { ...this.headers };
        options.method = req.method;
        options.agent = this.httpsAgent;

        const proxyReq = https.request(options, (proxyRes) => {
            // Copy headers from response
            const headers = { ...proxyRes.headers };
            
            // Remove problematic headers
            delete headers['content-encoding'];
            delete headers['content-length'];
            delete headers['transfer-encoding'];
            
            res.writeHead(proxyRes.statusCode, headers);
            
            // Cache M3U8 responses
            if (path.endsWith('.m3u8') && proxyRes.statusCode === 200) {
                const chunks = [];
                proxyRes.on('data', chunk => chunks.push(chunk));
                proxyRes.on('end', () => {
                    const data = Buffer.concat(chunks).toString();
                    this.cache.set(path, {
                        data: data,
                        headers: headers,
                        timestamp: Date.now()
                    });
                    res.end(data);
                });
            } else {
                proxyRes.pipe(res);
            }
        });

        proxyReq.on('error', (err) => {
            // Silently handle errors to reduce log noise
            if (!res.headersSent) {
                res.writeHead(502, { 'Content-Type': 'text/plain' });
                res.end('Proxy error');
            }
        });

        // Set timeout
        proxyReq.setTimeout(3000, () => {
            proxyReq.destroy();
        });

        // Pipe request body if present
        if (req.method !== 'GET' && req.method !== 'HEAD') {
            req.pipe(proxyReq);
        } else {
            proxyReq.end();
        }
    }
}

// Start proxy if run directly
if (require.main === module) {
    const port = process.argv[2] || 8000;
    const proxy = new M3U8ProxyV2(port);
    proxy.start();
}

module.exports = M3U8ProxyV2;
