const swaggerDef = {
  openapi: '3.0.0',
  info: {
    title: 'Hue Stream API',
    version: '2.7.0',
    description: `
# 🎬 Hue Stream API: The Ultimate Streaming Engine

The **Hue Stream API** is a high-performance, aggregator for Movies, TV Shows, KDramas, and Anime. 

### 🚀 Key Capabilities:
- **Massive Discovery**: Dynamic endpoints for HiTV/SFlix with 30+ countries/genres and TMDB backdrop enrichment.
- **Smart Mapping**: Automatically bridges discovery metadata to high-quality KissKH (Zenith) and SFlix (Nova) streams.
- **Dedicated KDrama Hub**: Deep integration with KissKH and Dramacool databases.
- **HiAnime Integration**: Full anime catalog and multi-server stream extraction.
- **Unified Aggregation**: Single TMDB ID fetches from 10+ providers simultaneously.
- **Live Sports**: Extraction for AiSports, Livesport, LiveTV.sx, and PPV.to.
    `,
    contact: { name: 'Developer Support', email: 'dev@example.com' }
  },
  servers: [{ url: '/', description: 'Root API' }],
  tags: [
    { name: 'SFlix & HiTV Discovery', description: 'Massive, TMDB-enriched catalog for Asian content.' },
    { name: 'MovieBox Discovery', description: 'Discovery for MovieBox (Shorts, Drama, etc).' },
    { name: 'Smart Mapping & Playback', description: 'Bridging metadata to playable streams.' },
    { name: 'KDrama (KissKH)', description: 'Specialized KissKH browsing and extraction.' },
    { name: 'KDrama (Dramacool)', description: 'Specialized Dramacool browsing and extraction.' },
    { name: 'HiAnime (Catalog & Streams)', description: 'Full Anime ecosystem.' },
    { name: 'Aggregate', description: 'Unified Movies/Series aggregation via TMDB ID.' },
    { name: 'Sports', description: 'Live sports extraction and event flow.' },
    { name: 'System & Admin', description: 'Metrics, Health, and Config.' }
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
          releaseDate: { type: 'string' },
          genre: { type: 'string' },
          backdrop: { type: 'string' },
          overview: { type: 'string' },
          tmdbId: { type: 'integer' }
        }
      }
    }
  },
  paths: {
    /* HiTV & SFlix Discovery */
    '/api/sflix/{country}/{sort}/{genre}': {
      get: {
        tags: ['SFlix & HiTV Discovery'],
        summary: 'Massive Dynamic SFlix Discovery',
        parameters: [
          { name: 'country', in: 'path', required: true, schema: { type: 'string', default: 'All' } },
          { name: 'sort', in: 'path', schema: { type: 'string', enum: ['foryou', 'latest', 'hottest', 'trending', 'popular'] } },
          { name: 'genre', in: 'path', schema: { type: 'string', default: 'All' } },
          { name: 'page', in: 'query', schema: { type: 'integer', default: 1 } }
        ],
        responses: { 200: { description: 'Enriched list' } }
      }
    },
    '/api/sflix/latest': { get: { tags: ['SFlix & HiTV Discovery'], summary: 'SFlix Latest Releases', responses: { 200: { description: 'List' } } } },
    '/api/sflix/trending': { get: { tags: ['SFlix & HiTV Discovery'], summary: 'SFlix Trending', responses: { 200: { description: 'List' } } } },
    '/api/hitv/drama': { get: { tags: ['SFlix & HiTV Discovery'], summary: 'HiTV Drama Filter', parameters: [{ name: 'country', in: 'query' }, { name: 'genre', in: 'query' }, { name: 'year', in: 'query' }], responses: { 200: { description: 'List' } } } },
    '/api/hitv/latest': { get: { tags: ['SFlix & HiTV Discovery'], summary: 'HiTV Latest', responses: { 200: { description: 'List' } } } },
    '/api/hitv/trending': { get: { tags: ['SFlix & HiTV Discovery'], summary: 'HiTV Trending', responses: { 200: { description: 'List' } } } },
    '/api/hitv/hero': { get: { tags: ['SFlix & HiTV Discovery'], summary: 'Smart Hero Slate', responses: { 200: { description: 'Snapshot' } } } },
    '/api/hitv/library': { get: { tags: ['SFlix & HiTV Discovery'], summary: 'Full Library Cache', responses: { 200: { description: '3,000+ items' } } } },
    '/api/hitv/search': { get: { tags: ['SFlix & HiTV Discovery'], summary: 'Search HiTV', parameters: [{ name: 'q', in: 'query', required: true }], responses: { 200: { description: 'Results' } } } },
    '/api/hitv/info/{id}': { get: { tags: ['SFlix & HiTV Discovery'], summary: 'Get Show Details', parameters: [{ name: 'id', in: 'path', required: true }], responses: { 200: { description: 'Metadata' } } } },
    '/api/hitv/countries': { get: { tags: ['SFlix & HiTV Discovery'], summary: 'Get All Countries', responses: { 200: { description: 'List' } } } },
    '/api/hitv/genres': { get: { tags: ['SFlix & HiTV Discovery'], summary: 'Get All Genres', responses: { 200: { description: 'List' } } } },

    /* MovieBox Discovery */
    '/api/moviebox/home': { get: { tags: ['MovieBox Discovery'], summary: 'MovieBox Homepage', responses: { 200: { description: 'Snapshot' } } } },
    '/api/moviebox/discover': { get: { tags: ['MovieBox Discovery'], summary: 'Targeted Discovery (Shorts, KDrama, etc)', responses: { 200: { description: 'Filtered sections' } } } },
    '/api/moviebox/search': {
      get: {
        tags: ['MovieBox Discovery'],
        summary: 'Search MovieBox Content',
        parameters: [
          { name: 'keyword', in: 'query', required: true, schema: { type: 'string' } },
          { name: 'page', in: 'query', schema: { type: 'integer', default: 1 } },
          { name: 'type', in: 'query', schema: { type: 'integer', default: 0 }, description: '0=All, 1=Movie, 2=Series' }
        ],
        responses: { 200: { description: 'Search results' } }
      }
    },
    '/api/moviebox/bollywood': { 
      get: { 
        tags: ['MovieBox Discovery'], 
        summary: 'Bollywood & Indian Content', 
        parameters: [
          { name: 'type', in: 'query', schema: { type: 'string', enum: ['movie', 'tv'] }, description: 'Default: movie' },
          { name: 'page', in: 'query', schema: { type: 'integer' } }
        ],
        responses: { 200: { description: 'Indian content list' } } 
      } 
    },
    '/api/moviebox/shorts': { 
      get: { 
        tags: ['MovieBox Discovery'], 
        summary: 'Short TV Content', 
        parameters: [
          { name: 'page', in: 'query', schema: { type: 'integer' } }
        ],
        responses: { 200: { description: 'Short TV content list' } } 
      } 
    },
    '/api/moviebox/filter': { 
      get: { 
        tags: ['MovieBox Discovery'], 
        summary: 'Advanced Filter', 
        parameters: [
          { name: 'channelId', in: 'query', schema: { type: 'integer' }, description: '4=Shorts, 2=Drama, 1=Movie' },
          { name: 'genre', in: 'query' },
          { name: 'country', in: 'query' },
          { name: 'year', in: 'query' },
          { name: 'page', in: 'query' }
        ], 
        responses: { 200: { description: 'List' } } 
      } 
    },
    '/api/moviebox/info': { get: { tags: ['MovieBox Discovery'], summary: 'Details', parameters: [{ name: 'path', in: 'query', required: true }], responses: { 200: { description: 'Metadata' } } } },
    '/api/moviebox/play/{id}/{path}': { get: { tags: ['MovieBox Discovery'], summary: 'Extract Streams', parameters: [{ name: 'id', in: 'path' }, { name: 'path', in: 'path' }, { name: 'season', in: 'query' }, { name: 'episode', in: 'query' }], responses: { 200: { description: 'Streams' } } } },

    /* Smart Mapping & Playback */
    '/api/hitv/play/{id}/{episode}': {
      get: {
        tags: ['Smart Mapping & Playback'],
        summary: 'Smart Play (HiTV -> Stream)',
        parameters: [
          { name: 'id', in: 'path', required: true },
          { name: 'episode', in: 'path', required: true },
          { name: 'title', in: 'query', required: true, description: 'Exact title for accurate mapping' }
        ],
        responses: { 200: { description: 'Streams' } }
      }
    },

    /* KDrama (KissKH) */
    '/api/kdrama/search': { get: { tags: ['KDrama (KissKH)'], summary: 'Search KissKH', parameters: [{ name: 'q', in: 'query' }], responses: { 200: { description: 'Results' } } } },
    '/api/kdrama/list': { get: { tags: ['KDrama (KissKH)'], summary: 'Advanced Filter', parameters: [{ name: 'type', in: 'query' }, { name: 'country', in: 'query' }], responses: { 200: { description: 'List' } } } },
    '/api/kdrama/latest': { get: { tags: ['KDrama (KissKH)'], summary: 'Latest Episodes', responses: { 200: { description: 'List' } } } },
    '/api/kdrama/popular': { get: { tags: ['KDrama (KissKH)'], summary: 'Popular content', responses: { 200: { description: 'List' } } } },
    '/api/kdrama/detail/{id}': { get: { tags: ['KDrama (KissKH)'], summary: 'Show Details', parameters: [{ name: 'id', in: 'path', required: true }], responses: { 200: { description: 'Metadata' } } } },
    '/api/kdrama/streams/{id}/{episode}': { get: { tags: ['KDrama (KissKH)'], summary: 'Extract Streams', parameters: [{ name: 'id', in: 'path' }, { name: 'episode', in: 'path' }], responses: { 200: { description: 'Streams' } } } },

    /* KDrama (Dramacool) */
    '/api/dramacool/search': { get: { tags: ['KDrama (Dramacool)'], summary: 'Search Dramacool', parameters: [{ name: 'q', in: 'query' }], responses: { 200: { description: 'Results' } } } },
    '/api/dramacool/latest': { get: { tags: ['KDrama (Dramacool)'], summary: 'Latest Episodes', responses: { 200: { description: 'List' } } } },
    '/api/dramacool/home': { get: { tags: ['KDrama (Dramacool)'], summary: 'Home Page Data', responses: { 200: { description: 'Snapshot' } } } },
    '/api/dramacool/info': { get: { tags: ['KDrama (Dramacool)'], summary: 'Details', parameters: [{ name: 'id', in: 'query' }], responses: { 200: { description: 'Metadata' } } } },
    '/api/dramacool/browse/streams/{id}/{episode}': { get: { tags: ['KDrama (Dramacool)'], summary: 'Extract Streams', parameters: [{ name: 'id', in: 'path' }, { name: 'episode', in: 'path' }], responses: { 200: { description: 'Streams' } } } },

    /* HiAnime */
    '/api/v1/home': { get: { tags: ['HiAnime (Catalog & Streams)'], summary: 'HiAnime Homepage', responses: { 200: { description: 'Payload' } } } },
    '/api/v1/search': { get: { tags: ['HiAnime (Catalog & Streams)'], summary: 'Search Anime', parameters: [{ name: 'keyword', in: 'query' }], responses: { 200: { description: 'Results' } } } },
    '/api/v1/anime/{id}': { get: { tags: ['HiAnime (Catalog & Streams)'], summary: 'Anime Details', parameters: [{ name: 'id', in: 'path' }], responses: { 200: { description: 'Metadata' } } } },
    '/api/v1/episodes/{id}': { get: { tags: ['HiAnime (Catalog & Streams)'], summary: 'Episode List', parameters: [{ name: 'id', in: 'path' }], responses: { 200: { description: 'Episodes' } } } },
    '/api/v1/stream': { get: { tags: ['HiAnime (Catalog & Streams)'], summary: 'Extract Stream', parameters: [{ name: 'id', in: 'query' }, { name: 'server', in: 'query' }], responses: { 200: { description: 'Streams' } } } },
    '/api/streams/anime/{id}': { get: { tags: ['HiAnime (Catalog & Streams)'], summary: 'Unified Anime Stream', parameters: [{ name: 'id', in: 'path' }], responses: { 200: { description: 'Streams' } } } },

    /* Aggregate & Specialized */
    '/api/streams/{type}/{tmdbId}': {
      get: {
        tags: ['Aggregate'],
        summary: 'Unified Aggregator (Movies/Series)',
        parameters: [{ name: 'type', in: 'path' }, { name: 'tmdbId', in: 'path' }, { name: 'season', in: 'query' }, { name: 'episode', in: 'query' }],
        responses: { 200: { description: 'Streams' } }
      }
    },
    '/api/drama/{type}/{tmdbId}': { get: { tags: ['Aggregate'], summary: 'Drama Exclusive Aggregator', responses: { 200: { description: 'Streams' } } } },
    '/api/streams/{provider}/{type}/{tmdbId}': { get: { tags: ['Aggregate'], summary: 'Direct Provider Aggregator', responses: { 200: { description: 'Streams' } } } },

    /* Sports */
    '/api/sports-stream/{matchName}': { get: { tags: ['Sports'], summary: 'Unified Sports Stream (Livesport, LiveTV, PPV)', parameters: [{ name: 'matchName', in: 'path' }], responses: { 200: { description: 'Streams' } } } },
    '/api/livetv/{matchName}': { get: { tags: ['Sports'], summary: 'LiveTV Alias', parameters: [{ name: 'matchName', in: 'path' }], responses: { 200: { description: 'Streams' } } } },
    '/api/ppv/streams/{matchName}': { get: { tags: ['Sports'], summary: 'PPV Alias', parameters: [{ name: 'matchName', in: 'path' }], responses: { 200: { description: 'Streams' } } } },
    
    '/api/aisports/leagues': { get: { tags: ['Sports'], summary: 'Get AiSports Leagues', responses: { 200: { description: 'List' } } } },
    '/api/aisports/list/{leagueId}': { get: { tags: ['Sports'], summary: 'Get AiSports Matches', parameters: [{ name: 'leagueId', in: 'path' }, { name: 'status', in: 'query', description: '0=All, 1=Live, 2=Finished' }, { name: 'type', in: 'query', description: 'Sport type (e.g. football)' }], responses: { 200: { description: 'List' } } } },
    '/api/aisports/play/{id}': { get: { tags: ['Sports'], summary: 'Get AiSports Streams', parameters: [{ name: 'id', in: 'path' }], responses: { 200: { description: 'Streams' } } } },
    '/api/aisports/flow/{id}': { get: { tags: ['Sports'], summary: 'Get Match Flow/Commentary', parameters: [{ name: 'id', in: 'path' }, { name: 'pageSize', in: 'query' }], responses: { 200: { description: 'Flow Data' } } } },

    /* System */
    '/api/status': { get: { tags: ['System & Admin'], summary: 'Metrics & Status', responses: { 200: { description: 'Payload' } } } },
    '/api/health': { get: { tags: ['System & Admin'], summary: 'Health Check', responses: { 200: { description: 'OK' } } } },
    '/api/config': { get: { tags: ['System & Admin'], summary: 'Server Config (ReadOnly)', responses: { 200: { description: 'Config' } } } }
  }
};

module.exports = swaggerDef;
