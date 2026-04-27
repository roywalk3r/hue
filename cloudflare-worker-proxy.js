/**
 * Hue Stream API
 * Professional Streaming Aggregator
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */

/**
 * Cloudflare Worker for M3U8 Proxy
 * Deploy to: https://workers.cloudflare.com
 */

const TARGET_BASE = 'https://gg.poocloud.in';
const CACHE_TTL = 2; // 2 seconds cache for M3U8

// Headers to forward to the target
const HEADERS = {
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
    'Sec-GPC': '1'
};

// CORS headers for responses
const CORS_HEADERS = {
    'Access-Control-Allow-Origin': '*',
    'Access-Control-Allow-Methods': 'GET, HEAD, OPTIONS',
    'Access-Control-Allow-Headers': '*',
    'Access-Control-Max-Age': '86400'
};

addEventListener('fetch', event => {
    event.respondWith(handleRequest(event.request));
});

async function handleRequest(request) {
    const url = new URL(request.url);
    const path = url.pathname;

    // Handle CORS preflight
    if (request.method === 'OPTIONS') {
        return new Response(null, { headers: CORS_HEADERS });
    }

    // Handle test page
    if (path === '/') {
        return new Response(getTestPage(), {
            headers: {
                'Content-Type': 'text/html',
                ...CORS_HEADERS
            }
        });
    }

    // Only allow GET/HEAD for M3U8 and segments
    if (request.method !== 'GET' && request.method !== 'HEAD') {
        return new Response('Method not allowed', { status: 405 });
    }

    // Construct target URL
    const targetUrl = TARGET_BASE + path;

    try {
        // Create cache key
        const cacheKey = new Request(targetUrl, {
            headers: HEADERS
        });

        // Check cache for M3U8 files
        if (path.endsWith('.m3u8')) {
            const cache = caches.default;
            let response = await cache.match(cacheKey);

            if (response) {
                // Return cached response with CORS headers
                response = new Response(response.body, {
                    status: response.status,
                    statusText: response.statusText,
                    headers: {
                        ...response.headers,
                        ...CORS_HEADERS
                    }
                });
                return response;
            }
        }

        // Fetch from target
        const response = await fetch(targetUrl, {
            method: request.method,
            headers: HEADERS
        });

        // Copy response headers
        const headers = new Headers(response.headers);
        
        // Remove problematic headers
        headers.delete('content-encoding');
        headers.delete('content-length');
        headers.delete('transfer-encoding');
        
        // Add CORS headers
        Object.entries(CORS_HEADERS).forEach(([key, value]) => {
            headers.set(key, value);
        });

        // Create new response
        const newResponse = new Response(response.body, {
            status: response.status,
            statusText: response.statusText,
            headers: headers
        });

        // Cache M3U8 responses
        if (path.endsWith('.m3u8') && response.status === 200) {
            const cache = caches.default;
            await cache.put(cacheKey, newResponse.clone());
        }

        return newResponse;

    } catch (error) {
        console.error('Proxy error:', error);
        return new Response('Proxy error', {
            status: 502,
            headers: CORS_HEADERS
        });
    }
}

function getTestPage() {
    return `<!DOCTYPE html>
<html>
<head>
    <title>Live Stream Player</title>
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <script src="https://cdn.jsdelivr.net/npm/hls.js@latest"></script>
    <style>
        * { margin: 0; padding: 0; box-sizing: border-box; }
        body { 
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; 
            background: #000; 
            color: #fff; 
            min-height: 100vh;
            display: flex;
            flex-direction: column;
        }
        .container { 
            max-width: 1200px; 
            margin: 0 auto; 
            padding: 20px;
            width: 100%;
        }
        .header {
            text-align: center;
            margin-bottom: 30px;
        }
        .header h1 {
            font-size: 2em;
            margin-bottom: 10px;
        }
        .video-container {
            position: relative;
            background: #111;
            border-radius: 10px;
            overflow: hidden;
            margin-bottom: 20px;
        }
        #video { 
            width: 100%; 
            display: block;
        }
        .stats {
            background: #111;
            padding: 15px;
            border-radius: 10px;
            font-family: monospace;
            font-size: 14px;
            line-height: 1.6;
        }
        .stats .good { color: #4ade80; }
        .stats .warning { color: #fbbf24; }
        .controls {
            display: flex;
            gap: 10px;
            margin-bottom: 20px;
            flex-wrap: wrap;
        }
        .btn {
            background: #333;
            color: #fff;
            border: none;
            padding: 10px 20px;
            border-radius: 5px;
            cursor: pointer;
            font-size: 14px;
            transition: background 0.2s;
        }
        .btn:hover { background: #444; }
        .btn.active { background: #dc2626; }
        .url-input {
            flex: 1;
            min-width: 300px;
            background: #333;
            color: #fff;
            border: 1px solid #444;
            padding: 10px;
            border-radius: 5px;
            font-size: 14px;
        }
        @media (max-width: 768px) {
            .container { padding: 10px; }
            .header h1 { font-size: 1.5em; }
        }
    </style>
</head>
<body>
    <div class="container">
        <div class="header">
            <h1>🎺 Live Stream Player</h1>
            <p>Powered by Cloudflare Workers</p>
        </div>
        
        <div class="controls">
            <input type="text" class="url-input" id="streamUrl" value="/pl_sun/tracks-v1a1/mono.ts.m3u8" placeholder="Enter M3U8 URL">
            <button class="btn" id="loadBtn">Load Stream</button>
            <button class="btn" id="muteBtn">Mute</button>
            <button class="btn" id="fullscreenBtn">Fullscreen</button>
        </div>
        
        <div class="video-container">
            <video id="video" controls muted></video>
        </div>
        
        <div class="stats" id="stats">
            Loading player...
        </div>
    </div>

    <script>
        const video = document.getElementById('video');
        const stats = document.getElementById('stats');
        const loadBtn = document.getElementById('loadBtn');
        const muteBtn = document.getElementById('muteBtn');
        const fullscreenBtn = document.getElementById('fullscreenBtn');
        const streamUrl = document.getElementById('streamUrl');
        
        let hls;
        let bufferCount = 0;
        let isPlaying = false;

        function updateStats(message, type = 'normal') {
            const className = type === 'good' ? 'good' : type === 'warning' ? 'warning' : '';
            stats.innerHTML = \`<div class="\${className}">\${message}</div>\`;
        }

        function loadStream(url) {
            if (hls) {
                hls.destroy();
            }
            
            bufferCount = 0;
            updateStats('Loading stream...', 'warning');
            
            if (Hls.isSupported()) {
                hls = new Hls({
                    debug: false,
                    enableWorker: true,
                    lowLatencyMode: true,
                    backBufferLength: 90,
                    maxBufferLength: 30,
                    maxMaxBufferLength: 60
                });
                
                hls.loadSource(url);
                hls.attachMedia(video);
                
                hls.on(Hls.Events.MANIFEST_PARSED, function() {
                    updateStats('Stream loaded! Press play to start.', 'good');
                    video.muted = true;
                    video.play().catch(e => {
                        updateStats('Click play to start streaming', 'warning');
                    });
                });
                
                hls.on(Hls.Events.ERROR, function(event, data) {
                    if (data.fatal) {
                        switch(data.type) {
                            case Hls.ErrorTypes.NETWORK_ERROR:
                                updateStats('Network error, retrying...', 'warning');
                                hls.startLoad();
                                break;
                            case Hls.ErrorTypes.MEDIA_ERROR:
                                updateStats('Media error, recovering...', 'warning');
                                hls.recoverMediaError();
                                break;
                            default:
                                updateStats('Fatal error: ' + data.details, 'warning');
                                break;
                        }
                    }
                });
                
                hls.on(Hls.Events.FRAG_BUFFERED, function() {
                    bufferCount++;
                    updateStats(\`
                        Segments buffered: \${bufferCount}<br>
                        Live latency: \${hls.latency ? Math.round(hls.latency) + 's' : 'N/A'}<br>
                        Position: \${Math.round(video.currentTime)}s<br>
                        Buffer: \${Math.round(hls.buffered.length ? hls.buffered.end(0) - video.currentTime : 0)}s
                    \`, 'good');
                });
                
            } else if (video.canPlayType('application/vnd.apple.mpegurl')) {
                video.src = url;
                updateStats('Using native HLS player', 'good');
            } else {
                updateStats('HLS not supported in this browser', 'warning');
            }
        }

        // Event listeners
        loadBtn.addEventListener('click', () => {
            const url = streamUrl.value.trim();
            if (url) {
                loadStream(url);
            }
        });

        muteBtn.addEventListener('click', () => {
            video.muted = !video.muted;
            muteBtn.textContent = video.muted ? 'Unmute' : 'Mute';
            muteBtn.classList.toggle('active', video.muted);
        });

        fullscreenBtn.addEventListener('click', () => {
            if (!document.fullscreenElement) {
                video.requestFullscreen();
            } else {
                document.exitFullscreen();
            }
        });

        video.addEventListener('play', () => {
            isPlaying = true;
            loadBtn.textContent = 'Reload';
        });

        video.addEventListener('pause', () => {
            isPlaying = false;
        });

        // Auto-load initial stream
        loadStream(streamUrl.value);
    </script>
</body>
</html>`;
}
