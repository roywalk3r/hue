/**
 * PPV.to Provider
 * Fetches live streams from api.ppv.to
 */

const axios = require('axios');
const LivesportWatchScraper = require('../livesport-watch-scraper');

class PPVProvider {
    constructor() {
        this.scraper = new LivesportWatchScraper();
        this.baseURL = 'https://api.ppv.to/api';
        this.client = axios.create({
            timeout: 10000,
            headers: {
                'User-Agent': 'Mozilla/5.0 (iPhone; CPU iPhone OS 18_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.5 Mobile/15E148 Safari/604.1',
                'Referer': 'https://ppv.to/'
            }
        });
    }

    /**
     * Extract match name from path
     * /sports-stream/italy-vs-great-britain-md-round-robin -> italy-vs-great-britain-md-round-robin
     */
    extractMatchName(path) {
        const match = path.match(/\/sports-stream\/(.+?)(?:\/|$)/);
        return match ? match[1].replace(/\/$/, '') : null;
    }

    /**
     * Get stream info from PPV API
     */
    async getPPVStream(uri) {
        try {
            // Find the category and slug from the URI
            // Example URI: olympics/2026-02-08/curling/italy-vs-great-britain-md-round-robin-1770573900
            const response = await this.client.get(`${this.baseURL}/streams/${uri}`);
            if (response.data && response.data.success) {
                return response.data.data;
            }
            return null;
        } catch (error) {
            console.error('[PPV] API error:', error.message);
            return null;
        }
    }

    /**
     * Decode the obfuscated modistreams/fetch response
     */
    decodeModistreams(encoded, uri) {
        try {
            // The encoding used by modistreams/vidsrc/etc is often a character rotation
            // followed by base64. Based on "CrQBSmlr..." -> "laligatv" pattern:
            
            const rotate = (str) => {
                return str.split('').map(char => {
                    const code = char.charCodeAt(0);
                    if (code >= 65 && code <= 90) return String.fromCharCode(((code - 65 + 13) % 26) + 65);
                    if (code >= 97 && code <= 122) return String.fromCharCode(((code - 97 + 13) % 26) + 97);
                    return char;
                }).join('');
            };

            // First split by space (payload + encoded + origin)
            const parts = encoded.split(' ');
            const targetPart = parts.length > 1 ? parts[1] : encoded;
            
            // Rotation (ROT13)
            const rotated = rotate(targetPart);
            
            // Base64 decode
            const decoded = Buffer.from(rotated, 'base64').toString('utf8');
            
            // If decoded is gibberish but contains a known slug pattern, use it
            const commonPatterns = ['nbatv', 'espn', 'tnt', 'laliga', 'seriea', 'ligue1', 'nfl', 'ufc', 'pl_'];
            for (const p of commonPatterns) {
                if (decoded.toLowerCase().includes(p)) {
                    const match = decoded.match(/([a-zA-Z0-9_-]+)$/);
                    if (match) return match[1];
                }
            }

            console.log(`[PPV] Decoded stream ID: ${decoded}`);
            return decoded;
        } catch (e) {
            console.log('[PPV] Decoding failed:', e.message);
            return null;
        }
    }

    /**
     * Fetch stream ID from modistreams.org/fetch
     */
    async fetchModistreams(uri) {
        try {
            console.log(`[PPV] Fetching stream for URI: ${uri}`);
            
            const { execSync } = require('child_process');
            const userAgent = 'Mozilla/5.0 (iPhone; CPU iPhone OS 18_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.5 Mobile/15E148 Safari/604.1';
            const cmd = `curl -L -s -m 10 -H "User-Agent: ${userAgent}" -H "Referer: https://modistreams.org/embed/${uri}" -H "Origin: https://modistreams.org" -H "Content-Type: text/plain" --data "${uri}" "https://modistreams.org/fetch"`;
            
            const output = execSync(cmd).toString();

            if (output) {
                // Try to identify the slug from the response
                const slug = this.decodeModistreams(output, uri);

                if (slug) {
                    console.log(`[PPV] Identified slug: ${slug}`);
                    return await this.scraper.extractor.decodeStreamId(slug, `https://modistreams.org/embed/${uri}`);
                }
            }
        } catch (error) {
            console.log('[PPV] modistreams/fetch (curl) failed:', error.message);
        }
        return null;
    }

    /**
     * Main fetch function - compatible with provider registry
     */
    async getStreams(ctx) {
        const { req, matchName } = ctx;
        if (!req && !matchName) return [];

        let finalMatchName = matchName || this.extractMatchName(req.path);
        if (!finalMatchName) return [];

        console.log(`[PPV] Fetching streams for: ${finalMatchName}`);

        try {
            const listRes = await this.client.get(`${this.baseURL}/streams`);
            let matchUri = null;
            let matchData = null;

            if (listRes.data && listRes.data.success) {
                for (const cat of listRes.data.streams) {
                    const found = cat.streams.find(s => {
                        const uri = s.uri_name.toLowerCase();
                        const name = s.name.toLowerCase();
                        const search = finalMatchName.toLowerCase();
                        const searchSpaces = search.replace(/-/g, ' ');
                        
                        return uri.includes(search) || 
                               search.includes(uri) ||
                               name.includes(searchSpaces) ||
                               searchSpaces.includes(name);
                    });
                    if (found) {
                        matchUri = found.uri_name;
                        matchData = found;
                        break;
                    }
                }
            }

            if (!matchUri) {
                console.log('[PPV] Match not found in live list');
                return [];
            }

            // 1. Try direct fetch first (fastest)
            let m3u8Url = await this.fetchModistreams(matchUri);

            // 2. If direct fetch failed, try browser extraction from the details source
            if (!m3u8Url) {
                const details = await this.getPPVStream(matchUri);
                if (details && details.sources) {
                    const iframeSource = details.sources.find(s => s.type === 'iframe');
                    if (iframeSource) {
                        console.log(`[PPV] Falling back to browser extraction: ${iframeSource.data}`);
                        m3u8Url = await this.scraper.extractor.extractM3U8FromEmbed(iframeSource.data);
                    }
                }
            }

            if (m3u8Url) {
                const proxyBase = 'https://sport-proxy.roywalk3r.deno.net';
                const headers = {
                    'Referer': 'https://modistreams.org/',
                    'Origin': 'https://modistreams.org',
                    'User-Agent': 'Mozilla/5.0 (iPhone; CPU iPhone OS 18_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.5 Mobile/15E148 Safari/604.1'
                };

                const finalUrl = `${proxyBase}/m3u8-proxy?url=${encodeURIComponent(m3u8Url)}&headers=${encodeURIComponent(JSON.stringify(headers))}`;

                return [{
                    title: matchData ? matchData.name : 'PPV Stream',
                    url: finalUrl,
                    provider: 'ppv',
                    source: 'modistreams',
                    isLive: true,
                    headers: headers,
                    originalUrl: m3u8Url,
                    isProxied: true
                }];
            }

            return [];
        } catch (error) {
            console.error('[PPV] Error:', error.message);
            return [];
        }
    }
}

// Export the function for the registry
async function getPPVStreams() {
    const provider = new PPVProvider();
    return await provider.getStreams(arguments[0]);
}

module.exports = { getPPVStreams };
