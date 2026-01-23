# Livesport Stream Scraper

A Node.js project to extract M3U8 stream URLs from Livesport.su

## 📁 Project Structure

```
├── livesport-watch-scraper.js          # Main scraper for watch URLs
├── livesport-m3u8-extractor.js         # M3U8 extraction logic
├── livesport-api-scraper.js            # API-based scraper
├── m3u8-proxy.js                       # Basic proxy server
├── m3u8-proxy-v2.js                    # Improved proxy with caching
├── cloudflare-worker-proxy.js          # Cloudflare Worker for production
├── test-m3u8.js                        # M3U8 URL tester
└── test-livesport-api.js               # API endpoint tester
```

## 🚀 Quick Start

### 1. Install dependencies
```bash
npm install axios cheerio playwright playwright-extra puppeteer-extra-plugin-stealth
```

### 2. Scrape a watch URL
```bash
node livesport-watch-scraper.js https://livesport.su/watch/ppv-world-darts-championship
```

### 3. Start the proxy server
```bash
node m3u8-proxy-v2.js
```

### 4. Watch the stream
- Open: http://localhost:6000/test
- Or use M3U8 URL: http://localhost:6000/pl_sun/tracks-v1a1/mono.ts.m3u8

## 🌐 Cloudflare Deployment

### Deploy to Cloudflare Workers
1. Copy `cloudflare-worker-proxy.js`
2. Deploy at https://workers.cloudflare.com
3. Get your worker URL and use it directly

### Benefits of Cloudflare Worker
- ✅ Global CDN
- ✅ No server needed
- ✅ Free tier available
- ✅ Automatic HTTPS
- ✅ Edge caching

## 📖 Usage Examples

### Scrape Multiple Streams
```javascript
const LivesportWatchScraper = require('./livesport-watch-scraper');

const scraper = new LivesportWatchScraper();
const streams = await scraper.scrapeWatchUrl('https://livesport.su/watch/match-name');
```

### Test M3U8 URL
```bash
node test-m3u8.js
```

### API Usage
```javascript
const LivesportAPIScraper = require('./livesport-api-scraper');

const api = new LivesportAPIScraper();
const liveMatches = await api.getLiveMatches();
```

## 🔧 Configuration

### Environment Variables
```bash
# Optional: Custom proxy port
PORT=6000

# Optional: Custom headers
USER_AGENT="Mozilla/5.0..."
```

## 📝 Notes

- No Puppeteer required (uses HTTP requests)
- Works with mobile headers
- Handles rate limiting
- Supports live streams
- M3U8 URLs are cached for 2 seconds

## 🐛 Troubleshooting

### Proxy Issues
- Check if port 6000 is available
- Verify M3U8 URL is accessible
- Check network connectivity

### Scraper Issues
- Verify the watch URL format
- Check if the match is live
- Look for rate limiting messages

## 📄 License

MIT
