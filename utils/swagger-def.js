const swaggerDef = {
  openapi: '3.0.0',
  info: {
    title: 'Hue Stream API',
    version: '2.5.0',
    description: `
# Unified Streaming API

The **Hue Stream API** is a high-performance aggregator for Movies, TV Shows, KDramas, and Anime. 

## 🚀 Key Features
- **Smart Aggregation**: Fetch from multiple sources with one ID.
- **Massive Discovery**: Powerful dynamic discovery for Asian content (HiTV/SFlix) with TMDB enrichment.
- **Dedicated Drama Hub**: Specialized endpoints for KissKH, Dramacool, and HiTV.
- **Smart Mapping**: Automatically maps discovery items to Zenith (KissKH) and Nova (SFlix) streams.
- **Anime Catalog**: Full HiAnime v1 catalog integration.
- **Proxy**: Built-in m3u8/ts proxy for bypassing CORS.
    `,
    contact: { name: 'Developer Support', email: 'dev@example.com' }
  },
  servers: [{ url: '/api', description: 'Main API' }],
  tags: [
    { name: 'HiTV & SFlix Discovery', description: 'Massive discovery powered by HiTV data with TMDB enrichment.' },
    { name: 'Drama Search', description: 'Keyword search across different drama providers.' },
    { name: 'KDrama (KissKH)', description: 'KissKH specialized browsing and extraction.' },
    { name: 'KDrama (Dramacool)', description: 'Dramacool specialized browsing and extraction.' },
    { name: 'Aggregate', description: 'Unified stream extraction using TMDB IDs.' },
    { name: 'Anime', description: 'HiAnime catalog and streams.' }
  ],
  components: {
    schemas: {
      DiscoveryItem: {
        type: 'object',
        properties: {
          id: { type: 'string' },
          title: { type: 'string' },
          cover: { type: 'string' },
          rating: { type: 'string' },
          releaseDate: { type: 'string', format: 'date' },
          genre: { type: 'string' },
          backdrop: { type: 'string' },
          tmdbId: { type: 'integer' },
          overview: { type: 'string' },
          isBackdrop: { type: 'boolean' }
        }
      }
    }
  },
  paths: {
    '/hitv/search': {
        get: {
            tags: ['Drama Search'],
            summary: 'Search HiTV Catalog',
            description: 'Search for series or movies in the HiTV database.',
            parameters: [
                { name: 'q', in: 'query', required: true, schema: { type: 'string' }, example: 'Lovely Runner' },
                { name: 'type', in: 'query', schema: { type: 'integer', default: 2 }, description: '1=Movie, 2=Series' }
            ],
            responses: { 200: { description: 'Search results' } }
        }
    },
    '/kdrama/search': {
        get: {
            tags: ['Drama Search'],
            summary: 'Search KissKH Catalog',
            parameters: [{ name: 'q', in: 'query', required: true, schema: { type: 'string' } }],
            responses: { 200: { description: 'Results' } }
        }
    },
    '/dramacool/search': {
        get: {
            tags: ['Drama Search'],
            summary: 'Search Dramacool Catalog',
            parameters: [{ name: 'q', in: 'query', required: true, schema: { type: 'string' } }],
            responses: { 200: { description: 'Results' } }
        }
    },
    '/sflix/{country}/{sort}/{genre}': {
        get: {
            tags: ['HiTV & SFlix Discovery'],
            summary: 'Massive Dynamic Discovery',
            parameters: [
                { name: 'country', in: 'path', required: true, schema: { type: 'string', default: 'All' } },
                { name: 'sort', in: 'path', schema: { type: 'string', enum: ['foryou', 'latest', 'hottest', 'trending', 'popular'], default: 'foryou' } },
                { name: 'genre', in: 'path', schema: { type: 'string', default: 'All' } },
                { name: 'page', in: 'query', schema: { type: 'integer', default: 1 } }
            ],
            responses: { 200: { description: 'Enriched list' } }
        }
    },
    '/hitv/drama': {
        get: {
            tags: ['HiTV & SFlix Discovery'],
            summary: 'Drama Discovery (TMDB Enriched)',
            parameters: [
                { name: 'page', in: 'query', schema: { type: 'integer', default: 1 } },
                { name: 'country', in: 'query', schema: { type: 'string', default: 'Korea' } },
                { name: 'genre', in: 'query', schema: { type: 'string', default: 'All' } },
                { name: 'sort', in: 'query', schema: { type: 'string', default: 'Hottest' } }
            ],
            responses: { 200: { description: 'Enriched list' } }
        }
    },
    '/hitv/play/{id}/{episode}': {
        get: {
            tags: ['HiTV & SFlix Discovery'],
            summary: 'Smart Play (HiTV -> Stream)',
            parameters: [
                { name: 'id', in: 'path', required: true },
                { name: 'episode', in: 'path', required: true },
                { name: 'title', in: 'query', required: true, schema: { type: 'string' } }
            ],
            responses: { 200: { description: 'Streams' } }
        }
    },
    '/hitv/hero': {
      get: { tags: ['HiTV & SFlix Discovery'], summary: 'Hero Slate', responses: { 200: { description: 'Enriched Hero items' } } }
    },
    '/hitv/library': {
      get: { tags: ['HiTV & SFlix Discovery'], summary: 'Massive Cached Library', responses: { 200: { description: 'Catalog' } } }
    },
    '/kdrama/list': {
        get: {
            tags: ['KDrama (KissKH)'],
            summary: 'Browse KissKH Library',
            parameters: [
                { name: 'page', in: 'query', schema: { type: 'integer', default: 1 } },
                { name: 'type', in: 'query', schema: { type: 'string' }, description: '0=All, 1=Drama, 2=Movie' }
            ],
            responses: { 200: { description: 'List' } }
        }
    },
    '/dramacool/latest': {
        get: { tags: ['KDrama (Dramacool)'], summary: 'Latest Episodes (Dramacool)', responses: { 200: { description: 'List' } } }
    },
    '/streams/{type}/{tmdbId}': {
      get: {
        tags: ['Aggregate'],
        summary: 'Unified Aggregator (Movies/Series)',
        parameters: [
          { name: 'type', in: 'path', required: true, schema: { type: 'string', enum: ['movie', 'series'] } },
          { name: 'tmdbId', in: 'path', required: true, schema: { type: 'string' } }
        ],
        responses: { 200: { description: 'Streams' } }
      }
    }
  }
};

module.exports = swaggerDef;