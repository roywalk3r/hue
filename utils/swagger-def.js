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
- **HiTV Discovery**: New robust discovery system for Asian Dramas with rich metadata and filtering.
- **Smart Mapping**: Automatically maps HiTV discovery items to Zenith (KissKH) and Nova (SFlix) streams.
- **Proxy**: Built-in m3u8 proxy for bypassing CORS with IPv4 failover.

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
    { name: 'HiTV (Discovery)', description: 'Discovery, Browse, and Smart Play for Asian content.' },
    { name: 'Drama', description: 'Specialized endpoint for KDrama/Asian content.' },
    { name: 'Direct Provider', description: 'Extract streams from a specific provider.' },
    { name: 'KDrama (KissKH)', description: 'KissKH browsing and extraction.' },
    { name: 'KDrama (Dramacool)', description: 'Dramacool browsing and extraction.' },
    { name: 'Sports', description: 'Live sports streaming.' },
    { name: 'System', description: 'Health and Metrics.' }
  ],
  paths: {
    '/hitv/browse': {
        get: {
            tags: ['HiTV (Discovery)'],
            summary: 'Browse KDramas/Movies',
            description: 'Discover Asian content with rich metadata, filtering, and sorting.',
            parameters: [
                { name: 'page', in: 'query', schema: { type: 'integer', default: 1 } },
                { name: 'country', in: 'query', schema: { type: 'string', default: 'Korea' } },
                { name: 'category', in: 'query', schema: { type: 'string', default: 'Drama' } },
                { name: 'genre', in: 'query', schema: { type: 'string', default: 'All' } },
                { name: 'sort', in: 'query', schema: { type: 'string', default: 'Hottest' } }
            ],
            responses: { 200: { description: 'List of shows' } }
        }
    },
    '/hitv/play/{id}/{episode}': {
        get: {
            tags: ['HiTV (Discovery)'],
            summary: 'Smart Play (HiTV -> Stream)',
            description: 'Maps a HiTV ID to a stream from Zenith (KissKH) or Nova (SFlix).',
            parameters: [
                { name: 'id', in: 'path', required: true, schema: { type: 'string' } },
                { name: 'episode', in: 'path', required: true, schema: { type: 'integer' } },
                { name: 'title', in: 'query', required: true, schema: { type: 'string' }, description: 'The exact title of the show (from browse) for accurate mapping' }
            ],
            responses: { 200: { description: 'Playable streams and subtitles' } }
        }
    },
    '/hitv/search': {
        get: {
            tags: ['HiTV (Discovery)'],
            summary: 'Search HiTV',
            parameters: [
                { name: 'q', in: 'query', required: true, schema: { type: 'string' } },
                { name: 'type', in: 'query', schema: { type: 'integer', default: 2 }, description: '1=Movie, 2=Series' }
            ],
            responses: { 200: { description: 'Search results' } }
        }
    },
    '/hitv/info/{id}': {
        get: {
            tags: ['HiTV (Discovery)'],
            summary: 'Show Details',
            parameters: [{ name: 'id', in: 'path', required: true, schema: { type: 'string' } }],
            responses: { 200: { description: 'Metadata and episodes' } }
        }
    },
    '/hitv/trending': {
        get: { tags: ['HiTV (Discovery)'], summary: 'Trending Shows', responses: { 200: { description: 'Trending list' } } }
    },
    '/hitv/latest': {
        get: { tags: ['HiTV (Discovery)'], summary: 'Latest Releases', responses: { 200: { description: 'New releases list' } } }
    },
    '/hitv/countries': { get: { tags: ['HiTV (Discovery)'], summary: 'Get Countries', responses: { 200: { description: 'List of countries' } } } },
    '/hitv/categories': { get: { tags: ['HiTV (Discovery)'], summary: 'Get Categories', responses: { 200: { description: 'List of categories' } } } },
    '/hitv/genres': { get: { tags: ['HiTV (Discovery)'], summary: 'Get Genres', responses: { 200: { description: 'List of genres' } } } },
    '/hitv/sorts': { get: { tags: ['HiTV (Discovery)'], summary: 'Get Sort Options', responses: { 200: { description: 'List of sort options' } } } },
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
            summary: 'Extract Sports Stream from Livesport.su',
            parameters: [{ name: 'matchName', in: 'path', required: true, description: 'Match slug/path' }],
            responses: { 200: { description: 'Live streams' } }
        }
    },
    '/livetv/{matchName}': {
        get: {
            tags: ['Sports'],
            summary: 'Extract Sports Stream from LiveTV.sx',
            parameters: [{ name: 'matchName', in: 'path', required: true, description: 'Match slug/path' }],
            responses: { 200: { description: 'Live streams' } }
        }
    },
    '/livetv': {
        get: {
            tags: ['Sports'],
            summary: 'Get LiveTV Sports and Matches',
            parameters: [
                { name: 'sportId', in: 'query', schema: { type: 'string' }, description: 'Filter by sport ID' }
            ],
            responses: { 200: { description: 'List of sports and matches' } }
        }
    },
    '/livetv/sports': {
        get: {
            tags: ['Sports'],
            summary: 'Get Available Sports Categories',
            responses: { 200: { description: 'List of sports' } }
        }
    },
    '/livetv/matches': {
        get: {
            tags: ['Sports'],
            summary: 'Get LiveTV Matches',
            parameters: [
                { name: 'sportId', in: 'query', schema: { type: 'string' }, description: 'Filter by sport ID' }
            ],
            responses: { 200: { description: 'List of matches' } }
        }
    },
    '/livetv/match': {
        get: {
            tags: ['Sports'],
            summary: 'Get Stream Variants for a Match',
            parameters: [
                { name: 'url', in: 'query', required: true, description: 'Match URL from LiveTV' },
                { name: 'resolve', in: 'query', schema: { type: 'boolean', default: false }, description: 'Resolve to direct stream URLs' }
            ],
            responses: { 200: { description: 'Stream variants' } }
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