const { getNewPage } = require("../utils/browserManager");

const UA = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36';

/**
 * Vidsrc-Embed Provider (Browser-based)
 * Uses Playwright to solve Cloudflare Turnstile and capture stream links.
 */
async function getStreams(tmdbId, mediaType, season, episode) {
  const embedUrl = (mediaType === 'tv' || mediaType === 'series')
    ? `https://vidsrc-embed.ru/embed/tv/${tmdbId}/${season}/${episode}`
    : `https://vidsrc-embed.ru/embed/movie/${tmdbId}`;

  console.log(`[VidsrcEmbed] Scraping via Browser: ${embedUrl}`);
  
  let page;
  try {
    // Acquire a page from the global manager (handles concurrency limits)
    page = await getNewPage({ browserType: 'firefox' });
    
    let hlsUrl = null;
    let hlsHeaders = {};
    const subtitles = [];

    // Capture the M3U8 from network traffic
    page.on('request', req => {
      const u = req.url();
      // Look for m3u8 and ignore the domain-placeholder versions ({v1}, etc)
      if (u.includes('.m3u8') && !u.includes('{v')) {
        if (!hlsUrl) {
            hlsUrl = u;
            hlsHeaders = req.headers(); // Capture actual headers used
            console.log(`[VidsrcEmbed] Captured HLS: ${u.substring(0, 60)}...`);
        }
      }
      if ((u.includes('.vtt') || u.includes('.srt')) && !subtitles.some(s => s.url === u)) {
        subtitles.push({ url: u, lang: 'en', label: 'English' });
      }
    });

    // Navigate and wait for network activity to settle
    await page.goto(embedUrl, { waitUntil: 'domcontentloaded', timeout: 30000 });

    // Wait up to 15 seconds for the player to initialize and hlsUrl to be captured
    const start = Date.now();
    while (!hlsUrl && Date.now() - start < 15000) {
      await new Promise(r => setTimeout(r, 1000));
      
      // Attempt to click play buttons if they appear in iframes
      const frames = page.frames();
      console.log(`[VidsrcEmbed] Scanning ${frames.length} frames...`);
      
      for (const frame of frames) {
        try {
            const clicked = await frame.evaluate(() => {
                const selectors = ['#pl_but', '.play-btn', '.vjs-big-play-button', '#player_iframe'];
                for (const s of selectors) {
                    const btn = document.querySelector(s);
                    if (btn) {
                        btn.click();
                        return true;
                    }
                }
                return false;
            });
            if (clicked) console.log(`[VidsrcEmbed] Clicked play button in frame: ${frame.url()}`);
        } catch (e) {}
      }
    }

    if (hlsUrl) {
      // Clean headers (remove pseudo-headers starting with :)
      const cleanHeaders = {};
      for (const [k, v] of Object.entries(hlsHeaders)) {
          if (!k.startsWith(':')) cleanHeaders[k] = v;
      }
      
      // Ensure User-Agent is present
      if (!cleanHeaders['user-agent']) cleanHeaders['user-agent'] = UA;
      // Ensure Referer is present (default to cloudnestra if missing)
      if (!cleanHeaders['referer']) cleanHeaders['referer'] = "https://cloudnestra.com/";

      return [{
        url: hlsUrl,
        quality: "Auto",
        provider: "VidsrcEmbed",
        subtitles: subtitles.length > 0 ? subtitles : undefined,
        headers: cleanHeaders
      }];
    }
  } catch (error) {
    console.error(`[VidsrcEmbed] Browser error: ${error.message}`);
  } finally {
    // Ensure the page and context are closed to release the slot in BrowserManager
    if (page) await page.context().close();
  }
  return [];
}

module.exports = { getStreams };