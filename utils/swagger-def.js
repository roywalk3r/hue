const swaggerDef = {
  openapi: '3.0.0',
  info: {
    title: 'Hue Stream API',
    version: '2.1.0',
    description: `
# Unified Streaming API

The **Hue Stream API** is a high-performance aggregator for Movies, TV Shows, and KDramas. It optimizes for low-resource environments by managing browser instances globally and caching results.

## 🚀 Key Features
- **Smart Aggregation**: Fetch from multiple sources with one ID.
- **Fast vs Slow**: Automatically prioritizes instant HTTP providers (Vixsrc, VidRock) over slow browser scrapers (SFlix, KissKH).
- **Dedicated Drama Endpoint**: Specialized /drama endpoint exclusively for Asian Dramas (KissKH, Dramacool) to avoid polluting main movie searches.
- **Hybrid Extraction**: Dramacool provider uses API for discovery and Browser for fail-safe stream extraction.
- **KDrama Hub**: Full browsing capabilities (Search, Popular, Upcoming) for KissKH and Dramacool.
- **Proxy**: Built-in m3u8 proxy for bypassing CORS.

## 🔒 Authentication
Currently public. Future versions may implement API Key protection.
    `,
    contact: {
      name: 'Developer Support',
      email: 'dev@example.com'
    }
  },
  servers: [
    {
      url: '/api',
      description: 'Main API'
    }
  ],
  tags: [
    { name: 'Aggregate', description: 'Unified stream extraction using TMDB IDs.' },
    { name: 'Drama', description: 'Specialized endpoint for KDrama/Asian content.' },
    { name: 'Direct Provider', description: 'Extract streams from a specific provider.' },
    { name: 'KDrama (KissKH)', description: 'KissKH browsing and extraction.' },
    { name: 'KDrama (Dramacool)', description: 'Dramacool browsing and extraction.' },
    { name: 'Sports', description: 'Live sports streaming.' },
    { name: 'System', description: 'Health and Metrics.' }
  ],
  components: {
    schemas: {
      Stream: {
        type: 'object',
        properties: {
          name: { type: 'string', example: 'Hianime 1080p' },
          url: { type: 'string', description: 'Playback URL' },
          quality: { type: 'string', example: '1080p' },
          provider: { type: 'string', example: 'Hianime' },
          headers: { type: 'object', description: 'Required headers (Referer, User-Agent)' }
        }
      },
      DramaItem: {
        type: 'object',
        properties: {
          id: { type: 'string' },
          title: { type: 'string' },
          image: { type: 'string' },
          episodes: { type: 'integer' }
        }
      }
    }
  },
  paths: {
    '/streams/{type}/{tmdbId}': {
      get: {
        tags: ['Aggregate'],
        summary: 'Get Unified Streams (Movies/Series)',
        description: 'Aggregates streams from general providers (Vixsrc, SFlix, VidRock, etc.) for a TMDB ID. Excludes drama-specific providers for performance.',
        parameters: [
          { name: 'type', in: 'path', required: true, schema: { type: 'string', enum: ['movie', 'series'] } },
          { name: 'tmdbId', in: 'path', required: true, schema: { type: 'string' } },
          { name: 'season', in: 'query', schema: { type: 'integer' } },
          { name: 'episode', in: 'query', schema: { type: 'integer' } }
        ],
        responses: { 200: { description: 'List of streams' } }
      }
    },
    '/drama/{type}/{tmdbId}': {
      get: {
        tags: ['Drama'],
        summary: 'Get Drama Streams',
        description: 'Exclusive endpoint for fetching streams from Asian Drama providers (KissKH, Dramacool).',
        parameters: [
          { name: 'type', in: 'path', required: true, schema: { type: 'string', enum: ['tv', 'movie'] } },
          { name: 'tmdbId', in: 'path', required: true, schema: { type: 'string' } },
          { name: 'season', in: 'query', schema: { type: 'integer' } },
          { name: 'episode', in: 'query', schema: { type: 'integer' } }
        ],
        responses: { 200: { description: 'List of drama streams' } }
      }
    },
    '/streams/{provider}/{type}/{tmdbId}': {
      get: {
        tags: ['Direct Provider'],
        summary: 'Get Provider Streams',
        description: 'Extract streams from a specific provider (e.g., hianime, vixsrc, sflix).',
        parameters: [
          { name: 'provider', in: 'path', required: true, schema: { type: 'string', enum: ['hianime', 'vixsrc', 'sflix', 'kisskh'] } },
          { name: 'type', in: 'path', required: true, schema: { type: 'string', enum: ['movie', 'series'] } },
          { name: 'tmdbId', in: 'path', required: true, schema: { type: 'string' } },
          { name: 'season', in: 'query', schema: { type: 'integer' } },
          { name: 'episode', in: 'query', schema: { type: 'integer' } }
        ],
        responses: { 200: { description: 'List of streams from specific provider' } }
      }
    },
    '/kdrama/search': {
      get: {
        tags: ['KDrama (KissKH)'],
        summary: 'Search KissKH',
        parameters: [{ name: 'q', in: 'query', required: true, schema: { type: 'string' } }],
        responses: { 200: { description: 'Search results' } }
      }
    },
    '/kdrama/list': {
      get: {
        tags: ['KDrama (KissKH)'],
        summary: 'Browse KissKH Library',
        description: 'Advanced filtering for the KissKH library.',
        parameters: [
          { name: 'page', in: 'query', schema: { type: 'integer', default: 1 } },
          { name: 'type', in: 'query', schema: { type: 'string' }, description: '0=All, 1=Drama, 2=Movie, 3=Show' },
          { name: 'order', in: 'query', schema: { type: 'string', enum: ['latest', 'popular', 'release'] } },
          { name: 'country', in: 'query', schema: { type: 'string', enum: ['kr', 'cn', 'jp', 'th', 'ph'] } },
          { name: 'status', in: 'query', schema: { type: 'string', enum: ['ongoing', 'completed'] } }
        ],
        responses: { 200: { description: 'Filtered list' } }
      }
    },
    '/kdrama/latest': {
        get: {
            tags: ['KDrama (KissKH)'],
            summary: 'Latest Episodes (KissKH)',
            description: 'Shortcut for /list?order=latest',
            parameters: [{ name: 'page', in: 'query', schema: { type: 'integer' } }],
            responses: { 200: { description: 'Latest releases' } }
        }
    },
    '/kdrama/popular': {
        get: {
            tags: ['KDrama (KissKH)'],
            summary: 'Popular Dramas (KissKH)',
            description: 'Shortcut for /list?order=popular',
            parameters: [{ name: 'page', in: 'query', schema: { type: 'integer' } }],
            responses: { 200: { description: 'Popular content' } }
        }
    },
    '/kdrama/upcoming': {
        get: {
            tags: ['KDrama (KissKH)'],
            summary: 'Upcoming Dramas',
            responses: { 200: { description: 'List of upcoming dramas' } }
        }
    },
    '/kdrama/detail/{id}': {
        get: {
            tags: ['KDrama (KissKH)'],
            summary: 'Drama Details',
            parameters: [{ name: 'id', in: 'path', required: true, schema: { type: 'string' } }],
            responses: { 200: { description: 'Metadata and episodes' } }
        }
    },
    '/kdrama/streams/{id}/{episode}': {
        get: {
            tags: ['KDrama (KissKH)'],
            summary: 'Extract Stream (KissKH)',
            parameters: [
                { name: 'id', in: 'path', required: true, description: 'KissKH ID' },
                { name: 'episode', in: 'path', required: true, description: 'Episode number' }
            ],
            responses: { 200: { description: 'Playable streams' } }
        }
    },
    '/dramacool/search': {
        get: {
            tags: ['KDrama (Dramacool)'],
            summary: 'Search Dramacool',
            parameters: [{ name: 'q', in: 'query', required: true }, { name: 'page', in: 'query' }],
            responses: { 200: { description: 'Search results' } }
        }
    },
    '/dramacool/latest': {
        get: {
            tags: ['KDrama (Dramacool)'],
            summary: 'Latest Episodes (Dramacool)',
            parameters: [{ name: 'page', in: 'query' }],
            responses: { 200: { description: 'Latest releases' } }
        }
    },
    '/dramacool/latest-kdrama': {
        get: {
            tags: ['KDrama (Dramacool)'],
            summary: 'Latest K-Dramas',
            parameters: [{ name: 'page', in: 'query' }],
            responses: { 200: { description: 'Latest Korean content' } }
        }
    },
    '/dramacool/popular': {
        get: {
            tags: ['KDrama (Dramacool)'],
            summary: 'Popular Dramas',
            parameters: [{ name: 'page', in: 'query' }],
            responses: { 200: { description: 'Popular content' } }
        }
    },
    '/dramacool/ongoing': {
        get: {
            tags: ['KDrama (Dramacool)'],
            summary: 'Ongoing Dramas',
            parameters: [{ name: 'page', in: 'query' }],
            responses: { 200: { description: 'Current airings' } }
        }
    },
    '/dramacool/upcoming': {
        get: {
            tags: ['KDrama (Dramacool)'],
            summary: 'Upcoming Dramas',
            parameters: [{ name: 'page', in: 'query' }],
            responses: { 200: { description: 'Future releases' } }
        }
    },
    '/dramacool/home': {
        get: {
            tags: ['KDrama (Dramacool)'],
            summary: 'Home Page Data',
            description: 'Get featured sliders and latest content from Dramacool home.',
            responses: { 200: { description: 'Home data' } }
        }
    },
    '/dramacool/info': {
        get: {
            tags: ['KDrama (Dramacool)'],
            summary: 'Drama Details',
            parameters: [{ name: 'id', in: 'query', required: true, description: 'Dramacool Slug ID' }],
            responses: { 200: { description: 'Metadata' } }
        }
    },
    '/dramacool/browse/streams/{id}/{episode}': {
        get: {
            tags: ['KDrama (Dramacool)'],
            summary: 'Extract Stream (Dramacool)',
            parameters: [
                { name: 'id', in: 'path', required: true, description: 'Drama Slug' },
                { name: 'episode', in: 'path', required: true, description: 'Episode number' }
            ],
            responses: { 200: { description: 'Playable streams' } }
        }
    },
    '/sports-stream/{matchName}': {
        get: {
            tags: ['Sports'],
            summary: 'Extract Sports Stream',
            parameters: [{ name: 'matchName', in: 'path', required: true, description: 'Match slug/path' }],
            responses: { 200: { description: 'Live streams' } }
        }
    },
    '/status': {
      get: {
        tags: ['System'],
        summary: 'System Status',
        responses: { 200: { description: 'Full system status (metrics, providers, endpoints)' } }
      }
    }
  }
};

module.exports = swaggerDef;
