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
  servers: [{ url: '/api', description: 'Main API' }],
  tags: [
    { name: 'SFlix & HiTV Discovery', description: 'Massive, TMDB-enriched catalog for Asian content.' },
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
    '/sflix/{country}/{sort}/{genre}': {
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
    '/sflix/latest': { get: { tags: ['SFlix & HiTV Discovery'], summary: 'SFlix Latest Releases', responses: { 200: { description: 'List' } } } },
    '/sflix/trending': { get: { tags: ['SFlix & HiTV Discovery'], summary: 'SFlix Trending', responses: { 200: { description: 'List' } } } },
    '/hitv/drama': { get: { tags: ['SFlix & HiTV Discovery'], summary: 'HiTV Drama Filter', parameters: [{ name: 'country', in: 'query' }, { name: 'genre', in: 'query' }, { name: 'year', in: 'query' }], responses: { 200: { description: 'List' } } } },
    '/hitv/latest': { get: { tags: ['SFlix & HiTV Discovery'], summary: 'HiTV Latest', responses: { 200: { description: 'List' } } } },
    '/hitv/trending': { get: { tags: ['SFlix & HiTV Discovery'], summary: 'HiTV Trending', responses: { 200: { description: 'List' } } } },
    '/hitv/hero': { get: { tags: ['SFlix & HiTV Discovery'], summary: 'Smart Hero Slate', responses: { 200: { description: 'Snapshot' } } } },
    '/hitv/library': { get: { tags: ['SFlix & HiTV Discovery'], summary: 'Full Library Cache', responses: { 200: { description: '3,000+ items' } } } },
    '/hitv/search': { get: { tags: ['SFlix & HiTV Discovery'], summary: 'Search HiTV', parameters: [{ name: 'q', in: 'query', required: true }], responses: { 200: { description: 'Results' } } } },
    '/hitv/info/{id}': { get: { tags: ['SFlix & HiTV Discovery'], summary: 'Get Show Details', parameters: [{ name: 'id', in: 'path', required: true }], responses: { 200: { description: 'Metadata' } } } },
    '/hitv/countries': { get: { tags: ['SFlix & HiTV Discovery'], summary: 'Get All Countries', responses: { 200: { description: 'List' } } } },
    '/hitv/genres': { get: { tags: ['SFlix & HiTV Discovery'], summary: 'Get All Genres', responses: { 200: { description: 'List' } } } },

    /* Smart Mapping & Playback */
    '/hitv/play/{id}/{episode}': {
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
    '/kdrama/search': { get: { tags: ['KDrama (KissKH)'], summary: 'Search KissKH', parameters: [{ name: 'q', in: 'query' }], responses: { 200: { description: 'Results' } } } },
    '/kdrama/list': { get: { tags: ['KDrama (KissKH)'], summary: 'Advanced Filter', parameters: [{ name: 'type', in: 'query' }, { name: 'country', in: 'query' }], responses: { 200: { description: 'List' } } } },
    '/kdrama/latest': { get: { tags: ['KDrama (KissKH)'], summary: 'Latest Episodes', responses: { 200: { description: 'List' } } } },
    '/kdrama/popular': { get: { tags: ['KDrama (KissKH)'], summary: 'Popular content', responses: { 200: { description: 'List' } } } },
    '/kdrama/detail/{id}': { get: { tags: ['KDrama (KissKH)'], summary: 'Show Details', parameters: [{ name: 'id', in: 'path', required: true }], responses: { 200: { description: 'Metadata' } } } },
    '/kdrama/streams/{id}/{episode}': { get: { tags: ['KDrama (KissKH)'], summary: 'Extract Streams', parameters: [{ name: 'id', in: 'path' }, { name: 'episode', in: 'path' }], responses: { 200: { description: 'Streams' } } } },

    /* KDrama (Dramacool) */
    '/dramacool/search': { get: { tags: ['KDrama (Dramacool)'], summary: 'Search Dramacool', parameters: [{ name: 'q', in: 'query' }], responses: { 200: { description: 'Results' } } } },
    '/dramacool/latest': { get: { tags: ['KDrama (Dramacool)'], summary: 'Latest Episodes', responses: { 200: { description: 'List' } } } },
    '/dramacool/home': { get: { tags: ['KDrama (Dramacool)'], summary: 'Home Page Data', responses: { 200: { description: 'Snapshot' } } } },
    '/dramacool/info': { get: { tags: ['KDrama (Dramacool)'], summary: 'Details', parameters: [{ name: 'id', in: 'query' }], responses: { 200: { description: 'Metadata' } } } },
    '/dramacool/browse/streams/{id}/{episode}': { get: { tags: ['KDrama (Dramacool)'], summary: 'Extract Streams', parameters: [{ name: 'id', in: 'path' }, { name: 'episode', in: 'path' }], responses: { 200: { description: 'Streams' } } } },

    /* HiAnime */
    '/v1/home': { get: { tags: ['HiAnime (Catalog & Streams)'], summary: 'HiAnime Homepage', responses: { 200: { description: 'Payload' } } } },
    '/v1/search': { get: { tags: ['HiAnime (Catalog & Streams)'], summary: 'Search Anime', parameters: [{ name: 'keyword', in: 'query' }], responses: { 200: { description: 'Results' } } } },
    '/v1/anime/{id}': { get: { tags: ['HiAnime (Catalog & Streams)'], summary: 'Anime Details', parameters: [{ name: 'id', in: 'path' }], responses: { 200: { description: 'Metadata' } } } },
    '/v1/episodes/{id}': { get: { tags: ['HiAnime (Catalog & Streams)'], summary: 'Episode List', parameters: [{ name: 'id', in: 'path' }], responses: { 200: { description: 'Episodes' } } } },
    '/v1/stream': { get: { tags: ['HiAnime (Catalog & Streams)'], summary: 'Extract Stream', parameters: [{ name: 'id', in: 'query' }, { name: 'server', in: 'query' }], responses: { 200: { description: 'Streams' } } } },
    '/streams/anime/{id}': { get: { tags: ['HiAnime (Catalog & Streams)'], summary: 'Unified Anime Stream', parameters: [{ name: 'id', in: 'path' }], responses: { 200: { description: 'Streams' } } } },

    /* Aggregate & Specialized */
    '/streams/{type}/{tmdbId}': {
      get: {
        tags: ['Aggregate'],
        summary: 'Unified Aggregator (Movies/Series)',
        parameters: [{ name: 'type', in: 'path' }, { name: 'tmdbId', in: 'path' }, { name: 'season', in: 'query' }, { name: 'episode', in: 'query' }],
        responses: { 200: { description: 'Streams' } }
      }
    },
    '/drama/{type}/{tmdbId}': { get: { tags: ['Aggregate'], summary: 'Drama Exclusive Aggregator', responses: { 200: { description: 'Streams' } } } },
    '/streams/{provider}/{type}/{tmdbId}': { get: { tags: ['Aggregate'], summary: 'Direct Provider Aggregator', responses: { 200: { description: 'Streams' } } } },

    /* Sports */
    '/sports-stream/{matchName}': { get: { tags: ['Sports'], summary: 'Unified Sports Stream (Livesport, LiveTV, PPV)', parameters: [{ name: 'matchName', in: 'path' }], responses: { 200: { description: 'Streams' } } } },
    '/livetv/{matchName}': { get: { tags: ['Sports'], summary: 'LiveTV Alias', parameters: [{ name: 'matchName', in: 'path' }], responses: { 200: { description: 'Streams' } } } },
    '/ppv/streams/{matchName}': { get: { tags: ['Sports'], summary: 'PPV Alias', parameters: [{ name: 'matchName', in: 'path' }], responses: { 200: { description: 'Streams' } } } },
    
    '/aisports/leagues': { get: { tags: ['Sports'], summary: 'Get AiSports Leagues', responses: { 200: { description: 'List' } } } },
    '/aisports/list/{leagueId}': { get: { tags: ['Sports'], summary: 'Get AiSports Matches', parameters: [{ name: 'leagueId', in: 'path' }, { name: 'status', in: 'query', description: '0=All, 1=Live, 2=Finished' }, { name: 'type', in: 'query', description: 'Sport type (e.g. football)' }], responses: { 200: { description: 'List' } } } },
    '/aisports/play/{id}': { get: { tags: ['Sports'], summary: 'Get AiSports Streams', parameters: [{ name: 'id', in: 'path' }], responses: { 200: { description: 'Streams' } } } },
    '/aisports/flow/{id}': { get: { tags: ['Sports'], summary: 'Get Match Flow/Commentary', parameters: [{ name: 'id', in: 'path' }, { name: 'pageSize', in: 'query' }], responses: { 200: { description: 'Flow Data' } } } },

    /* System */
    '/status': { get: { tags: ['System & Admin'], summary: 'Metrics & Status', responses: { 200: { description: 'Payload' } } } },
    '/health': { get: { tags: ['System & Admin'], summary: 'Health Check', responses: { 200: { description: 'OK' } } } },
    '/config': { get: { tags: ['System & Admin'], summary: 'Server Config (ReadOnly)', responses: { 200: { description: 'Config' } } } }
  }
};

module.exports = swaggerDef;
