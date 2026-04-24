/**
 * PPV.to Provider
 * Fetches live streams from api.ppv.to
 */

const axios = require('axios');
const { execFileSync } = require('child_process');
const LivesportWatchScraper = require('../livesport-watch-scraper');
const { resolveWithPlaywright } = require('../utils/playwrightResolver');

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

    isPpvUriPath(value) {
        return /^[a-z0-9-]+\/\d{4}-\d{2}-\d{2}\/[a-z0-9-]+$/i.test(String(value || '').trim());
    }

    parsePpvUri(value) {
        const m = String(value || '').trim().match(/^([a-z0-9-]+)\/(\d{4}-\d{2}-\d{2})\/([a-z0-9-]+)$/i);
        if (!m) return null;
        return { competition: m[1].toLowerCase(), date: m[2], slug: m[3].toLowerCase() };
    }

    deriveSlugFromUri(uri) {
        const parsed = this.parsePpvUri(uri);
        if (!parsed) return null;
        const homeShort = parsed.slug.split('-')[0];
        if (!homeShort) return null;
        return `${parsed.competition}_${homeShort}`.toLowerCase();
    }

    encodeVarint(value) {
        const out = [];
        let n = Number(value) >>> 0;
        while (n >= 0x80) {
            out.push((n & 0x7f) | 0x80);
            n >>>= 7;
        }
        out.push(n);
        return Buffer.from(out);
    }

    buildModistreamsPayload(uri) {
        const text = String(uri || '');
        const body = Buffer.from(text, 'utf8');
        // Protobuf wire format: field #1 (length-delimited) + bytes(uri)
        return Buffer.concat([Buffer.from([0x0a]), this.encodeVarint(body.length), body]);
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

    extractSlugFromFetchResponse(raw, uri) {
        const fallback = this.deriveSlugFromUri(uri);
        if (!raw) return fallback;
        const text = Buffer.isBuffer(raw) ? raw.toString('utf8') : String(raw);
        if (!text) return fallback;

        const parsed = this.parsePpvUri(uri);
        const requiredPrefix = parsed ? `${parsed.competition}_` : null;

        // Try direct plain-text slug match first.
        const directMatches = text.match(/[a-z0-9]+_[a-z0-9]+/ig) || [];
        for (const m of directMatches) {
            const cand = m.toLowerCase();
            if (!requiredPrefix || cand.startsWith(requiredPrefix)) return cand;
        }

        // Some PPV responses append a plain token at the tail (e.g., "laligatv").
        const tailTokenMatch = text.match(/[\x00-\x1f]([a-z0-9_]{5,})\s*$/i);
        if (tailTokenMatch && tailTokenMatch[1]) {
            const tailToken = tailTokenMatch[1].toLowerCase();
            if (!requiredPrefix || tailToken.startsWith(requiredPrefix) || /tv$/.test(tailToken)) {
                return tailToken;
            }
        }

        // Then scan base64-like chunks and decode candidates.
        const b64Chunks = text.match(/[A-Za-z0-9+/=]{8,}/g) || [];
        for (let i = b64Chunks.length - 1; i >= 0; i--) {
            const token = b64Chunks[i].replace(/[^A-Za-z0-9+/=]/g, '');
            if (!token) continue;
            try {
                const decoded = Buffer.from(token, 'base64').toString('utf8');
                const hits = decoded.match(/[a-z0-9]+_[a-z0-9]+/ig) || [];
                for (const h of hits) {
                    const cand = h.toLowerCase();
                    if (!requiredPrefix || cand.startsWith(requiredPrefix)) return cand;
                }
            } catch (_) {
                // ignore invalid base64 chunks
            }
        }

        return fallback;
    }

    /**
     * Fetch stream ID from modistreams.org/fetch
     */
    async fetchModistreams(uri) {
        try {
            console.log(`[PPV] Fetching stream for URI: ${uri}`);
            const userAgent = 'Mozilla/5.0 (iPhone; CPU iPhone OS 18_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.5 Mobile/15E148 Safari/604.1';
            const output = execFileSync('curl', [
                '-L', '-s', '-m', '12',
                '-H', `User-Agent: ${userAgent}`,
                '-H', `Referer: https://modistreams.org/embed/${uri}`,
                '-H', 'Origin: https://modistreams.org',
                '-H', 'Content-Type: application/octet-stream',
                '--data-binary', '@-',
                'https://modistreams.org/fetch'
            ], {
                input: this.buildModistreamsPayload(uri),
                maxBuffer: 1024 * 1024 * 2
            });

            if (!output || output.length === 0) return null;

            const trimmed = output.toString('utf8').trim();
            if (
                trimmed.startsWith('{') &&
                (trimmed.includes('"errors"') || trimmed.includes('Internal Server Error'))
            ) {
                console.log('[PPV] modistreams/fetch returned error payload');
                return null;
            }

            // Try to identify a stable slug from binary response; fallback to URI-derived slug.
            let slug = this.extractSlugFromFetchResponse(output, uri);
            if (!slug) slug = this.decodeModistreams(trimmed, uri);

            if (!slug || /[^\w-]/.test(slug) || slug.includes('�')) {
                console.log(`[PPV] Invalid/garbled slug from fetch response: ${slug || 'null'}`);
                return null;
            }

            console.log(`[PPV] Identified slug: ${slug}`);
            return `https://gg.poocloud.in/${slug}/tracks-v1a1/mono.ts.m3u8`;
        } catch (error) {
            console.log('[PPV] modistreams/fetch (curl) failed:', error.message);
        }
        return null;
    }

    async triggerPlaybackGesture(page, rounds = 2) {
        if (!page) return;
        const selectors = [
            '.jw-display-icon-container',
            '.jw-icon-playback',
            '.vjs-big-play-button',
            'button[aria-label*="play" i]',
            '#player',
            '.player',
            'video',
            'body'
        ];

        for (let i = 0; i < rounds; i++) {
            for (const sel of selectors) {
                try {
                    const el = await page.$(sel);
                    if (el) await el.click({ timeout: 500, force: true }).catch(() => {});
                } catch (_) {}
            }

            try { await page.mouse.move(682, 430); } catch (_) {}
            try { await page.mouse.click(682, 430, { delay: 40 }); } catch (_) {}
            try { await page.keyboard.press('Space'); } catch (_) {}

            try {
                await page.evaluate(() => {
                    const fire = (el, type) => {
                        try {
                            el.dispatchEvent(new MouseEvent(type, { bubbles: true, cancelable: true, view: window }));
                        } catch (_) {}
                    };
                    const target = document.querySelector('video, #player, .jwplayer, .player, body');
                    if (target) {
                        fire(target, 'pointerdown');
                        fire(target, 'pointerup');
                        fire(target, 'mousedown');
                        fire(target, 'mouseup');
                        fire(target, 'click');
                    }
                    const videos = Array.from(document.querySelectorAll('video'));
                    for (const v of videos) {
                        try { v.muted = true; } catch (_) {}
                        try { v.autoplay = true; } catch (_) {}
                        try { v.playsInline = true; } catch (_) {}
                        try { v.play().catch(() => {}); } catch (_) {}
                    }
                });
            } catch (_) {}

            try { await page.waitForTimeout(420); } catch (_) {}
        }
    }

    captureSecureM3u8ViaGasmXvfb(uri) {
        try {
            const script = `
const { chromium } = require('playwright-extra');
const StealthPlugin = require('puppeteer-extra-plugin-stealth');
chromium.use(StealthPlugin());

(async () => {
  const uri = process.env.PPV_URI;
  const pulse = async (page, rounds = 2) => {
    const selectors = [
      '.jw-display-icon-container',
      '.jw-icon-playback',
      '.vjs-big-play-button',
      'button[aria-label*="play" i]',
      '#player',
      '.player',
      'video',
      'body'
    ];
    for (let i = 0; i < rounds; i++) {
      for (const sel of selectors) {
        try {
          const el = await page.$(sel);
          if (el) await el.click({ timeout: 450, force: true }).catch(() => {});
        } catch (_) {}
      }
      try { await page.mouse.move(682, 430); } catch (_) {}
      try { await page.mouse.click(682, 430, { delay: 40 }); } catch (_) {}
      try { await page.keyboard.press('Space'); } catch (_) {}
      try {
        await page.evaluate(() => {
          const fire = (el, type) => {
            try {
              el.dispatchEvent(new MouseEvent(type, { bubbles: true, cancelable: true, view: window }));
            } catch (_) {}
          };
          const target = document.querySelector('video, #player, .jwplayer, .player, body');
          if (target) {
            fire(target, 'pointerdown');
            fire(target, 'pointerup');
            fire(target, 'mousedown');
            fire(target, 'mouseup');
            fire(target, 'click');
          }
          const videos = Array.from(document.querySelectorAll('video'));
          for (const v of videos) {
            try { v.muted = true; } catch (_) {}
            try { v.autoplay = true; } catch (_) {}
            try { v.playsInline = true; } catch (_) {}
            try { v.play().catch(() => {}); } catch (_) {}
          }
        });
      } catch (_) {}
      await page.waitForTimeout(420);
    }
  };

  const browser = await chromium.launch({ headless: true, args: ['--no-sandbox', '--disable-setuid-sandbox'] });
  const context = await browser.newContext({
    userAgent: 'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/145.0.0.0 Safari/537.36',
    viewport: { width: 1365, height: 860 }
  });
  const page = await context.newPage();
  const discovered = [];
  const push = (u) => {
    if (!u || typeof u !== 'string') return;
    if (!u.includes('/secure/')) return;
    if (!u.includes('.m3u8')) return;
    if (!discovered.includes(u)) discovered.push(u);
  };
  page.on('request', req => push(req.url()));
  page.on('response', res => push(res.url()));

  await page.goto(\`https://modistreams.org/embed/\${uri}\`, { waitUntil: 'domcontentloaded', timeout: 60000 });
  await page.waitForTimeout(3000);
  await pulse(page, 3);
  await page.evaluate(async (matchUri) => {
    const gasm = await import('/js/wasm/gasm.js');
    try { await gasm.default('/js/wasm/gasm.wasm'); } catch (_) {}
    const run = async (p, ms) => {
      try {
        await Promise.race([
          Promise.resolve(p),
          new Promise((_, reject) => setTimeout(() => reject(new Error('timeout')), ms))
        ]);
      } catch (_) {}
    };
    await run(gasm.set_stream_jw(matchUri), 7000);
    await run(gasm.set_stream_jw(matchUri, 'jw'), 7000);
    await run(gasm.set_stream(matchUri), 5000);
  }, uri);
  const started = Date.now();
  while (Date.now() - started < 12000) {
    await pulse(page, 1);
    if (discovered.some(u => /\\/secure\\/.+\\/index\\.m3u8(\\?|$)/i.test(u))) break;
    if (discovered.some(u => /\\/secure\\/.+\\.m3u8(\\?|$)/i.test(u))) break;
    await page.waitForTimeout(500);
  }

  const pick = discovered.find(u => /\\/secure\\/.+\\/index\\.m3u8(\\?|$)/i.test(u))
    || discovered.find(u => /\\/secure\\/.+\\.m3u8(\\?|$)/i.test(u))
    || '';
  console.log('__SECURE__=' + pick);
  await context.close();
  await browser.close();
})().catch((e) => {
  console.log('__SECURE__=');
});
`;

            const output = execFileSync('xvfb-run', ['-a', 'node', '-'], {
                input: script,
                env: { ...process.env, PPV_URI: uri },
                maxBuffer: 1024 * 1024 * 4,
                timeout: 70000
            }).toString('utf8');

            const match = output.match(/__SECURE__=(https?:\/\/[^\s]+)/);
            if (match && match[1]) {
                console.log(`[PPV] GASM xvfb secure capture success: ${match[1]}`);
                return match[1];
            }
        } catch (error) {
            console.log('[PPV] captureSecureM3u8ViaGasmXvfb failed:', error.message);
        }
        return null;
    }

    async captureSecureM3u8ViaGasm(uri) {
        let browser = null;
        let page = null;
        let context = null;
        try {
            console.log(`[PPV] Starting GASM secure capture for: ${uri}`);
            const xvfbPick = this.captureSecureM3u8ViaGasmXvfb(uri);
            if (xvfbPick) return xvfbPick;

            const { chromium } = require('playwright-extra');
            const StealthPlugin = require('puppeteer-extra-plugin-stealth');
            if (!chromium.__ppvStealthInstalled) {
                chromium.use(StealthPlugin());
                chromium.__ppvStealthInstalled = true;
            }

            const embedUrl = `https://modistreams.org/embed/${uri}`;
            const discovered = [];
            const push = (u) => {
                if (!u || typeof u !== 'string') return;
                if (!/^https?:\/\//i.test(u)) return;
                if (!u.includes('/secure/')) return;
                if (!u.includes('.m3u8')) return;
                if (!discovered.includes(u)) discovered.push(u);
            };

            browser = await chromium.launch({
                headless: true,
                args: ['--no-sandbox', '--disable-setuid-sandbox']
            });
            context = await browser.newContext({
                userAgent: 'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/145.0.0.0 Safari/537.36',
                viewport: { width: 1365, height: 860 }
            });
            page = await context.newPage();

            page.on('request', (req) => push(req.url()));
            page.on('response', (res) => push(res.url()));

            await page.goto(embedUrl, { waitUntil: 'domcontentloaded', timeout: 45000 });
            await page.waitForTimeout(2200);
            await this.triggerPlaybackGesture(page, 3);

            // Trigger wasm stream generation. We intentionally don't await these calls;
            // they can reject/hang while still emitting secure URL requests we need.
            await page.evaluate(async (matchUri) => {
                try {
                    const gasm = await import('/js/wasm/gasm.js');
                    try {
                        await gasm.default('/js/wasm/gasm.wasm');
                    } catch (_) {
                        // already initialized or non-fatal init error
                    }
                    const runWithTimeout = async (promiseLike, ms) => {
                        try {
                            await Promise.race([
                                Promise.resolve(promiseLike),
                                new Promise((_, reject) => setTimeout(() => reject(new Error('timeout')), ms))
                            ]);
                        } catch (_) {
                            // ignore: side effects (network requests) are what we need
                        }
                    };

                    const attempts = [
                        () => gasm.set_stream_jw(matchUri),
                        () => gasm.set_stream_jw(matchUri, 'jw'),
                        () => gasm.set_stream(matchUri)
                    ];
                    for (const makeAttempt of attempts) {
                        try { await runWithTimeout(makeAttempt(), 2200); } catch (_) {}
                        try {
                            const videos = Array.from(document.querySelectorAll('video'));
                            for (const v of videos) {
                                try { v.muted = true; } catch (_) {}
                                try { v.play().catch(() => {}); } catch (_) {}
                            }
                        } catch (_) {}
                        await new Promise(resolve => setTimeout(resolve, 250));
                    }
                    await new Promise(resolve => setTimeout(resolve, 1200));
                } catch (_) {
                    // ignore
                }
            }, uri);

            const started = Date.now();
            while (Date.now() - started < 12000) {
                if (discovered.some(u => /\/secure\/.+\/index\.m3u8(\?|$)/i.test(u))) break;
                if (discovered.some(u => /\/secure\/.+\.m3u8(\?|$)/i.test(u))) break;
                await this.triggerPlaybackGesture(page, 1);
                await page.waitForTimeout(350);
            }

            const pick = discovered.find(u => /\/secure\/.+\/index\.m3u8(\?|$)/i.test(u))
                || discovered.find(u => /\/secure\/.+\/tracks-v\d+a\d+\/mono\.ts\.m3u8(\?|$)/i.test(u))
                || discovered.find(u => /\/secure\/.+\.m3u8(\?|$)/i.test(u));

            if (pick) {
                console.log(`[PPV] GASM secure capture success: ${pick}`);
                return pick;
            }
            console.log(`[PPV] GASM secure capture found no secure URL for: ${uri}`);
        } catch (error) {
            console.log('[PPV] captureSecureM3u8ViaGasm failed:', error.message);
        } finally {
            if (context) {
                try { await context.close(); } catch (_) {}
            }
            if (browser) {
                try { await browser.close(); } catch (_) {}
            }
        }
        return null;
    }

    async captureM3u8FromEmbed(embedUrl) {
        let browser;
        try {
            const { chromium } = require('playwright');
            browser = await chromium.launch({
                headless: true,
                args: ['--no-sandbox', '--disable-setuid-sandbox']
            });

            const context = await browser.newContext({
                userAgent: 'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/145.0.0.0 Safari/537.36'
            });
            const page = await context.newPage();

            const found = [];
            const pushUnique = (u) => {
                if (!u || typeof u !== 'string') return;
                if (!/^https?:\/\//i.test(u)) return;
                if (found.includes(u)) return;
                found.push(u);
            };

            page.on('request', (req) => {
                const u = req.url();
                if (u.includes('.m3u8') || u.includes('/secure/')) {
                    pushUnique(u);
                }
            });

            page.on('response', (res) => {
                const u = res.url();
                if (u.includes('.m3u8') || u.includes('/secure/')) {
                    pushUnique(u);
                }
            });

            await page.goto(embedUrl, { waitUntil: 'domcontentloaded', timeout: 45000 });

            // Some embeds only request HLS after explicit interaction.
            const selectors = ['#player', 'video', 'body'];
            const startedAt = Date.now();
            while (Date.now() - startedAt < 35000) {
                if (found.some(u => u.includes('.m3u8') || u.includes('/secure/'))) break;

                for (const sel of selectors) {
                    try {
                        const el = await page.$(sel);
                        if (el) await el.click({ timeout: 600, force: true }).catch(() => {});
                    } catch (_) {}
                }
                await page.waitForTimeout(2500);
            }

            await context.close();

            const pick = found.find(u => /\/index\.m3u8(\?|$)/i.test(u))
                || found.find(u => /\.m3u8(\?|$)/i.test(u))
                || found.find(u => /\/secure\//i.test(u));

            if (pick) {
                console.log(`[PPV] Playwright capture success: ${pick}`);
                return pick;
            }
        } catch (e) {
            console.log('[PPV] captureM3u8FromEmbed failed:', e.message);
        } finally {
            if (browser) {
                try { await browser.close(); } catch (_) {}
            }
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
            let matchUri = null;
            let matchData = null;

            // If caller already provided a PPV URI path, use it directly.
            if (this.isPpvUriPath(finalMatchName)) {
                matchUri = finalMatchName;
            } else {
                const listRes = await this.client.get(`${this.baseURL}/streams`);
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
            }

            if (!matchUri) {
                console.log('[PPV] Match not found in live list');
                return [];
            }

            if (!matchData) {
                const simpleName = matchUri.split('/').pop()?.replace(/-/g, ' ') || 'PPV Stream';
                matchData = { name: simpleName };
            }

            // 1. Prefer direct modistreams fetch first (no browser startup).
            const withTimeout = (promise, ms) => Promise.race([
                promise,
                new Promise(resolve => setTimeout(() => resolve(null), ms))
            ]);
            let m3u8Url = await withTimeout(this.fetchModistreams(matchUri), 18000);

            // 2. Fallback to secure URL generation via PPV wasm path.
            if (!m3u8Url) {
                m3u8Url = await withTimeout(this.captureSecureM3u8ViaGasm(matchUri), 80000);
            }

            // 3. If still missing, try browser extraction from the details source.
            if (!m3u8Url) {
                const details = await this.getPPVStream(matchUri);
                if (details && details.sources) {
                    const iframeSource = details.sources.find(s => s.type === 'iframe');
                    if (iframeSource) {
                        console.log(`[PPV] Falling back to browser extraction: ${iframeSource.data.substring(0, 100)}...`);

                        // Most reliable path for PPV currently.
                        m3u8Url = await this.captureM3u8FromEmbed(iframeSource.data);

                        // Prefer shared Playwright resolver with correct referer for this ecosystem.
                        if (!m3u8Url) {
                            const resolved = await resolveWithPlaywright(iframeSource.data, 'https://modistreams.org/');
                            if (resolved && Array.isArray(resolved.streams) && resolved.streams.length > 0) {
                                m3u8Url = resolved.streams[0].url;
                            }
                        }

                        // Final fallback to legacy embed extractor.
                        if (!m3u8Url) {
                            m3u8Url = await this.scraper.extractor.extractM3U8FromEmbed(iframeSource.data);
                        }
                    }
                }
            }

            if (m3u8Url) {
                const headers = {
                    'Referer': 'https://modistreams.org/',
                    'Origin': 'https://modistreams.org',
                    'User-Agent': 'Mozilla/5.0 (iPhone; CPU iPhone OS 18_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.5 Mobile/15E148 Safari/604.1'
                };

                return [{
                    title: matchData ? matchData.name : 'PPV Stream',
                    url: m3u8Url,
                    provider: 'ppv',
                    source: 'modistreams',
                    isLive: true,
                    // Use URL-mode proxying to avoid oversized encrypted tokens in nested HLS playlists.
                    proxyMode: 'url',
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
