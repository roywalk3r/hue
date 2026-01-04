/**
 * Update proxy configuration to use Cloudflare Worker
 */

// In your config, set:
const proxyConfig = {
    m3u8ProxyUrl: 'https://m3u8.testpjmail.workers.dev',
    enableProxy: true,
    // This will make all M3U8 URLs use your Cloudflare Worker
};

// Example usage:
// Original: https://gg.poocloud.in/pl_sun/tracks-v1a1/mono.ts.m3u8
// Proxied:  https://m3u8.testpjmail.workers.dev/pl_sun/tracks-v1a1/mono.ts.m3u8

console.log('Update your config to use the Cloudflare Worker URL for better performance!');
