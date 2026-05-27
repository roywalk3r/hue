/**
 * Cloudflare Worker proxy for MovieBox streaming API
 * Deploy at: https://dash.cloudflare.com → Workers & Pages → Create Service
 *
 * Usage: Set MOVIEBOX_PROXY_URL=https://your-worker.your-subdomain.workers.dev
 */

export default {
  async fetch(request, env) {
    const url = new URL(request.url);

    // Extract target URL from query param or path
    const target = url.searchParams.get('target') || 'https://movibox.net';

    // Build forward URL preserving the original path and query
    const targetUrl = new URL(request.url);
    targetUrl.hostname = new URL(target).hostname;
    targetUrl.protocol = 'https:';

    // Clone request headers and add CORS
    const headers = new Headers(request.headers);
    headers.set('Origin', 'https://moviebox.id');
    headers.set('Referer', 'https://moviebox.id/');

    // Forward the request
    const response = await fetch(targetUrl.toString(), {
      method: request.method,
      headers: headers,
      body: request.body,
    });

    // Return response with CORS headers
    const corsHeaders = new Headers(response.headers);
    corsHeaders.set('Access-Control-Allow-Origin', '*');
    corsHeaders.set('Access-Control-Allow-Methods', 'GET, POST, OPTIONS');

    return new Response(response.body, {
      status: response.status,
      statusText: response.statusText,
      headers: corsHeaders,
    });
  },
};
