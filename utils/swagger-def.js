const swaggerDef = {
  openapi: '3.0.0',
  info: {
    title: 'Hue Stream API',
    version: '3.1.0',
    description: `
# 🎬 Hue Stream API: Professional Streaming Engine

Advanced aggregator for Movies, TV, Asian Drama, Anime, and Live Sports.

### 🚀 Highlights:
- **Regional Discovery**: Deep integration with HiTV, MovieBox, and SFlix for global content.
- **Smart Playback**: Automatic mapping from metadata to direct streaming URLs.
- **High-Performance Proxy**: Integrated segment rewriting for bypassing regional blocks.
- **Live Resolution**: Real-time extraction of sports HLS streams from hidden players.

### 🛠 Providers:
- **General**: Nova (SFlix), Aura (Vixsrc), Luna (Vidrock), Solar (Vidsrc-xyz), Stellar (Vidsrc-icu), Nebula (Vidsrc-embed).
- **Specialized**: Zenith (KissKH), Orient (Dramacool), Cine (Moviesmod), Ultra (UHDMovies), Prime (4KHDHub), Hydra (MP4Hydra).
- **Premium**: Echo (Showbox), Echo Pro (Showbox Playwright), Cinema (MovieBox).
- **Sports**: Stadium (Livesport), Field (AiSports), Relay (Streamed), Arena (LiveTV), Event (PPV).
    `,
    contact: { name: 'Developer Support' }
  },
  servers: [{ url: '/', description: 'Default Server' }],
  tags: [
    { name: 'Unified Aggregation', description: 'Universal stream extraction using TMDB IDs.' },
    { name: 'HiTV Discovery', description: 'Global KDrama/CDrama catalog and browsing.' },
    { name: 'SFlix Discovery', description: 'Regional content hub (HiTV-Powered).' },
    { name: 'MovieBox Discovery', description: 'Curated rows, search, and direct MP4 streams.' },
    { name: 'KDrama Hubs', description: 'Specialized Zenith (KissKH) and Orient (Dramacool) extractors.' },
    { name: 'Anime (HiAnime)', description: 'Anime catalog and multi-server stream resolution.' },
    { name: 'Live Sports', description: 'Real-time resolution for global sports and PPV events.' },
    { name: 'System & Metrics', description: 'Monitoring, cache management, and configuration.' }
  ],
  paths: {
    /* HiTV Discovery */
    '/api/hitv/browse': { 
      get: { 
        tags: ['HiTV Discovery'], 
        summary: 'Browse HiTV Library', 
        parameters: [
          { name: 'page', in: 'query', schema: { type: 'integer', default: 1 } },
          { name: 'country', in: 'query', schema: { type: 'string', default: 'Korea' } },
          { name: 'genre', in: 'query', schema: { type: 'string', default: 'All' } },
          { name: 'sort', in: 'query', schema: { type: 'string', default: 'Hottest' } }
        ],
        responses: { 200: { description: 'OK' } } 
      } 
    },
    '/api/hitv/play/{id}/{episode}': { 
      get: { 
        tags: ['HiTV Discovery'], 
        summary: 'Smart Play HiTV/SFlix Content', 
        parameters: [
          { name: 'id', in: 'path', required: true, schema: { type: 'string' }, description: 'HiTV ID or SFlix Slug' },
          { name: 'episode', in: 'path', required: true, schema: { type: 'integer' } },
          { name: 'title', in: 'query', schema: { type: 'string' } }
        ],
        responses: { 200: { description: 'OK' } } 
      } 
    },
    '/api/hitv/info/{id}': { 
      get: { 
        tags: ['HiTV Discovery'], 
        summary: 'Get Detailed Show Info', 
        parameters: [{ name: 'id', in: 'path', required: true, schema: { type: 'string' } }],
        responses: { 200: { description: 'OK' } } 
      } 
    },
    '/api/hitv/hero': { get: { tags: ['HiTV Discovery'], summary: 'Get Hero Slider Data', responses: { 200: { description: 'OK' } } } },

    /* SFlix Discovery */
    '/api/sflix/{country}/{sort}/{genre}': { 
      get: { 
        tags: ['SFlix Discovery'], 
        summary: 'Browse SFlix Regional Hub', 
        parameters: [
          { name: 'country', in: 'path', required: true, schema: { type: 'string' } },
          { name: 'sort', in: 'path', schema: { type: 'string' } },
          { name: 'genre', in: 'path', schema: { type: 'string' } },
          { name: 'page', in: 'query', schema: { type: 'integer' } }
        ],
        responses: { 200: { description: 'OK' } } 
      } 
    },

    /* MovieBox Discovery */
    '/api/moviebox/trending': { get: { tags: ['MovieBox Discovery'], summary: 'Get Global Trending Items', responses: { 200: { description: 'OK' } } } },
    '/api/moviebox/section': { 
      get: { 
        tags: ['MovieBox Discovery'], 
        summary: 'Extract Catalog Row', 
        parameters: [{ name: 'title', in: 'query', required: true, schema: { type: 'string' }, example: 'Popular Series' }],
        responses: { 200: { description: 'OK' } } 
      } 
    },
    '/api/moviebox/search': { get: { tags: ['MovieBox Discovery'], summary: 'Search Catalog', parameters: [{ name: 'keyword', in: 'query', required: true, schema: { type: 'string' } }], responses: { 200: { description: 'OK' } } } },
    '/api/moviebox/play/{id}/{path}': { 
      get: { 
        tags: ['MovieBox Discovery'], 
        summary: 'Extract Direct Streams', 
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
    '/api/kdrama/streams/{id}/{episode}': { 
      get: { 
        tags: ['KDrama Hubs'], 
        summary: 'Extract Zenith (KissKH) Streams', 
        parameters: [
          { name: 'id', in: 'path', required: true, schema: { type: 'string' } },
          { name: 'episode', in: 'path', required: true, schema: { type: 'integer' } }
        ],
        responses: { 200: { description: 'OK' } } 
      } 
    },
    '/api/dramacool/browse/streams/{id}/{episode}': { 
      get: { 
        tags: ['KDrama Hubs'], 
        summary: 'Extract Orient (Dramacool) Streams', 
        parameters: [
          { name: 'id', in: 'path', required: true, schema: { type: 'string' } },
          { name: 'episode', in: 'path', required: true, schema: { type: 'integer' } }
        ],
        responses: { 200: { description: 'OK' } } 
      } 
    },

    /* Unified Aggregation */
    '/api/streams/{type}/{tmdbId}': {
      get: {
        tags: ['Unified Aggregation'],
        summary: 'Aggregator (General Content)',
        parameters: [
          { name: 'type', in: 'path', required: true, schema: { type: 'string', enum: ['movie', 'series'] } },
          { name: 'tmdbId', in: 'path', required: true, schema: { type: 'string' } },
          { name: 'season', in: 'query', schema: { type: 'integer' } },
          { name: 'episode', in: 'query', schema: { type: 'integer' } },
          { name: 'nocache', in: 'query', schema: { type: 'integer', enum: [0, 1], default: 0 } }
        ],
        responses: { 200: { description: 'OK' } }
      }
    },
    '/api/drama/{type}/{tmdbId}': {
      get: {
        tags: ['Unified Aggregation'],
        summary: 'Aggregator (Drama Only)',
        parameters: [
          { name: 'type', in: 'path', required: true, schema: { type: 'string', enum: ['movie', 'series'] } },
          { name: 'tmdbId', in: 'path', required: true, schema: { type: 'string' } },
          { name: 'season', in: 'query', schema: { type: 'integer' } },
          { name: 'episode', in: 'query', schema: { type: 'integer' } }
        ],
        responses: { 200: { description: 'OK' } }
      }
    },

    /* Anime */
    '/api/streams/anime/{id}': {
      get: {
        tags: ['Anime (HiAnime)'],
        summary: 'Resolve HiAnime Stream',
        parameters: [
          { name: 'id', in: 'path', required: true, schema: { type: 'string' } },
          { name: 'server', in: 'query', schema: { type: 'string' } },
          { name: 'type', in: 'query', schema: { type: 'string', enum: ['sub', 'dub', 'raw'], default: 'sub' } }
        ],
        responses: { 200: { description: 'OK' } }
      }
    },

    /* Sports */
    '/api/aisports/list/{leagueId}': { 
      get: { 
        tags: ['Live Sports'], 
        summary: 'Match List (AiSports)', 
        parameters: [{ name: 'leagueId', in: 'path', required: true, schema: { type: 'integer' } }],
        responses: { 200: { description: 'OK' } } 
      } 
    },
    '/api/ppv/streams/{matchName}': { 
      get: { 
        tags: ['Live Sports'], 
        summary: 'Extract PPV.to Streams', 
        parameters: [{ name: 'matchName', in: 'path', required: true, schema: { type: 'string' } }],
        responses: { 200: { description: 'OK' } } 
      } 
    },
    '/sports-stream/{matchName}': { 
      get: { 
        tags: ['Live Sports'], 
        summary: 'Smart Sports Resolver (Multi-Source)', 
        parameters: [{ name: 'matchName', in: 'path', required: true, schema: { type: 'string' } }],
        responses: { 200: { description: 'OK' } } 
      } 
    },

    /* System */
    '/api/status': { get: { tags: ['System & Metrics'], summary: 'Get System Metrics', responses: { 200: { description: 'OK' } } } },
    '/api/health': { get: { tags: ['System & Metrics'], summary: 'Health Check', responses: { 200: { description: 'OK' } } } }
  },
  components: {
    schemas: {
      Stream: {
        type: 'object',
        properties: {
          title: { type: 'string', example: 'Nova - 1080p (HLS)' },
          url: { type: 'string', format: 'uri' },
          quality: { type: 'string' },
          provider: { type: 'string' }
        }
      }
    }
  }
};

module.exports = swaggerDef;
