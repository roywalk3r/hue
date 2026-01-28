const swaggerDef = {
  openapi: '3.0.0',
  info: {
    title: 'Hue Stream API',
    version: '2.2.0',
    description: `
# Unified Streaming API

The **Hue Stream API** is a high-performance aggregator for Movies, TV Shows, KDramas, and Anime. It optimizes for low-resource environments by managing browser instances globally and caching results.

## 🚀 Key Features
- **Smart Aggregation**: Fetch from multiple sources with one ID.
- **Fast vs Slow**: Automatically prioritizes instant HTTP providers (Vixsrc, VidRock) over slow browser scrapers (SFlix, KissKH).
- **Dedicated Drama Endpoint**: Specialized /drama endpoint exclusively for Asian Dramas (KissKH, Dramacool) to avoid polluting main movie searches.
- **Hybrid Extraction**: Dramacool provider uses API for discovery and Browser for fail-safe stream extraction.
- **KDrama Hub**: Full browsing capabilities (Search, Popular, Upcoming) for KissKH and Dramacool.
- **HiTV Discovery**: New robust discovery system for Asian Dramas with rich metadata and filtering.
- **Smart Mapping**: Automatically maps HiTV discovery items to Zenith (KissKH) and Nova (SFlix) streams.
- **Anime Streams**: HiAnime server discovery + stream extraction via a single endpoint.
- **HiAnime Catalog**: Full HiAnime v1 catalog is mounted under /api/v1 (alias: /api/anime).
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
    { name: 'Anime', description: 'HiAnime server discovery and stream extraction.' },
    {
      name: 'HiAnime (Catalog)',
      description: 'HiAnime v1 catalog, discovery, and metadata. See /api/v1/openapi.json for full schema detail.',
      externalDocs: {
        description: 'HiAnime OpenAPI',
        url: '/api/v1/openapi.json'
      }
    },
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
    '/streams/anime/{id}': {
      get: {
        tags: ['Anime'],
        summary: 'HiAnime Streams (Servers or Playable Stream)',
        description: 'Returns a playable stream using the default server (hd-1) when server is omitted. Use serversOnly=true to return the server list without streaming.',
        parameters: [
          { name: 'id', in: 'path', required: true, schema: { type: 'string' }, description: 'HiAnime anime ID, e.g. hells-paradise-season-2-20405::ep=163701' },
          { name: 'server', in: 'query', schema: { type: 'string', enum: ['hd-1', 'hd-2', 'megaplay', 'vidwish'] }, description: 'Stream server name from the servers list' },
          { name: 'type', in: 'query', schema: { type: 'string', enum: ['sub', 'dub'], default: 'sub' } },
          { name: 'serversOnly', in: 'query', schema: { type: 'boolean', default: false }, description: 'Return only the server list (no stream)' }
        ],
        responses: { 200: { description: 'Servers list or playable stream' } }
      }
    },
    '/v1/home': {
      get: {
        tags: ['HiAnime (Catalog)'],
        summary: 'HiAnime Home',
        description: 'Homepage collections: spotlight, trending, top airing, popular, etc.',
        responses: { 200: { description: 'Home payload' } }
      }
    },
    '/v1/spotlight': {
      get: {
        tags: ['HiAnime (Catalog)'],
        summary: 'HiAnime Spotlight',
        responses: { 200: { description: 'Spotlight items' } }
      }
    },
    '/v1/topten': {
      get: {
        tags: ['HiAnime (Catalog)'],
        summary: 'HiAnime Top Ten',
        responses: { 200: { description: 'Top ten lists' } }
      }
    },
    '/v1/anime/{id}': {
      get: {
        tags: ['HiAnime (Catalog)'],
        summary: 'Anime Details',
        parameters: [
          { name: 'id', in: 'path', required: true, schema: { type: 'string' } }
        ],
        responses: { 200: { description: 'Anime detail payload' } }
      }
    },
    '/v1/anime/random': {
      get: {
        tags: ['HiAnime (Catalog)'],
        summary: 'Random Anime',
        responses: { 200: { description: 'Random anime detail payload' } }
      }
    },
    '/v1/search': {
      get: {
        tags: ['HiAnime (Catalog)'],
        summary: 'Search Anime',
        parameters: [
          { name: 'keyword', in: 'query', required: true, schema: { type: 'string' } },
          { name: 'page', in: 'query', schema: { type: 'integer', default: 1 } }
        ],
        responses: { 200: { description: 'Search results' } }
      }
    },
    '/v1/suggestion': {
      get: {
        tags: ['HiAnime (Catalog)'],
        summary: 'Search Suggestions',
        parameters: [
          { name: 'keyword', in: 'query', required: true, schema: { type: 'string' } }
        ],
        responses: { 200: { description: 'Suggestion results' } }
      }
    },
    '/v1/characters/{id}': {
      get: {
        tags: ['HiAnime (Catalog)'],
        summary: 'Anime Characters',
        parameters: [
          { name: 'id', in: 'path', required: true, schema: { type: 'string' } },
          { name: 'page', in: 'query', schema: { type: 'integer', default: 1 } }
        ],
        responses: { 200: { description: 'Characters list' } }
      }
    },
    '/v1/character/{id}': {
      get: {
        tags: ['HiAnime (Catalog)'],
        summary: 'Character Details',
        parameters: [
          { name: 'id', in: 'path', required: true, schema: { type: 'string' } }
        ],
        responses: { 200: { description: 'Character detail payload' } }
      }
    },
    '/v1/actor/{id}': {
      get: {
        tags: ['HiAnime (Catalog)'],
        summary: 'Voice Actor Details',
        parameters: [
          { name: 'id', in: 'path', required: true, schema: { type: 'string' } }
        ],
        responses: { 200: { description: 'Voice actor detail payload' } }
      }
    },
    '/v1/genre/{genre}': {
      get: {
        tags: ['HiAnime (Catalog)'],
        summary: 'Browse by Genre',
        parameters: [
          { name: 'genre', in: 'path', required: true, schema: { type: 'string' } },
          { name: 'page', in: 'query', schema: { type: 'integer', default: 1 } }
        ],
        responses: { 200: { description: 'Genre results' } }
      }
    },
    '/v1/az-list/{letter}': {
      get: {
        tags: ['HiAnime (Catalog)'],
        summary: 'A-Z List',
        parameters: [
          { name: 'letter', in: 'path', required: true, schema: { type: 'string' } },
          { name: 'page', in: 'query', schema: { type: 'integer', default: 1 } }
        ],
        responses: { 200: { description: 'A-Z results' } }
      }
    },
    '/v1/producer/{id}': {
      get: {
        tags: ['HiAnime (Catalog)'],
        summary: 'Browse by Producer',
        parameters: [
          { name: 'id', in: 'path', required: true, schema: { type: 'string' } },
          { name: 'page', in: 'query', schema: { type: 'integer', default: 1 } }
        ],
        responses: { 200: { description: 'Producer results' } }
      }
    },
    '/v1/filter': {
      get: {
        tags: ['HiAnime (Catalog)'],
        summary: 'Filter Anime',
        parameters: [
          { name: 'keyword', in: 'query', schema: { type: 'string' } },
          { name: 'type', in: 'query', schema: { type: 'string' } },
          { name: 'status', in: 'query', schema: { type: 'string' } },
          { name: 'rated', in: 'query', schema: { type: 'string' } },
          { name: 'score', in: 'query', schema: { type: 'string' } },
          { name: 'season', in: 'query', schema: { type: 'string' } },
          { name: 'language', in: 'query', schema: { type: 'string' } },
          { name: 'sort', in: 'query', schema: { type: 'string' } },
          { name: 'genres', in: 'query', schema: { type: 'string' } },
          { name: 'page', in: 'query', schema: { type: 'integer', default: 1 } }
        ],
        responses: { 200: { description: 'Filtered results' } }
      }
    },
    '/v1/episodes/{id}': {
      get: {
        tags: ['HiAnime (Catalog)'],
        summary: 'Episode List',
        parameters: [
          { name: 'id', in: 'path', required: true, schema: { type: 'string' } }
        ],
        responses: { 200: { description: 'Episodes list' } }
      }
    },
    '/v1/servers/{id}': {
      get: {
        tags: ['HiAnime (Catalog)'],
        summary: 'Episode Servers',
        parameters: [
          { name: 'id', in: 'path', required: true, schema: { type: 'string' } }
        ],
        responses: { 200: { description: 'Servers list' } }
      }
    },
    '/v1/stream': {
      get: {
        tags: ['HiAnime (Catalog)'],
        summary: 'Episode Stream',
        parameters: [
          { name: 'id', in: 'query', required: true, schema: { type: 'string' } },
          { name: 'server', in: 'query', schema: { type: 'string', enum: ['hd-1', 'hd-2', 'hd-3', 'megaplay', 'vidwish'], default: 'hd-1' } },
          { name: 'type', in: 'query', schema: { type: 'string', enum: ['sub', 'dub'], default: 'sub' } }
        ],
        responses: { 200: { description: 'Stream payload' } }
      }
    },
    '/v1/schedule': {
      get: {
        tags: ['HiAnime (Catalog)'],
        summary: 'Monthly Schedule',
        parameters: [
          { name: 'date', in: 'query', schema: { type: 'string' }, description: 'e.g. 21 or 2024-10-21' }
        ],
        responses: { 200: { description: 'Schedule list' } }
      }
    },
    '/v1/schedule/next/{id}': {
      get: {
        tags: ['HiAnime (Catalog)'],
        summary: 'Next Episode Schedule',
        parameters: [
          { name: 'id', in: 'path', required: true, schema: { type: 'string' } }
        ],
        responses: { 200: { description: 'Next episode schedule' } }
      }
    },
    '/v1/meta': {
      get: {
        tags: ['HiAnime (Catalog)'],
        summary: 'HiAnime Meta',
        description: 'Returns genres, A-Z list, explore routes, and filter options.',
        responses: { 200: { description: 'Meta payload' } }
      }
    },
    '/v1/{query}': {
      get: {
        tags: ['HiAnime (Catalog)'],
        summary: 'Explore Lists',
        description: 'Explore lists such as top-airing, most-popular, most-favorite, latest-completed, etc.',
        parameters: [
          { name: 'query', in: 'path', required: true, schema: { type: 'string' } },
          { name: 'page', in: 'query', schema: { type: 'integer', default: 1 } }
        ],
        responses: { 200: { description: 'Explore results' } }
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
        description: 'Extract streams from a specific provider (e.g., hianime, vixsrc, sflix). KissKH captures an episode token and uses proxy URL mode (no h_token).',
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
    '/kdrama/show': {
        get: {
            tags: ['KDrama (KissKH)'],
            summary: 'Show Library',
            description: 'KissKH show library list.',
            responses: { 200: { description: 'Show list' } }
        }
    },
    '/kdrama/last-update': {
        get: {
            tags: ['KDrama (KissKH)'],
            summary: 'Last Update',
            parameters: [{ name: 'ispc', in: 'query', schema: { type: 'boolean', default: false } }],
            responses: { 200: { description: 'Latest updated items' } }
        }
    },
    '/kdrama/most-view': {
        get: {
            tags: ['KDrama (KissKH)'],
            summary: 'Most Viewed',
            parameters: [
                { name: 'ispc', in: 'query', schema: { type: 'boolean', default: false } },
                { name: 'c', in: 'query', schema: { type: 'integer', default: 1, description: 'Category (1=drama, 2=movie)' } }
            ],
            responses: { 200: { description: 'Most viewed list' } }
        }
    },
    '/kdrama/top-rating': {
        get: {
            tags: ['KDrama (KissKH)'],
            summary: 'Top Rating',
            parameters: [{ name: 'ispc', in: 'query', schema: { type: 'boolean', default: false } }],
            responses: { 200: { description: 'Top rated list' } }
        }
    },
    '/kdrama/animate': {
        get: {
            tags: ['KDrama (KissKH)'],
            summary: 'Animate',
            parameters: [{ name: 'ispc', in: 'query', schema: { type: 'boolean', default: false } }],
            responses: { 200: { description: 'Animated list' } }
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
            description: 'Captures an episode token, then fetches the stream and subtitles via proxy URL mode (no h_token).',
            parameters: [
                { name: 'id', in: 'path', required: true, description: 'KissKH ID' },
                { name: 'episode', in: 'path', required: true, description: 'Episode number' }
            ],
            responses: { 200: { description: 'Playable streams (with subtitles when available)' } }
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
