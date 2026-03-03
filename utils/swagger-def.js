const swaggerDef = {
  openapi: '3.0.0',
  info: {
    title: 'Hue Stream API',
    version: '2.8.1',
    description: `
# 🎬 Hue Stream API: Professional Streaming Engine

Advanced aggregator for Movies, TV, Asian Drama, Anime, and Live Sports.

### 🚀 Highlights:
- **Regional Discovery**: Deep integration with HiTV and MovieBox for global content.
- **Smart Playback**: Automatic mapping from metadata to direct streaming URLs.
- **High-Performance Proxy**: Integrated segment rewriting for bypassing regional blocks.
- **Live Resolution**: Real-time extraction of sports HLS streams from hidden players.
    `,
    contact: { name: 'Developer Support' }
  },
  servers: [{ url: '/', description: 'Default Server' }],
  tags: [
    { name: 'Asian Content', description: 'HiTV, SFlix, and MovieBox Regional discovery.' },
    { name: 'Anime', description: 'HiAnime stream resolution and catalog endpoints.' },
    { name: 'KDrama Hubs', description: 'Specialized KissKH and Dramacool extractors.' },
    { name: 'Aggregation', description: 'Unified multi-source extraction via TMDB ID.' },
    { name: 'Sports', description: 'Real-time resolution for global sports events.' },
    { name: 'System', description: 'Health and performance metrics.' }
  ],
  paths: {
    /* Regional Discovery */
    '/api/moviebox/section': { 
      get: { 
        tags: ['Asian Content'], 
        summary: 'Extract Homepage Row', 
        parameters: [{ name: 'title', in: 'query', required: true, schema: { type: 'string' }, example: 'Trending Now' }],
        responses: { 200: { description: 'OK' } } 
      } 
    },
    '/api/moviebox/hero': { get: { tags: ['Asian Content'], summary: 'Curated Slider Data', responses: { 200: { description: 'OK' } } } },
    '/api/moviebox/sa': { get: { tags: ['Asian Content'], summary: 'SA Drama Row', responses: { 200: { description: 'OK' } } } },
    '/api/moviebox/thai': { get: { tags: ['Asian Content'], summary: 'Thai Drama Row', responses: { 200: { description: 'OK' } } } },
    '/api/moviebox/turkish': { get: { tags: ['Asian Content'], summary: 'Turkish Drama Row', responses: { 200: { description: 'OK' } } } },
    '/api/moviebox/western': { get: { tags: ['Asian Content'], summary: 'Western TV Row', responses: { 200: { description: 'OK' } } } },
    '/api/moviebox/animation': { get: { tags: ['Asian Content'], summary: 'Animation Collection', responses: { 200: { description: 'OK' } } } },
    '/api/moviebox/black-shows': { get: { tags: ['Asian Content'], summary: 'Black Content Row', responses: { 200: { description: 'OK' } } } },
    '/api/moviebox/shorts': { get: { tags: ['Asian Content'], summary: 'Short TV & Mini-Series', parameters: [{ name: 'page', in: 'query', schema: { type: 'integer' } }], responses: { 200: { description: 'OK' } } } },
    '/api/moviebox/bollywood': { get: { tags: ['Asian Content'], summary: 'Indian/Bollywood Content', parameters: [{ name: 'page', in: 'query', schema: { type: 'integer' } }], responses: { 200: { description: 'OK' } } } },
    '/api/moviebox/search': { get: { tags: ['Asian Content'], summary: 'Search MovieBox', parameters: [{ name: 'keyword', in: 'query', required: true, schema: { type: 'string' } }], responses: { 200: { description: 'OK' } } } },
    '/api/moviebox/play/{id}/{path}': { 
      get: { 
        tags: ['Asian Content'], 
        summary: 'Extract MovieBox Streams', 
        parameters: [
          { name: 'id', in: 'path', required: true, schema: { type: 'string' } },
          { name: 'path', in: 'path', required: true, schema: { type: 'string' } },
          { name: 'season', in: 'query', schema: { type: 'integer' } },
          { name: 'episode', in: 'query', schema: { type: 'integer' } }
        ], 
        responses: { 200: { description: 'OK' } } 
      } 
    },

    /* KDrama Hubs */
    '/api/kdrama/latest': { get: { tags: ['KDrama Hubs'], summary: 'KissKH Latest Releases', responses: { 200: { description: 'OK' } } } },
    '/api/kdrama/streams/{id}/{episode}': { 
      get: { 
        tags: ['KDrama Hubs'], 
        summary: 'Extract KissKH Streams', 
        parameters: [
          { name: 'id', in: 'path', required: true, schema: { type: 'string' } },
          { name: 'episode', in: 'path', required: true, schema: { type: 'integer' } }
        ],
        responses: { 200: { description: 'OK' } } 
      } 
    },
    '/api/dramacool/latest': { get: { tags: ['KDrama Hubs'], summary: 'Dramacool Latest Releases', responses: { 200: { description: 'OK' } } } },
    '/api/dramacool/browse/streams/{id}/{episode}': { 
      get: { 
        tags: ['KDrama Hubs'], 
        summary: 'Extract Dramacool Streams', 
        parameters: [
          { name: 'id', in: 'path', required: true, schema: { type: 'string' } },
          { name: 'episode', in: 'path', required: true, schema: { type: 'integer' } }
        ],
        responses: { 200: { description: 'OK' } } 
      } 
    },

    /* Aggregation */
    '/api/streams/anime/{id}': {
      get: {
        tags: ['Anime'],
        summary: 'Resolve HiAnime episode streams',
        parameters: [
          { name: 'id', in: 'path', required: true, schema: { type: 'string' }, description: 'HiAnime episode identifier' },
          { name: 'server', in: 'query', schema: { type: 'string' }, description: 'Preferred server name such as hd-1' },
          { name: 'type', in: 'query', schema: { type: 'string', enum: ['sub', 'dub', 'raw'], default: 'sub' } },
          { name: 'serversOnly', in: 'query', schema: { type: 'boolean' }, description: 'Return available servers without resolving a stream' }
        ],
        responses: { 200: { description: 'OK' } }
      }
    },
    '/api/streams/{type}/{tmdbId}': {
      get: {
        tags: ['Aggregation'],
        summary: 'Unified Aggregator (TMDB ID)',
        parameters: [
          { name: 'type', in: 'path', required: true, schema: { type: 'string', enum: ['movie', 'series'] } },
          { name: 'tmdbId', in: 'path', required: true, schema: { type: 'string' } },
          { name: 'season', in: 'query', schema: { type: 'integer' } },
          { name: 'episode', in: 'query', schema: { type: 'integer' } }
        ],
        responses: { 200: { description: 'OK' } }
      }
    },

    /* Sports */
    '/api/aisports/list/{leagueId}': { 
      get: { 
        tags: ['Sports'], 
        summary: 'Match List (Live/Upcoming)', 
        parameters: [
          { name: 'leagueId', in: 'path', required: true, schema: { type: 'integer' }, example: 0 },
          { name: 'status', in: 'query', schema: { type: 'integer' }, description: '0=All, 1=Live, 2=Upcoming' }
        ],
        responses: { 200: { description: 'OK' } } 
      } 
    },
    '/api/aisports/play/{id}': { 
      get: { 
        tags: ['Sports'], 
        summary: 'Extract Live Streams', 
        parameters: [{ name: 'id', in: 'path', required: true, schema: { type: 'string' } }],
        responses: { 200: { description: 'OK' } } 
      } 
    },
    '/api/sports-stream/{matchName}': { 
      get: { 
        tags: ['Sports'], 
        summary: 'Smart Sports Resolver', 
        parameters: [{ name: 'matchName', in: 'path', required: true, schema: { type: 'string' } }],
        responses: { 200: { description: 'OK' } } 
      } 
    },

    /* System */
    '/api/status': { get: { tags: ['System'], summary: 'Server Status & Metrics', responses: { 200: { description: 'OK' } } } },
    '/api/health': { get: { tags: ['System'], summary: 'Health Check', responses: { 200: { description: 'OK' } } } }
  }
};

module.exports = swaggerDef;
