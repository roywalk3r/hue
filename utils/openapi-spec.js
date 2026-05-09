/**
 * HUE ENGINE ⬢
 * High-Performance Stream Aggregation Engine
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */

/**
 * HUE ENGINE ⬢ — Comprehensive OpenAPI 3.1.0 Specification
 */
const pkg = require('../package.json');

function buildOpenApiSpec() {
  return {
    openapi: '3.1.0',
    info: {
      title: 'HUE ENGINE ⬢',
      version: pkg.version,
      description: 'Technical Manual for the Hue Stream Aggregation Engine. Engineered for high-performance stream telemetry across decentralized providers. Authorized access only. Secure HMAC signing enforced on all proxy egress.',
      contact: { name: 'System Administrator' },
    },
    servers: [{ url: '/', description: 'Default Server' }],
    tags: [
      { name: 'Telemetry', description: 'Real-time stream aggregation and resolution.' },
      { name: 'Registry', description: 'Provider node management and status.' },
      { name: 'Gateway', description: 'Smart mapping for regional discovery systems.' },
      { name: 'Drama', description: 'Specialized extractors for regional drama content.' },
      { name: 'Sports', description: 'Live broadcast telemetry and event resolution.' },
      { name: 'LiveTV', description: 'Linear broadcast channel management.' },
      { name: 'Anime', description: 'Specialized discovery for animation content.' },
      { name: 'System', description: 'Core engine configuration and diagnostics.' },
      { name: 'Proxy', description: 'Secure egress routing and header manipulation.' },
      { name: 'Auth', description: 'Access control and session management.' },
    ],
    paths: {
      // ── Telemetry ────────────────────────────────────────────
      '/api/streams/{type}/{tmdbId}': {
        get: {
          tags: ['Telemetry'],
          summary: 'Aggregate streams across all providers',
          description: 'Fetches streams from every enabled provider in parallel with a 15s deadline. Returns complete or partial results.',
          parameters: [
            { name: 'type', in: 'path', required: true, schema: { type: 'string', enum: ['movie', 'series'] }, description: 'Content type' },
            { name: 'tmdbId', in: 'path', required: true, schema: { type: 'string' }, description: 'TMDB ID' },
            { name: 'season', in: 'query', schema: { type: 'integer' }, description: 'Season number (series only)' },
            { name: 'episode', in: 'query', schema: { type: 'integer' }, description: 'Episode number (series only)' },
            { name: 'nocache', in: 'query', schema: { type: 'string', enum: ['0', '1'] }, description: 'Set to 1 to bypass cache' },
          ],
          responses: {
            200: { description: 'Stream results', content: { 'application/json': { schema: { $ref: '#/components/schemas/StreamAggregationResponse' } } } },
            400: { description: 'Invalid type', content: { 'application/json': { schema: { $ref: '#/components/schemas/ErrorResponse' } } } },
            500: { description: 'Internal error', content: { 'application/json': { schema: { $ref: '#/components/schemas/ErrorResponse' } } } },
          },
        },
      },
      '/api/streams/{type}/{tmdbId}/live': {
        get: {
          tags: ['Telemetry'],
          summary: 'Live SSE stream aggregation',
          description: 'Transmits results progressively via Server-Sent Events as each provider completes. Events: `cached`, `provider`, `done`, `error`.',
          parameters: [
            { name: 'type', in: 'path', required: true, schema: { type: 'string', enum: ['movie', 'series'] }, description: 'Content type' },
            { name: 'tmdbId', in: 'path', required: true, schema: { type: 'string' }, description: 'TMDB ID' },
            { name: 'season', in: 'query', schema: { type: 'integer' }, description: 'Season number (series only)' },
            { name: 'episode', in: 'query', schema: { type: 'integer' }, description: 'Episode number (series only)' },
            { name: 'nocache', in: 'query', schema: { type: 'string', enum: ['0', '1'] }, description: 'Set to 1 to bypass cache' },
          ],
          responses: {
            200: {
              description: 'SSE stream with provider results as they arrive',
              content: {
                'text/event-stream': {
                  schema: { $ref: '#/components/schemas/SseStreamEvent' },
                },
              },
            },
            400: { description: 'Invalid type', content: { 'application/json': { schema: { $ref: '#/components/schemas/ErrorResponse' } } } },
          },
        },
      },
      '/api/streams/drama/{type}/{tmdbId}': {
        get: {
          tags: ['Telemetry'],
          summary: 'Aggregate drama streams (KissKH + Dramacool only)',
          parameters: [
            { name: 'type', in: 'path', required: true, schema: { type: 'string', enum: ['movie', 'series'] } },
            { name: 'tmdbId', in: 'path', required: true, schema: { type: 'string' } },
            { name: 'season', in: 'query', schema: { type: 'integer' } },
            { name: 'episode', in: 'query', schema: { type: 'integer' } },
          ],
          responses: {
            200: { description: 'Drama stream results', content: { 'application/json': { schema: { $ref: '#/components/schemas/DramaStreamResponse' } } } },
          },
        },
      },

      // ── Registry ─────────────────────────────────────────────
      '/api/hitv/browse': {
        get: {
          tags: ['Registry'],
          summary: 'Browse HiTV library',
          parameters: [
            { name: 'page', in: 'query', schema: { type: 'integer', default: 1 } },
            { name: 'pageSize', in: 'query', schema: { type: 'integer', default: 28 } },
            { name: 'country', in: 'query', schema: { type: 'string', default: 'Korea' } },
            { name: 'category', in: 'query', schema: { type: 'string', default: 'Drama' } },
            { name: 'genre', in: 'query', schema: { type: 'string', default: 'All' } },
            { name: 'sort', in: 'query', schema: { type: 'string', default: 'Hottest' } },
          ],
          responses: { 200: { description: 'Browse results' } },
        },
      },
      '/api/hitv/latest': {
        get: { tags: ['Registry'], summary: 'Latest dramas', parameters: [
          { name: 'page', in: 'query', schema: { type: 'integer', default: 1 } },
          { name: 'country', in: 'query', schema: { type: 'string', default: 'Korea' } },
          { name: 'genre', in: 'query', schema: { type: 'string', default: 'All' } },
        ], responses: { 200: { description: 'Latest results' } } },
      },
      '/api/hitv/trending': {
        get: { tags: ['Registry'], summary: 'Trending dramas', parameters: [
          { name: 'page', in: 'query', schema: { type: 'integer', default: 1 } },
          { name: 'country', in: 'query', schema: { type: 'string', default: 'Korea' } },
          { name: 'genre', in: 'query', schema: { type: 'string', default: 'All' } },
        ], responses: { 200: { description: 'Trending results' } } },
      },
      '/api/hitv/search': {
        get: { tags: ['Registry'], summary: 'Search HiTV', parameters: [
          { name: 'q', in: 'query', required: true, schema: { type: 'string' }, description: 'Search query' },
          { name: 'type', in: 'query', schema: { type: 'integer', default: 2 } },
          { name: 'page', in: 'query', schema: { type: 'integer', default: 1 } },
        ], responses: { 200: { description: 'Search results' }, 400: { description: 'Missing query' } } },
      },
      '/api/hitv/library': { get: { tags: ['Registry'], summary: 'HiTV library data', responses: { 200: { description: 'Library items' } } } },
      '/api/hitv/hero': { get: { tags: ['Registry'], summary: 'Hero slider data', responses: { 200: { description: 'Hero items' } } } },
      '/api/hitv/drama': {
        get: { tags: ['Registry'], summary: 'Browse dramas with filters', parameters: [
          { name: 'page', in: 'query', schema: { type: 'integer', default: 1 } },
          { name: 'country', in: 'query', schema: { type: 'string', default: 'Korea' } },
          { name: 'genre', in: 'query', schema: { type: 'string', default: 'All' } },
          { name: 'sort', in: 'query', schema: { type: 'string', default: 'Hottest' } },
          { name: 'year', in: 'query', schema: { type: 'integer' } },
        ], responses: { 200: { description: 'Drama browse results' } } },
      },
      '/api/hitv/info/{id}': {
        get: { tags: ['Registry'], summary: 'Show details', parameters: [
          { name: 'id', in: 'path', required: true, schema: { type: 'string' }, description: 'HiTV show ID' },
        ], responses: { 200: { description: 'Show details' }, 404: { description: 'Not found' } } },
      },
      '/api/hitv/countries': { get: { tags: ['Registry'], summary: 'Available countries', responses: { 200: { description: 'Country list' } } } },
      '/api/hitv/categories': { get: { tags: ['Registry'], summary: 'Available categories', responses: { 200: { description: 'Category list' } } } },
      '/api/hitv/genres': { get: { tags: ['Registry'], summary: 'Available genres', responses: { 200: { description: 'Genre list' } } } },
      '/api/hitv/sorts': { get: { tags: ['Registry'], summary: 'Available sort options', responses: { 200: { description: 'Sort options' } } } },
      '/api/hitv/play/{id}/{episode}': {
        get: { tags: ['Registry'], summary: 'Smart Play — aggregate streams', parameters: [
          { name: 'id', in: 'path', required: true, schema: { type: 'string' }, description: 'HiTV ID or slug' },
          { name: 'episode', in: 'path', required: true, schema: { type: 'integer' } },
          { name: 'title', in: 'query', schema: { type: 'string' } },
        ], responses: { 200: { description: 'Stream results' } } },
      },
      '/api/hitv/{country}/{sort}/{genre}': {
        get: { tags: ['Registry'], summary: 'Dynamic discovery', parameters: [
          { name: 'country', in: 'path', required: true, schema: { type: 'string' } },
          { name: 'sort', in: 'path', schema: { type: 'string' } },
          { name: 'genre', in: 'path', schema: { type: 'string' } },
          { name: 'page', in: 'query', schema: { type: 'integer', default: 1 } },
          { name: 'year', in: 'query', schema: { type: 'integer' } },
        ], responses: { 200: { description: 'Discovery results' } } },
      },

      // ── Registry Direct Access ───────────────────
      '/api/hitv/taxonomy': {
        get: {
          tags: ['Registry'],
          summary: 'Full HiTV taxonomy — categories, genres, locations, sorts, years, statuses',
          description: 'Returns the complete HiTV content taxonomy cached from the live API. Use this to build dynamic discovery UIs. Response includes all valid values for the gallery filter params.',
          responses: {
            200: {
              description: 'Full taxonomy',
              content: { 'application/json': { example: {
                success: true,
                categories: [
                  { category: 1, name: 'Drama', genres: [{ genre: 3, name: 'Romance' }, { genre: 4, name: 'Fantasy' }] },
                  { category: 2, name: 'Variety Show', genres: [{ genre: 7, name: 'TV Show' }, { genre: 61, name: 'BL' }] },
                  { category: 3, name: 'Movie', genres: [{ genre: 18, name: 'Romance' }, { genre: 29, name: 'Animation' }] },
                  { category: 4, name: 'Animation', genres: [{ genre: 42, name: 'Anime' }, { genre: 45, name: 'Action' }] },
                ],
                locations: [
                  { location: 1, name: 'Korea' }, { location: 5, name: 'Japan' },
                  { location: 2, name: 'Mainland China' }, { location: 4, name: 'Thailand' },
                  { location: 3, name: 'Western' }, { location: 6, name: 'Turkey' },
                  { location: 10, name: 'Taiwan' }, { location: 12, name: 'Philippines' },
                ],
                sorts: [
                  { sort: 'hot', name: 'Hot' }, { sort: 'new', name: 'Latest' }, { sort: 'score', name: 'Rating' },
                ],
                statuses: [
                  { value: '-1', name: 'All' }, { value: 'updating', name: 'Ongoing' }, { value: 'finished', name: 'Completed' },
                ],
                years: [{ value: '-1', name: 'All' }, { value: '2025', name: '2025' }, { value: '2024', name: '2024' }],
              } } },
            },
          },
        },
      },
      '/api/hitv/gallery': {
        get: {
          tags: ['Registry'],
          summary: 'Browse HiTV gallery with full filters — any country, genre, category, sort',
          description: 'Direct decrypted access to the HiTV gallery API. Supports every combination of category (Drama/Movie/Variety/Animation), location (Korea/Japan/China/Thailand/etc.), genre, and sort. Returns full series data with titles, cast, episodes, and artwork.',
          parameters: [
            { name: 'cate', in: 'query', schema: { type: 'integer', default: 1 }, description: 'Category: 1=Drama, 2=Variety Show, 3=Movie, 4=Animation' },
            { name: 'location', in: 'query', schema: { type: 'integer', default: 1 }, description: 'Country: 1=Korea, 5=Japan, 2=Mainland China, 4=Thailand, 3=Western, 6=Turkey, 10=Taiwan, 12=Philippines… (see /taxonomy for full list)' },
            { name: 'genre', in: 'query', schema: { type: 'integer', default: -1 }, description: 'Genre ID: -1=All, 3=Romance, 4=Fantasy, 56=BL, 57=GL, 1=Action… (see /taxonomy for full list)' },
            { name: 'sort', in: 'query', schema: { type: 'string', default: 'hot' }, description: 'Sort: hot, new, score (rating)' },
            { name: 'page', in: 'query', schema: { type: 'integer', default: 1 }, description: 'Page number (48 items per page)' },
            { name: 'did', in: 'query', schema: { type: 'string' }, description: 'Device ID for decryption (auto-generated if omitted)' },
          ],
          responses: {
            200: {
              description: 'Gallery results with series list',
              content: { 'application/json': { example: {
                success: true,
                total: 145,
                more: 1,
                categories: [{ category: 1, name: 'Drama', genres: [{}] }],
                series: [
                  {
                    sid: 's_abc123', sidAlias: 'alice-in-borderland-s03', name: 'Alice in Borderland S03',
                    category: 1, lastSerialNo: 6, finished: 0, cornerMemo: 'Updated to 6',
                    crew: { starring: 'Yamazaki Kento, Tsuchiya Tao', director: '', writer: '' },
                    image: { thumb: { url: 'https://s.hitv.vip/…', width: 600, height: 800 } },
                  },
                ],
              } } },
            },
            502: { description: 'Decryption failed — API returned no encrypted data' },
          },
        },
      },
      '/api/hitv/episodes/{sid}': {
        get: {
          tags: ['Registry'],
          summary: 'Get episode list for a series',
          description: 'Fetches and decrypts the full episode list for a HiTV series, including serial numbers, titles, and thumbnails.',
          parameters: [
            { name: 'sid', in: 'path', required: true, schema: { type: 'string' }, description: 'Series ID, e.g. s_abc123' },
            { name: 'did', in: 'query', schema: { type: 'string' }, description: 'Device ID for decryption' },
          ],
          responses: {
            200: { description: 'Episode list', content: { 'application/json': { example: {
              success: true,
              sid: 's_f2prDoHCi7Oadj6HElLT',
              episodes: [
                { eid: 'e_f2prDoHCi7Oadj6HElLT', serialNo: 1, title: 'Episode 1', thumb: { url: 'https://s.hitv.vip/…' } },
                { eid: 'e_abc123', serialNo: 2, title: 'Episode 2', thumb: { url: 'https://s.hitv.vip/…' } },
              ],
            } } } },
            502: { description: 'Failed to fetch episodes' },
          },
        },
      },
      '/api/hitv/episode/{eid}': {
        get: {
          tags: ['Registry'],
          summary: 'Get episode detail with stream sources',
          description: 'Returns full episode metadata including available stream definitions (quality levels, formats) for a given episode ID.',
          parameters: [
            { name: 'eid', in: 'path', required: true, schema: { type: 'string' }, description: 'Episode ID, e.g. e_f2prDoHCi7Oadj6HElLT' },
            { name: 'did', in: 'query', schema: { type: 'string' }, description: 'Device ID for decryption' },
          ],
          responses: {
            200: { description: 'Episode detail', content: { 'application/json': { example: {
              success: true,
              sid: 's_f2prDoHCi7Oadj6HElLT',
              eid: 'e_f2prDoHCi7Oadj6HElLT',
              scid: 'sc_xyz',
              serialNo: 1,
              title: 'Episode 1',
              duration: 3540,
              streams: [{ quality: '1080p', format: 'hls', definition: 'FHD' }],
            } } } },
            502: { description: 'Failed to fetch episode detail' },
          },
        },
      },
      '/api/hitv/streams/{eid}': {
        get: {
          tags: ['Registry'],
          summary: 'Resolve stream sources for an episode',
          description: 'Returns stream source definitions (scid, qualities) for an episode. Falls back to episode detail when direct resolution is unavailable.',
          parameters: [
            { name: 'eid', in: 'path', required: true, schema: { type: 'string' }, description: 'Episode ID, e.g. e_f2prDoHCi7Oadj6HElLT' },
            { name: 'quality', in: 'query', schema: { type: 'integer', default: 1 }, description: 'Quality level (1=auto)' },
            { name: 'did', in: 'query', schema: { type: 'string' }, description: 'Device ID for decryption' },
          ],
          responses: {
            200: { description: 'Stream sources', content: { 'application/json': { example: {
              success: true,
              eid: 'e_f2prDoHCi7Oadj6HElLT',
              title: '1. Senpai, grant me an audience!',
              sources: [
                {
                  scid: 'sc_TgAZFOAyuC4FY6p7t_Fd',
                  srcPrior: 1,
                  qualities: [
                    { resolution: '1080P', value: 10, vipType: 101 },
                    { resolution: '720P', value: 3, vipType: 100 },
                    { resolution: '480P', value: 2, vipType: 100 },
                  ],
                },
              ],
            } } } },
            502: { description: 'Failed to resolve streams' },
          },
        },
      },
      '/api/hitv/direct/*': {
        get: {
          tags: ['Registry'],
          summary: 'Generic HiTV API proxy — fetch and decrypt any endpoint',
          description: 'Pass any HiTV API path as the wildcard suffix and it will be fetched and decrypted. Useful for endpoints not covered by dedicated routes.',
          parameters: [
            { name: 'path', in: 'path', required: true, schema: { type: 'string' }, description: 'HiTV API path, e.g. /s1/w/routine/api/search/config', example: '/s1/w/routine/api/search/config' },
            { name: 'did', in: 'query', schema: { type: 'string' }, description: 'Device ID for decryption' },
          ],
          responses: {
            200: { description: 'Decrypted API response', content: { 'application/json': { example: { success: true, hotwords: [{ word: 'only friends dream on', tag: '热' }] } } } },
            502: { description: 'No encrypted data in response' },
          },
        },
      },
      '/api/hitv/album/{aid}': {
        get: {
          tags: ['Registry'],
          summary: 'Get album detail — trending collections, genre collections, etc.',
          description: 'Fetches and decrypts a HiTV album including its series list and metadata. Albums are curated collections like trending, genre-based, or homepage carousels.',
          parameters: [
            { name: 'aid', in: 'path', required: true, schema: { type: 'string' }, description: 'Album ID, e.g. a_MT4IPBbd_619kbg8HYh1g' },
            { name: 'did', in: 'query', schema: { type: 'string' }, description: 'Device ID for decryption' },
          ],
          responses: {
            200: { description: 'Album detail with series list', content: { 'application/json': { example: {
              success: true,
              album: { aid: 'a_MT4IPBbd_619kbg8HYh1g', name: 'Trending Now' },
              seriesList: [{ sid: 's_abc123', name: 'Alice in Borderland S03', image: {} }],
              more: true,
            } } } },
            502: { description: 'Failed to fetch album' },
          },
        },
      },
      '/api/hitv/hotwords': {
        get: {
          tags: ['Registry'],
          summary: 'Get trending search hotwords',
          description: 'Returns the list of currently trending search terms on HiTV. Useful for search suggestion UIs.',
          parameters: [
            { name: 'did', in: 'query', schema: { type: 'string' }, description: 'Device ID for decryption' },
          ],
          responses: {
            200: { description: 'Trending hotwords', content: { 'application/json': { example: {
              success: true,
              hotwords: [{ word: 'only friends dream on', tag: '热' }, { word: 'alice in borderland', tag: '新' }],
            } } } },
          },
        },
      },
      '/api/hitv/navigations': {
        get: {
          tags: ['Registry'],
          summary: 'Get navigation/discovery entries',
          description: 'Returns the HiTV navigation menu structure and discovery entries. Useful for building dynamic navigation UIs that mirror the HiTV app.',
          parameters: [
            { name: 'did', in: 'query', schema: { type: 'string' }, description: 'Device ID for decryption' },
          ],
          responses: {
            200: { description: 'Navigation entries', content: { 'application/json': { example: {
              success: true,
              navigations: [{ id: 1, name: 'Home', type: 'home' }, { id: 2, name: 'Gallery', type: 'gallery' }],
            } } } },
          },
        },
      },

      // ── Gateway ──────────────────────────────────────────────
      '/api/moviebox/home': { get: { tags: ['Gateway'], summary: 'Home page data', responses: { 200: { description: 'Home sections' } } } },
      '/api/moviebox/discover': {
        get: { tags: ['Gateway'], summary: 'Discover content', parameters: [
          { name: 'channelId', in: 'query', schema: { type: 'integer' } },
        ], responses: { 200: { description: 'Discover sections or filtered data' } } },
      },
      '/api/moviebox/section': {
        get: { tags: ['Gateway'], summary: 'Get catalog row', parameters: [
          { name: 'title', in: 'query', required: true, schema: { type: 'string' }, example: 'Popular Series' },
        ], responses: { 200: { description: 'Section items' }, 400: { description: 'Missing title' } } },
      },
      '/api/moviebox/trending': { get: { tags: ['Gateway'], summary: 'Trending content', responses: { 200: { description: 'Trending items' } } } },
      '/api/moviebox/popular-movie': { get: { tags: ['Gateway'], summary: 'Popular movies', responses: { 200: { description: 'Popular movies' } } } },
      '/api/moviebox/popular-series': { get: { tags: ['Gateway'], summary: 'Popular series', responses: { 200: { description: 'Popular series' } } } },
      '/api/moviebox/hot-shorts': { get: { tags: ['Gateway'], summary: 'Hot shorts', responses: { 200: { description: 'Hot shorts' } } } },
      '/api/moviebox/sa': { get: { tags: ['Gateway'], summary: 'SA Drama', responses: { 200: { description: 'SA Drama items' } } } },
      '/api/moviebox/thai': { get: { tags: ['Gateway'], summary: 'Thai Drama', responses: { 200: { description: 'Thai Drama items' } } } },
      '/api/moviebox/turkish': { get: { tags: ['Gateway'], summary: 'Turkish Drama', responses: { 200: { description: 'Turkish Drama items' } } } },
      '/api/moviebox/western': { get: { tags: ['Gateway'], summary: 'Western TV', responses: { 200: { description: 'Western TV items' } } } },
      '/api/moviebox/black-shows': { get: { tags: ['Gateway'], summary: 'Must-watch Black Shows', responses: { 200: { description: 'Black shows' } } } },
      '/api/moviebox/hero': { 
        get: { 
          tags: ['Gateway'], 
          summary: 'Premium Hero content', 
          description: 'Returns a curated mix of TMDB trending hits and enriched Asian dramas, complete with trailers, stills, and full synopses.',
          responses: { 200: { description: 'Hero items with trailers and metadata' } } 
        } 
      },
      '/api/moviebox/bollywood': {
        get: { tags: ['Gateway'], summary: 'Bollywood content', parameters: [
          { name: 'type', in: 'query', schema: { type: 'string', default: 'movie' } },
          { name: 'page', in: 'query', schema: { type: 'integer', default: 1 } },
        ], responses: { 200: { description: 'Bollywood items' } } },
      },
      '/api/moviebox/shorts': {
        get: { tags: ['Gateway'], summary: 'Shorts', parameters: [
          { name: 'page', in: 'query', schema: { type: 'integer', default: 1 } },
        ], responses: { 200: { description: 'Shorts' } } },
      },
      '/api/moviebox/filter': {
        get: { tags: ['Gateway'], summary: 'Advanced filter search', parameters: [
          { name: 'page', in: 'query', schema: { type: 'integer', default: 1 } },
          { name: 'perPage', in: 'query', schema: { type: 'integer', default: 28 } },
          { name: 'channelId', in: 'query', schema: { type: 'integer', default: 2 } },
          { name: 'genre', in: 'query', schema: { type: 'string' } },
          { name: 'country', in: 'query', schema: { type: 'string' } },
          { name: 'year', in: 'query', schema: { type: 'string' } },
          { name: 'sort', in: 'query', schema: { type: 'string' } },
        ], responses: { 200: { description: 'Filtered results' } } },
      },
      '/api/moviebox/info': {
        get: { tags: ['Gateway'], summary: 'Content detail info', parameters: [
          { name: 'path', in: 'query', required: true, schema: { type: 'string' }, description: 'Gateway content path' },
        ], responses: { 200: { description: 'Content details' }, 400: { description: 'Missing path' } } },
      },
      '/api/moviebox/search': {
        get: { tags: ['Gateway'], summary: 'Search catalog', parameters: [
          { name: 'keyword', in: 'query', required: true, schema: { type: 'string' } },
          { name: 'page', in: 'query', schema: { type: 'integer', default: 1 } },
          { name: 'type', in: 'query', schema: { type: 'integer', default: 0 } },
        ], responses: { 200: { description: 'Search results' }, 400: { description: 'Missing keyword' } } },
      },
      '/api/moviebox/play/{id}/{path}': {
        get: { tags: ['Gateway'], summary: 'Extract direct streams', description: 'Wildcard path after ID. Supports season/episode for series.', parameters: [
          { name: 'id', in: 'path', required: true, schema: { type: 'string' } },
          { name: 'path', in: 'path', required: true, schema: { type: 'string' }, description: 'Remaining path (can contain slashes)' },
          { name: 'season', in: 'query', schema: { type: 'integer' } },
          { name: 'episode', in: 'query', schema: { type: 'integer' } },
        ], responses: { 200: { description: 'Stream results with captions' } } },
      },

      // ── Play Gateway ──────────────────────────────────────────
      '/api/play/{type}/{tmdbId}': {
        get: {
          tags: ['Telemetry'],
          summary: 'Smart Play Gateway',
          description: 'Unified endpoint that validates and returns the single best working proxied stream for a title. Ideal for direct player integration.',
          parameters: [
            { name: 'type', in: 'path', required: true, schema: { type: 'string', enum: ['movie', 'series'] } },
            { name: 'tmdbId', in: 'path', required: true, schema: { type: 'string' } },
            { name: 'season', in: 'query', schema: { type: 'integer', default: 1 } },
            { name: 'episode', in: 'query', schema: { type: 'integer', default: 1 } },
            { name: 'redirect', in: 'query', schema: { type: 'string', enum: ['0', '1'] }, description: 'Set to 1 to 302 redirect directly to the stream URL' },
          ],
          responses: {
            200: { description: 'Best stream found', content: { 'application/json': { schema: { $ref: '#/components/schemas/Stream' } } } },
            302: { description: 'Redirect to stream URL' },
            404: { description: 'No streams found' },
          },
        },
      },

      // ── Drama ─────────────────────────────────────────────────
      '/api/kdrama/search': {
        get: { tags: ['Drama'], summary: 'Search KissKH', parameters: [
          { name: 'q', in: 'query', required: true, schema: { type: 'string' } },
        ], responses: { 200: { description: 'Search results' }, 400: { description: 'Missing query' } } },
      },
      '/api/kdrama/latest': {
        get: { tags: ['Drama'], summary: 'Latest KissKH dramas', parameters: [
          { name: 'page', in: 'query', schema: { type: 'integer', default: 1 } },
        ], responses: { 200: { description: 'Latest dramas' } } },
      },
      '/api/kdrama/streams/{id}/{episode}': {
        get: { tags: ['Drama'], summary: 'KissKH streams', parameters: [
          { name: 'id', in: 'path', required: true, schema: { type: 'string' } },
          { name: 'episode', in: 'path', schema: { type: 'integer', default: 1 } },
        ], responses: { 200: { description: 'Stream results' } } },
      },
      '/api/dramacool/search': {
        get: { tags: ['Drama'], summary: 'Search Dramacool', parameters: [
          { name: 'q', in: 'query', required: true, schema: { type: 'string' } },
          { name: 'page', in: 'query', schema: { type: 'integer', default: 1 } },
        ], responses: { 200: { description: 'Search results' }, 400: { description: 'Missing query' } } },
      },
      '/api/dramacool/latest': {
        get: { tags: ['Drama'], summary: 'Latest Dramacool content', parameters: [
          { name: 'page', in: 'query', schema: { type: 'integer', default: 1 } },
        ], responses: { 200: { description: 'Latest content' } } },
      },
      '/api/dramacool/browse/streams/{id}/{episode}': {
        get: { tags: ['Drama'], summary: 'Dramacool streams', parameters: [
          { name: 'id', in: 'path', required: true, schema: { type: 'string' }, description: 'Dramacool path slug' },
          { name: 'episode', in: 'path', required: true, schema: { type: 'integer' } },
        ], responses: { 200: { description: 'Stream results' } } },
      },

      // ── Sports ────────────────────────────────────────────────
      '/api/aisports/leagues': { get: { tags: ['Sports'], summary: 'Available leagues', responses: { 200: { description: 'League list' } } } },
      '/api/aisports/list/{leagueId}': {
        get: { tags: ['Sports'], summary: 'Match list for a league', parameters: [
          { name: 'leagueId', in: 'path', required: true, schema: { type: 'integer' } },
          { name: 'status', in: 'query', schema: { type: 'string' } },
          { name: 'type', in: 'query', schema: { type: 'string' } },
        ], responses: { 200: { description: 'Match list' } } },
      },
      '/api/aisports/flow/{id}': {
        get: { tags: ['Sports'], summary: 'Match flow/timeline', parameters: [
          { name: 'id', in: 'path', required: true, schema: { type: 'string' } },
          { name: 'pageSize', in: 'query', schema: { type: 'integer' } },
          { name: 'lastSort', in: 'query', schema: { type: 'string' } },
        ], responses: { 200: { description: 'Match flow' } } },
      },
      '/api/aisports/play/{id}': {
        get: { tags: ['Sports'], summary: 'Play by match ID', parameters: [
          { name: 'id', in: 'path', required: true, schema: { type: 'string' } },
        ], responses: { 200: { description: 'Stream results' } } },
      },
      '/api/ppv/streams/{matchName}': {
        get: { tags: ['Sports'], summary: 'PPV event streams', parameters: [
          { name: 'matchName', in: 'path', required: true, schema: { type: 'string' }, description: 'PPV match name (can contain slashes)' },
        ], responses: { 200: { description: 'PPV stream results' } } },
      },

      // ── LiveTV ────────────────────────────────────────────────
      '/api/livetv': {
        get: { tags: ['LiveTV'], summary: 'Sports & matches overview', parameters: [
          { name: 'sportId', in: 'query', schema: { type: 'integer' } },
        ], responses: { 200: { description: 'Sports and matches' } } },
      },
      '/api/livetv/sports': { get: { tags: ['LiveTV'], summary: 'Available sports categories', responses: { 200: { description: 'Sports list' } } } },
      '/api/livetv/matches': {
        get: { tags: ['LiveTV'], summary: 'Match list', parameters: [
          { name: 'sportId', in: 'query', schema: { type: 'integer' } },
        ], responses: { 200: { description: 'Match list' } } },
      },
      '/api/livetv/match': {
        get: { tags: ['LiveTV'], summary: 'Resolve match streams', description: 'Supports SSE streaming mode with stream=true.', parameters: [
          { name: 'url', in: 'query', required: true, schema: { type: 'string' }, description: 'Match URL' },
          { name: 'resolve', in: 'query', schema: { type: 'string', enum: ['true', 'false'] } },
          { name: 'stream', in: 'query', schema: { type: 'string', enum: ['true', 'false'] }, description: 'Enable SSE mode' },
        ], responses: { 200: { description: 'Stream results (JSON or SSE)' } } },
      },

      // ── Anime ─────────────────────────────────────────────────
      '/api/anime/home': {
        get: {
          tags: ['Anime'],
          summary: 'Get anime home feed',
          description: 'Fetches the curated home feed with featured, trending, and latest categories.',
          responses: { 200: { description: 'Home data' } }
        }
      },
      '/api/anime/category': {
        get: {
          tags: ['Anime'],
          summary: 'Get anime by category',
          description: 'Filter anime by type (tv, movie, ova, ona).',
          parameters: [
            { name: 'type', in: 'query', schema: { type: 'string', enum: ['tv', 'movie', 'ova', 'ona'], default: 'tv' } },
            { name: 'page', in: 'query', schema: { type: 'integer', default: 1 } },
            { name: 'limit', in: 'query', schema: { type: 'integer', default: 20 } }
          ],
          responses: { 200: { description: 'Category items' } }
        }
      },
      '/api/anime/search': {
        get: {
          tags: ['Anime'],
          summary: 'Search for anime',
          description: 'Universal search across the HiAnime library.',
          parameters: [
            { name: 'q', in: 'query', required: true, schema: { type: 'string' } },
            { name: 'page', in: 'query', schema: { type: 'integer', default: 1 } }
          ],
          responses: { 200: { description: 'Search results' } }
        }
      },
      '/api/anime/latest': {
        get: {
          tags: ['Anime'],
          summary: 'Get latest updated anime',
          parameters: [
            { name: 'page', in: 'query', schema: { type: 'integer', default: 1 } }
          ],
          responses: { 200: { description: 'Latest anime list' } }
        }
      },
      '/api/anime/servers': {
        get: {
          tags: ['Anime'],
          summary: 'Get anime episode servers',
          description: 'Fetches the available streaming servers for a specific anime episode slug.',
          parameters: [
            { name: 'id', in: 'query', required: true, schema: { type: 'string' }, description: 'The anime episode slug (e.g., one-piece-episode-1158-xra5nl)' }
          ],
          responses: {
            200: { description: 'Server list', content: { 'application/json': { schema: { $ref: '#/components/schemas/AnimeServerResponse' } } } },
            400: { description: 'Missing ID', content: { 'application/json': { schema: { $ref: '#/components/schemas/ErrorResponse' } } } }
          }
        }
      },
      '/api/anime/stream': {
        get: {
          tags: ['Anime'],
          summary: 'Get direct anime stream',
          description: 'Extracts the final .m3u8 stream and tracks for a chosen server. When enableProxy is active, stream URLs and subtitle tracks are rewritten through the proxy endpoints.',
          parameters: [
            { name: 'id', in: 'query', required: true, schema: { type: 'string' }, description: 'Anime hex ID (24-char MongoDB ID) or episode slug' },
            { name: 'server', in: 'query', schema: { type: 'string' }, description: 'Target server name (e.g., HD-1)' },
            { name: 'type', in: 'query', schema: { type: 'string', enum: ['sub', 'dub'], default: 'sub' }, description: 'Audio type' },
            { name: 'episode', in: 'query', schema: { type: 'integer', default: 1 }, description: 'Episode number (required when id is a hex anime ID)' }
          ],
          responses: {
            200: { description: 'Stream data', content: { 'application/json': { schema: { $ref: '#/components/schemas/AnimeStreamResponse' } } } },
            400: { description: 'Missing ID', content: { 'application/json': { schema: { $ref: '#/components/schemas/ErrorResponse' } } } },
            404: { description: 'Stream not found', content: { 'application/json': { schema: { $ref: '#/components/schemas/ErrorResponse' } } } }
          }
        }
      },

      // ── System ────────────────────────────────────────────────
      '/api/config': {
        get: { tags: ['System'], summary: 'Get configuration', responses: { 200: { description: 'Current config', content: { 'application/json': { schema: { $ref: '#/components/schemas/ConfigResponse' } } } } } },
        post: { tags: ['System'], summary: 'Update configuration', requestBody: { content: { 'application/json': { schema: { type: 'object', description: 'Config patch — any field to update' } } } }, responses: { 200: { description: 'Config updated' } } },
      },
      '/api/cache/clear': {
        get: { tags: ['System'], summary: 'Clear cache (GET)', responses: { 200: { description: 'Cache cleared' } } },
        post: { tags: ['System'], summary: 'Clear cache (POST)', responses: { 200: { description: 'Cache cleared' } } },
      },
      '/api/status': { get: { tags: ['System'], summary: 'System metrics and provider status', responses: { 200: { description: 'Status info', content: { 'application/json': { schema: { $ref: '#/components/schemas/StatusResponse' } } } } } } },
      '/api/health': { get: { tags: ['System'], summary: 'Health check', responses: { 200: { description: 'OK', content: { 'application/json': { schema: { type: 'object', properties: { status: { type: 'string', example: 'OK' }, timestamp: { type: 'integer' } } } } } } } } },
      '/api/debug/env': { get: { tags: ['System'], summary: 'Debug environment info', responses: { 200: { description: 'Env details' } } } },
      '/api/auth/password': {
        post: { tags: ['System'], summary: 'Update password', requestBody: { content: { 'application/json': { schema: { type: 'object', required: ['username', 'currentPassword', 'newPassword'], properties: { username: { type: 'string' }, currentPassword: { type: 'string' }, newPassword: { type: 'string' } } } } } }, responses: { 200: { description: 'Password updated' }, 401: { description: 'Invalid credentials' } } },
      },

      // ── Proxy ─────────────────────────────────────────────────
      '/api/proxy': {
        get: {
          tags: ['Proxy'],
          summary: 'Proxy HTTP requests',
          description: 'Proxies M3U8 playlists (rewrites segment URIs), TS segments, and subtitles. Supports custom headers.',
          parameters: [
            { name: 'url', in: 'query', required: true, schema: { type: 'string', format: 'uri' }, description: 'Target URL to proxy' },
            { name: 'headers', in: 'query', schema: { type: 'string', description: 'JSON string of custom headers' } },
          ],
          responses: {
            200: { description: 'Proxied content (binary or rewritten M3U8)' },
            400: { description: 'Missing URL parameter' },
          },
        },
      },

      // ── Auth ──────────────────────────────────────────────────
      '/auth/login': {
        post: {
          tags: ['Auth'],
          summary: 'Login and obtain session cookie',
          requestBody: { content: { 'application/json': { schema: { type: 'object', required: ['username', 'password'], properties: { username: { type: 'string' }, password: { type: 'string' } } } } } },
          responses: {
            200: { description: 'Login successful', headers: { 'Set-Cookie': { schema: { type: 'string' }, description: 'session token cookie' } } },
            401: { description: 'Invalid credentials' },
          },
        },
      },
    },
    components: {
      schemas: {
        Stream: {
          type: 'object',
          properties: {
            title: { type: 'string', example: 'Nova - 1080p (HLS)' },
            url: { type: 'string', format: 'uri' },
            quality: { type: 'string', example: '1080p' },
            format: { type: 'string', enum: ['hls', 'mp4'], example: 'hls' },
            provider: { type: 'string', example: 'Nova' },
            subtitles: { type: 'array', items: { $ref: '#/components/schemas/Subtitle' } },
            headers: { type: 'object', description: 'Required request headers for the stream' },
          },
        },
        Subtitle: {
          type: 'object',
          properties: {
            url: { type: 'string', format: 'uri' },
            lang: { type: 'string', example: 'en' },
            label: { type: 'string', example: 'English' },
          },
        },
        StreamAggregationResponse: {
          type: 'object',
          properties: {
            success: { type: 'boolean', example: true },
            tmdbId: { type: 'string', example: '550' },
            imdbId: { type: 'string', example: 'tt0137523', nullable: true },
            count: { type: 'integer', example: 5 },
            providerTimings: { type: 'object', additionalProperties: { type: 'integer' }, example: { Nova: 3200, Aura: 1800 } },
            streams: { type: 'array', items: { $ref: '#/components/schemas/Stream' } },
            totalDurationMs: { type: 'integer', example: 15200 },
            status: { type: 'string', enum: ['Complete', 'Partial'] },
            fromCache: { type: 'boolean', description: 'Present when results are from cache' },
          },
        },
        DramaStreamResponse: {
          type: 'object',
          properties: {
            success: { type: 'boolean' },
            tmdbId: { type: 'string' },
            type: { type: 'string', example: 'drama' },
            count: { type: 'integer' },
            providerTimings: { type: 'object', additionalProperties: { type: 'integer' } },
            streams: { type: 'array', items: { $ref: '#/components/schemas/Stream' } },
          },
        },
        AnimeServerResponse: {
          type: 'object',
          properties: {
            success: { type: 'boolean', example: true },
            sub: { type: 'array', items: { type: 'object', properties: { id: { type: 'string' }, name: { type: 'string' }, type: { type: 'string' }, format: { type: 'string' } } } },
            dub: { type: 'array', items: { type: 'object', properties: { id: { type: 'string' }, name: { type: 'string' }, type: { type: 'string' }, format: { type: 'string' } } } },
          },
        },
        AnimeStreamResponse: {
          type: 'object',
          properties: {
            success: { type: 'boolean', example: true },
            stream: {
              type: 'object',
              properties: {
                link: { type: 'object', properties: { file: { type: 'string', format: 'uri', example: 'https://api.example.com/m3u8-proxy?h_token=abc123...' }, type: { type: 'string', example: 'hls' } }, description: 'When enableProxy is active, file is an encrypted h_token URL pointing to the proxy. Otherwise it is the raw CDN URL.' },
                tracks: { type: 'array', items: { type: 'object', properties: { file: { type: 'string', format: 'uri', example: 'https://api.example.com/sub-proxy?h_token=def456...' }, label: { type: 'string', example: 'English' }, kind: { type: 'string', example: 'captions' }, default: { type: 'boolean' } } } },
                intro: { type: 'object', properties: { start: { type: 'integer' }, end: { type: 'integer' } }, nullable: true },
                outro: { type: 'object', properties: { start: { type: 'integer' }, end: { type: 'integer' } }, nullable: true },
                server: { type: 'string', example: 'HD-1' },
              },
            },
          },
        },
        ErrorResponse: {
          type: 'object',
          properties: {
            success: { type: 'boolean', example: false },
            error: { type: 'string' },
            message: { type: 'string' },
          },
        },
        ConfigResponse: {
          type: 'object',
          properties: {
            success: { type: 'boolean' },
            config: { type: 'object', description: 'Current configuration' },
            overridePath: { type: 'string' },
            schemaVersion: { type: 'integer' },
          },
        },
        StatusResponse: {
          type: 'object',
          properties: {
            success: { type: 'boolean' },
            version: { type: 'string' },
            uptime: { type: 'integer' },
            metrics: { type: 'object', properties: {
              requestsTotal: { type: 'integer' },
              streamRequests: { type: 'integer' },
              streamsReturned: { type: 'integer' },
              tmdbToImdbLookups: { type: 'integer' },
            } },
            providers: { type: 'array', items: { type: 'object', properties: { name: { type: 'string' }, enabled: { type: 'boolean' } } } },
          },
        },
        SseStreamEvent: {
          type: 'object',
          description: 'SSE event payload. The event type is sent in the `event:` field of the SSE frame.',
          properties: {
            event: { type: 'string', enum: ['cached', 'provider', 'done', 'error'], description: 'SSE event type' },
            provider: { type: 'string', description: 'Provider name (for provider events)', example: 'Cinema' },
            streams: { type: 'array', items: { $ref: '#/components/schemas/Stream' }, description: 'Stream results' },
            count: { type: 'integer', description: 'Number of streams in this event' },
            elapsed_ms: { type: 'integer', description: 'Provider fetch duration in milliseconds' },
            total: { type: 'integer', description: 'Total unique streams across all providers (done event)' },
            cached: { type: 'boolean', description: 'Whether results came from cache' },
            message: { type: 'string', description: 'Error message (error event only)' },
          },
        },
      },
      securitySchemes: {
        cookieAuth: {
          type: 'apiKey',
          in: 'cookie',
          name: 'session',
        },
      },
    },
  };
}

module.exports = buildOpenApiSpec;