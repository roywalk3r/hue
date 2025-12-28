// Minimal Deno/Edge-compatible handler.
// This build is intentionally limited: providers that depend on Node or Puppeteer are not available.
// It exposes health and providers metadata; stream endpoints return a clear "unsupported" response.

function json(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { 'content-type': 'application/json' }
  });
}

function unsupported(): Response {
  return json(
    {
      success: false,
      error: 'UNSUPPORTED_RUNTIME',
      message: 'Deno/Edge build is a limited mode (no Puppeteer/Node modules). Streams are unavailable; deploy the Node/Express version for full functionality.'
    },
    501
  );
}

async function handler(req: Request): Promise<Response> {
  const url = new URL(req.url);
  const path = url.pathname;

  if (path === '/api/health') {
    return json({
      success: true,
      runtime: 'deno',
      message: 'Limited Deno mode: streams disabled; use Node deployment for full providers.'
    });
  }

  if (path.startsWith('/api/providers')) {
    return json({
      success: true,
      providers: [],
      note: 'Providers are disabled in the Deno build because most require Node/Puppeteer. Use the Node deployment for streams.'
    });
  }

  if (path.startsWith('/api/streams')) {
    return unsupported();
  }

  return new Response(
    'TMDB Embed API (Deno limited build). Streams not available here; deploy the Node version for full functionality.',
    { status: 200, headers: { 'content-type': 'text/plain' } }
  );
}

addEventListener('fetch', (event: FetchEvent) => {
  event.respondWith(handler(event.request));
});
