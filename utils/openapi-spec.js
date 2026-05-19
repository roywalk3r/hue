/**
 * HUE ENGINE ⬢
 * High-Performance Stream Aggregation Engine
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */

/**
 * HUE ENGINE ⬢ — OpenAPI 3.1.0 Specification
 * Accurate to Rust backend routes in huerust/src/app.rs
 */
const pkg = require('../package.json');

function buildOpenApiSpec(req) {
  const host = req?.headers?.host || process.env.API_DOMAIN || '187.124.19.247';
  const protocol = req?.headers?.['x-forwarded-proto'] === 'https' || req?.socket?.encrypted ? 'https' : 'http';
  const serverUrl = `${protocol}://${host}`;

  return {
    openapi: '3.1.0',
    info: {
      title: 'HUE ENGINE ⬢',
      version: 'v1.1.1',
      description: 'OpenAPI 3.1.0 — Unified API Gateway: nginx routes requests to Rust (streams, proxy, browse) or Node.js (sports, auth, docs).',
      contact: { name: 'System Administrator' },
    },
    servers: [
      { url: serverUrl, description: 'Unified API Gateway (nginx → Rust + Node.js)' },
    ],
    tags: [
      { name: 'Streams', description: 'Real-time stream aggregation, SSE, and smart play.' },
      { name: 'Discovery', description: 'Content browsing — MovieBox, HiTV, TMDB.' },
      { name: 'Drama', description: 'KissKH, Dramacool, and MovieBox drama sources.' },
      { name: 'Anime', description: 'HiAnime discovery and streaming.' },
      { name: 'Sports', description: 'Live sports and PPV event streams.' },
      { name: 'LiveTV', description: 'Linear broadcast channel management.' },
      { name: 'System', description: 'Health, metrics, status, and admin.' },
      { name: 'Proxy', description: 'Secure HLS, TS, and subtitle egress routing.' },
      { name: 'Auth', description: 'Session management and access control.' },
    ],
    paths: {
      // ═══════════════════════════════════════════════════════════
      //  STREAMS
      // ═══════════════════════════════════════════════════════════
      '/api/streams/{type}/{tmdbId}': {
        get: {
          tags: ['Streams'],
          summary: 'Aggregate streams across all providers',
          description: 'Parallel fetch from all registered providers with 15s deadline. Returns complete or partial results.',
          parameters: [
            { name: 'type', in: 'path', required: true, schema: { type: 'string', enum: ['movie', 'series'] } },
            { name: 'tmdbId', in: 'path', required: true, schema: { type: 'string' } },
            { name: 'season', in: 'query', schema: { type: 'integer' } },
            { name: 'episode', in: 'query', schema: { type: 'integer' } },
            { name: 'nocache', in: 'query', schema: { type: 'string', enum: ['0', '1'] } },
            { name: 'title', in: 'query', schema: { type: 'string' } },
            { name: 'dp', in: 'query', schema: { type: 'string' }, description: 'detailPath for provider lookup' },
          ],
          responses: {
            200: { description: 'Stream aggregation results' },
            400: { description: 'Invalid type' },
            500: { description: 'Internal error' },
          },
        },
      },
      '/api/streams/{type}/{tmdbId}/live': {
        get: {
          tags: ['Streams'],
          summary: 'Live SSE stream aggregation',
          description: 'Server-Sent Events stream. Events: cached, provider, done, error.',
          parameters: [
            { name: 'type', in: 'path', required: true, schema: { type: 'string', enum: ['movie', 'series'] } },
            { name: 'tmdbId', in: 'path', required: true, schema: { type: 'string' } },
            { name: 'season', in: 'query', schema: { type: 'integer' } },
            { name: 'episode', in: 'query', schema: { type: 'integer' } },
            { name: 'nocache', in: 'query', schema: { type: 'string', enum: ['0', '1'] } },
            { name: 'title', in: 'query', schema: { type: 'string' } },
            { name: 'dp', in: 'query', schema: { type: 'string' } },
          ],
          responses: {
            200: { description: 'SSE text/event-stream' },
            400: { description: 'Invalid type' },
          },
        },
      },
      '/api/play/{type}/{tmdbId}': {
        get: {
          tags: ['Streams'],
          summary: 'Smart Play — single best stream',
          description: 'Returns the single best working proxied stream. Ideal for direct player integration.',
          parameters: [
            { name: 'type', in: 'path', required: true, schema: { type: 'string', enum: ['movie', 'series'] } },
            { name: 'tmdbId', in: 'path', required: true, schema: { type: 'string' } },
            { name: 'season', in: 'query', schema: { type: 'integer', default: 1 } },
            { name: 'episode', in: 'query', schema: { type: 'integer', default: 1 } },
            { name: 'redirect', in: 'query', schema: { type: 'string', enum: ['0', '1'] }, description: 'Set 1 to 302 redirect directly to stream URL' },
          ],
          responses: {
            200: { description: 'Best stream found' },
            302: { description: 'Redirect to stream URL' },
            404: { description: 'No streams found' },
          },
        },
      },
      '/api/providers': {
        get: {
          tags: ['Streams'],
          summary: 'List registered providers',
          responses: {
            200: { description: 'Provider list with status' },
          },
        },
      },

      // ═══════════════════════════════════════════════════════════
      //  DISCOVERY — MovieBox
      // ═══════════════════════════════════════════════════════════
      '/api/moviebox/home': { get: { tags: ['Discovery'], summary: 'Home page sections', responses: { 200: { description: 'Home rails' } } } },
      '/api/moviebox/hero': { get: { tags: ['Discovery'], summary: 'Hero slider content', responses: { 200: { description: 'Hero items with trailers' } } } },
      '/api/moviebox/discover': {
        get: { tags: ['Discovery'], summary: 'Discover content', parameters: [
          { name: 'channelId', in: 'query', schema: { type: 'integer' } },
        ], responses: { 200: { description: 'Discover sections' } } },
      },
      '/api/moviebox/section': {
        get: { tags: ['Discovery'], summary: 'Get catalog row by title', parameters: [
          { name: 'title', in: 'query', required: true, schema: { type: 'string' } },
        ], responses: { 200: { description: 'Section items' }, 400: { description: 'Missing title' } } },
      },
      '/api/moviebox/trending': { get: { tags: ['Discovery'], summary: 'Trending content', responses: { 200: { description: 'Trending items' } } } },
      '/api/moviebox/popular-movie': { get: { tags: ['Discovery'], summary: 'Popular movies', responses: { 200: { description: 'Popular movies' } } } },
      '/api/moviebox/popular-series': { get: { tags: ['Discovery'], summary: 'Popular series', responses: { 200: { description: 'Popular series' } } } },
      '/api/moviebox/hot-shorts': { get: { tags: ['Discovery'], summary: 'Hot shorts', responses: { 200: { description: 'Hot shorts' } } } },
      '/api/moviebox/sa': { get: { tags: ['Discovery'], summary: 'SA Drama', responses: { 200: { description: 'SA Drama items' } } } },
      '/api/moviebox/thai': { get: { tags: ['Discovery'], summary: 'Thai Drama', responses: { 200: { description: 'Thai Drama items' } } } },
      '/api/moviebox/turkish': { get: { tags: ['Discovery'], summary: 'Turkish Drama', responses: { 200: { description: 'Turkish Drama items' } } } },
      '/api/moviebox/western': { get: { tags: ['Discovery'], summary: 'Western TV', responses: { 200: { description: 'Western TV items' } } } },
      '/api/moviebox/animation': { get: { tags: ['Discovery'], summary: 'Animation', responses: { 200: { description: 'Animation items' } } } },
      '/api/moviebox/black-shows': { get: { tags: ['Discovery'], summary: 'Black Shows', responses: { 200: { description: 'Black shows' } } } },
      '/api/moviebox/bollywood': {
        get: { tags: ['Discovery'], summary: 'Bollywood content', parameters: [
          { name: 'type', in: 'query', schema: { type: 'string', default: 'movie' } },
          { name: 'page', in: 'query', schema: { type: 'integer', default: 1 } },
        ], responses: { 200: { description: 'Bollywood items' } } },
      },
      '/api/moviebox/shorts': {
        get: { tags: ['Discovery'], summary: 'Shorts', parameters: [
          { name: 'page', in: 'query', schema: { type: 'integer', default: 1 } },
        ], responses: { 200: { description: 'Shorts' } } },
      },
      '/api/moviebox/filter': {
        get: { tags: ['Discovery'], summary: 'Advanced filter search', parameters: [
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
        get: { tags: ['Discovery'], summary: 'Content detail', parameters: [
          { name: 'path', in: 'query', required: true, schema: { type: 'string' } },
        ], responses: { 200: { description: 'Content details' }, 400: { description: 'Missing path' } } },
      },
      '/api/moviebox/search': {
        get: { tags: ['Discovery'], summary: 'Search MovieBox catalog', parameters: [
          { name: 'keyword', in: 'query', required: true, schema: { type: 'string' } },
          { name: 'page', in: 'query', schema: { type: 'integer', default: 1 } },
          { name: 'type', in: 'query', schema: { type: 'integer', default: 0 } },
        ], responses: { 200: { description: 'Search results' }, 400: { description: 'Missing keyword' } } },
      },

      // ═══════════════════════════════════════════════════════════
      //  DISCOVERY — HiTV
      // ═══════════════════════════════════════════════════════════
      '/api/hitv/browse': {
        get: { tags: ['Discovery'], summary: 'Browse HiTV library', parameters: [
          { name: 'page', in: 'query', schema: { type: 'integer', default: 1 } },
          { name: 'pageSize', in: 'query', schema: { type: 'integer', default: 28 } },
          { name: 'country', in: 'query', schema: { type: 'string', default: 'Korea' } },
          { name: 'category', in: 'query', schema: { type: 'string', default: 'Drama' } },
          { name: 'genre', in: 'query', schema: { type: 'string', default: 'All' } },
          { name: 'sort', in: 'query', schema: { type: 'string', default: 'Hottest' } },
        ], responses: { 200: { description: 'Browse results' } } },
      },
      '/api/hitv/latest': {
        get: { tags: ['Discovery'], summary: 'Latest dramas', parameters: [
          { name: 'page', in: 'query', schema: { type: 'integer', default: 1 } },
          { name: 'country', in: 'query', schema: { type: 'string', default: 'Korea' } },
          { name: 'genre', in: 'query', schema: { type: 'string', default: 'All' } },
        ], responses: { 200: { description: 'Latest results' } } },
      },
      '/api/hitv/trending': {
        get: { tags: ['Discovery'], summary: 'Trending dramas', parameters: [
          { name: 'page', in: 'query', schema: { type: 'integer', default: 1 } },
          { name: 'country', in: 'query', schema: { type: 'string', default: 'Korea' } },
          { name: 'genre', in: 'query', schema: { type: 'string', default: 'All' } },
        ], responses: { 200: { description: 'Trending results' } } },
      },
      '/api/hitv/hero': { get: { tags: ['Discovery'], summary: 'Hero slider data', responses: { 200: { description: 'Hero items' } } } },
      '/api/hitv/library': { get: { tags: ['Discovery'], summary: 'HiTV library data', responses: { 200: { description: 'Library items' } } } },
      '/api/hitv/search': {
        get: { tags: ['Discovery'], summary: 'Search HiTV', parameters: [
          { name: 'q', in: 'query', required: true, schema: { type: 'string' } },
          { name: 'type', in: 'query', schema: { type: 'integer', default: 2 } },
          { name: 'page', in: 'query', schema: { type: 'integer', default: 1 } },
        ], responses: { 200: { description: 'Search results' }, 400: { description: 'Missing query' } } },
      },
      '/api/hitv/drama': {
        get: { tags: ['Discovery'], summary: 'Browse dramas with filters', parameters: [
          { name: 'page', in: 'query', schema: { type: 'integer', default: 1 } },
          { name: 'country', in: 'query', schema: { type: 'string', default: 'Korea' } },
          { name: 'genre', in: 'query', schema: { type: 'string', default: 'All' } },
          { name: 'sort', in: 'query', schema: { type: 'string', default: 'Hottest' } },
          { name: 'year', in: 'query', schema: { type: 'integer' } },
        ], responses: { 200: { description: 'Drama browse results' } } },
      },
      '/api/hitv/info/{id}': {
        get: { tags: ['Discovery'], summary: 'Show details', parameters: [
          { name: 'id', in: 'path', required: true, schema: { type: 'string' } },
        ], responses: { 200: { description: 'Show details' }, 404: { description: 'Not found' } } },
      },
      '/api/hitv/countries': { get: { tags: ['Discovery'], summary: 'Available countries', responses: { 200: { description: 'Country list' } } } },
      '/api/hitv/categories': { get: { tags: ['Discovery'], summary: 'Available categories', responses: { 200: { description: 'Category list' } } } },
      '/api/hitv/genres': { get: { tags: ['Discovery'], summary: 'Available genres', responses: { 200: { description: 'Genre list' } } } },
      '/api/hitv/sorts': { get: { tags: ['Discovery'], summary: 'Available sort options', responses: { 200: { description: 'Sort options' } } } },
      '/api/hitv/play/{id}/{episode}': {
        get: { tags: ['Discovery'], summary: 'Smart Play — aggregate streams', parameters: [
          { name: 'id', in: 'path', required: true, schema: { type: 'string' } },
          { name: 'episode', in: 'path', required: true, schema: { type: 'integer' } },
          { name: 'title', in: 'query', schema: { type: 'string' } },
        ], responses: { 200: { description: 'Stream results' } } },
      },
      '/api/hitv/{country}/{sort}/{genre}': {
        get: { tags: ['Discovery'], summary: 'Dynamic discovery', parameters: [
          { name: 'country', in: 'path', required: true, schema: { type: 'string' } },
          { name: 'sort', in: 'path', schema: { type: 'string' } },
          { name: 'genre', in: 'path', schema: { type: 'string' } },
          { name: 'page', in: 'query', schema: { type: 'integer', default: 1 } },
          { name: 'year', in: 'query', schema: { type: 'integer' } },
        ], responses: { 200: { description: 'Discovery results' } } },
      },

      // ═══════════════════════════════════════════════════════════
      //  DRAMA — KissKH
      // ═══════════════════════════════════════════════════════════
      '/api/drama/kisskh/search': {
        get: { tags: ['Drama'], summary: 'Search KissKH', parameters: [
          { name: 'q', in: 'query', required: true, schema: { type: 'string' } },
        ], responses: { 200: { description: 'Search results' }, 400: { description: 'Missing query' } } },
      },
      '/api/drama/kisskh/show': {
        get: { tags: ['Drama'], summary: 'KissKH show list', responses: { 200: { description: 'Show list' } } },
      },
      '/api/drama/kisskh/browse': {
        get: { tags: ['Drama'], summary: 'Browse KissKH catalog', parameters: [
          { name: 'page', in: 'query', schema: { type: 'integer', default: 1 } },
          { name: 'pageSize', in: 'query', schema: { type: 'integer', default: 28 } },
          { name: 'contentType', in: 'query', schema: { type: 'integer', default: 0 } },
          { name: 'sub', in: 'query', schema: { type: 'integer', default: 0 } },
          { name: 'country', in: 'query', schema: { type: 'integer', default: 0 } },
          { name: 'status', in: 'query', schema: { type: 'integer', default: 0 } },
          { name: 'order', in: 'query', schema: { type: 'integer', default: 2 } },
        ], responses: { 200: { description: 'Browse results' } } },
      },
      '/api/drama/kisskh/upcoming': {
        get: { tags: ['Drama'], summary: 'Upcoming KissKH dramas', responses: { 200: { description: 'Upcoming list' } } },
      },
      '/api/drama/kisskh/top-rating': {
        get: { tags: ['Drama'], summary: 'Top rated KissKH dramas', responses: { 200: { description: 'Top rated list' } } },
      },
      '/api/drama/kisskh/streams/{id}/{episode}': {
        get: { tags: ['Drama'], summary: 'KissKH streams', parameters: [
          { name: 'id', in: 'path', required: true, schema: { type: 'string' } },
          { name: 'episode', in: 'path', schema: { type: 'integer', default: 1 } },
          { name: 'title', in: 'query', schema: { type: 'string' } },
        ], responses: { 200: { description: 'Stream results' } } },
      },

      // ═══════════════════════════════════════════════════════════
      //  DRAMA — Dramacool
      // ═══════════════════════════════════════════════════════════
      '/api/drama/dramacool/search': {
        get: { tags: ['Drama'], summary: 'Search Dramacool', parameters: [
          { name: 'q', in: 'query', required: true, schema: { type: 'string' } },
          { name: 'page', in: 'query', schema: { type: 'integer', default: 1 } },
        ], responses: { 200: { description: 'Search results' }, 400: { description: 'Missing query' } } },
      },
      '/api/drama/dramacool/streams/{id}/{episode}': {
        get: { tags: ['Drama'], summary: 'Dramacool streams', parameters: [
          { name: 'id', in: 'path', required: true, schema: { type: 'string' }, description: 'Dramacool path slug' },
          { name: 'episode', in: 'path', required: true, schema: { type: 'integer' } },
        ], responses: { 200: { description: 'Stream results' } } },
      },

      // ═══════════════════════════════════════════════════════════
      //  DRAMA — MovieBox
      // ═══════════════════════════════════════════════════════════
      '/api/drama/moviebox/browse': {
        get: { tags: ['Drama'], summary: 'Browse MovieBox drama rails', parameters: [
          { name: 'page', in: 'query', schema: { type: 'integer', default: 1 } },
          { name: 'perPage', in: 'query', schema: { type: 'integer', default: 28 } },
          { name: 'country', in: 'query', schema: { type: 'string' } },
        ], responses: { 200: { description: 'Rails data' } } },
      },
      '/api/drama/moviebox/search': {
        get: { tags: ['Drama'], summary: 'Search MovieBox drama', parameters: [
          { name: 'q', in: 'query', required: true, schema: { type: 'string' } },
          { name: 'page', in: 'query', schema: { type: 'integer', default: 1 } },
          { name: 'subjectType', in: 'query', schema: { type: 'integer', default: 0 } },
        ], responses: { 200: { description: 'Search results' } } },
      },
      '/api/drama/moviebox/streams/{title}/{episode}': {
        get: { tags: ['Drama'], summary: 'MovieBox drama streams', parameters: [
          { name: 'title', in: 'path', required: true, schema: { type: 'string' } },
          { name: 'episode', in: 'path', required: true, schema: { type: 'integer' } },
        ], responses: { 200: { description: 'Stream results' } } },
      },

      // ═══════════════════════════════════════════════════════════
      //  DRAMA — Unified
      // ═══════════════════════════════════════════════════════════
      '/api/drama/detail': {
        get: { tags: ['Drama'], summary: 'Unified drama detail', parameters: [
          { name: 'q', in: 'query', required: true, schema: { type: 'string' }, description: 'Title to search' },
          { name: 'id', in: 'query', schema: { type: 'string' }, description: 'Numeric KissKH ID (optional)' },
        ], responses: { 200: { description: 'Detail with episodes' }, 404: { description: 'Not found' } } },
      },

      // ═══════════════════════════════════════════════════════════
      //  ANIME
      // ═══════════════════════════════════════════════════════════
      '/api/anime/home': {
        get: { tags: ['Anime'], summary: 'Anime home feed', responses: { 200: { description: 'Home data with featured, trending, latest' } } },
      },
      '/api/anime/search': {
        get: { tags: ['Anime'], summary: 'Search anime', parameters: [
          { name: 'q', in: 'query', required: true, schema: { type: 'string' } },
          { name: 'page', in: 'query', schema: { type: 'integer', default: 1 } },
        ], responses: { 200: { description: 'Search results' }, 400: { description: 'Missing query' } } },
      },
      '/api/anime/latest': {
        get: { tags: ['Anime'], summary: 'Latest updated anime', parameters: [
          { name: 'page', in: 'query', schema: { type: 'integer', default: 1 } },
        ], responses: { 200: { description: 'Latest anime list' } } },
      },
      '/api/anime/category': {
        get: { tags: ['Anime'], summary: 'Anime by category', parameters: [
          { name: 'type', in: 'query', schema: { type: 'string', enum: ['tv', 'movie', 'ova', 'ona'], default: 'tv' } },
          { name: 'page', in: 'query', schema: { type: 'integer', default: 1 } },
          { name: 'limit', in: 'query', schema: { type: 'integer', default: 20 } },
        ], responses: { 200: { description: 'Category items' } } },
      },
      '/api/anime/detail/{slug}': {
        get: { tags: ['Anime'], summary: 'Anime detail', parameters: [
          { name: 'slug', in: 'path', required: true, schema: { type: 'string' }, description: 'Anime slug or hex ID' },
        ], responses: { 200: { description: 'Anime detail' }, 404: { description: 'Not found' } } },
      },
      '/api/anime/episodes/{id}': {
        get: { tags: ['Anime'], summary: 'Episode list', parameters: [
          { name: 'id', in: 'path', required: true, schema: { type: 'string' }, description: 'Anime hex ID' },
          { name: 'start', in: 'query', schema: { type: 'integer', default: 0 }, description: 'Start episode for pagination' },
        ], responses: { 200: { description: 'Episode list' }, 404: { description: 'Not found' } } },
      },
      '/api/anime/servers': {
        get: { tags: ['Anime'], summary: 'Episode servers', parameters: [
          { name: 'id', in: 'query', required: true, schema: { type: 'string' }, description: 'Episode slug' },
        ], responses: { 200: { description: 'Server list (sub/dub)' }, 400: { description: 'Missing ID' } } },
      },
      '/api/anime/stream': {
        get: { tags: ['Anime'], summary: 'Direct anime stream', parameters: [
          { name: 'id', in: 'query', required: true, schema: { type: 'string' }, description: 'Hex ID or episode slug' },
          { name: 'server', in: 'query', schema: { type: 'string' }, description: 'Target server (e.g. HD-1)' },
          { name: 'type', in: 'query', schema: { type: 'string', enum: ['sub', 'dub'], default: 'sub' }, description: 'Audio type' },
          { name: 'episode', in: 'query', schema: { type: 'integer', default: 1 }, description: 'Episode number (for hex ID)' },
        ], responses: { 200: { description: 'Stream with tracks and skip ranges' }, 400: { description: 'Missing ID' }, 404: { description: 'Stream not found' } } },
      },
      '/api/anime/filter': {
        post: {
          tags: ['Anime'],
          summary: 'Filter anime catalog',
          description: 'Advanced filtering with full facet support. Pass any combination of filters. Use "All" to skip a facet.',
          requestBody: {
            content: {
              'application/json': {
                schema: {
                  type: 'object',
                  properties: {
                    type: { type: 'string', enum: ['All','TV','Movie','OVA','Special','ONA','Music','Horror'], example: 'TV' },
                    status: { type: 'string', enum: ['All','Airing','Completed','Upcoming'], example: 'Completed' },
                    rated: { type: 'string', enum: ['All','G - All Ages','PG - Children','PG-13 - Teens','R - 17+','R+ - Mild Nudity','Rx - Hentai'], example: 'PG-13 - Teens' },
                    score: { type: 'string', enum: ['All','1','2','3','4','5','6','7','8','9','10'], example: '7' },
                    season: { type: 'string', enum: ['All','Winter','Spring','Summer','Fall'], example: 'Spring' },
                    language: { type: 'string', enum: ['All','English','Japanese','Chinese','Korean'], example: 'Japanese' },
                    sort: { type: 'string', enum: ['Default','Popularity','Score','Alphabetical'], example: 'Popularity' },
                    genres: { type: 'array', items: { type: 'string', enum: ['Action','Adventure','Cars','Comedy','Dementia','Demons','Drama','Ecchi','Fantasy','Game','Harem','Historical','Horror','Isekai','Josei','Kids','Magic','Martial Arts','Mecha','Military','Music','Mystery','Parody','Police','Psychological','Romance','Samurai','School','Sci-Fi','Seinen','Shoujo','Shoujo Ai','Shounen','Shounen Ai','Slice of Life','Space','Sports','Super Power','Supernatural','Thriller','Vampire'] }, example: ['Action','Fantasy'] },
                    startDate: { type: 'string', format: 'date', example: '2024-01-01' },
                    endDate: { type: 'string', format: 'date', example: '2024-12-31' },
                    page: { type: 'integer', default: 1 },
                    limit: { type: 'integer', default: 20 },
                  },
                },
                example: {
                  type: 'OVA',
                  status: 'All',
                  rated: 'All',
                  score: 'All',
                  season: 'All',
                  language: 'All',
                  sort: 'Default',
                  genres: [],
                  page: 1,
                  limit: 20,
                },
              },
            },
          },
          responses: {
            200: {
              description: 'Paginated filter results',
              content: {
                'application/json': {
                  schema: {
                    type: 'object',
                    properties: {
                      total: { type: 'integer', example: 51 },
                      page: { type: 'integer', example: 1 },
                      limit: { type: 'integer', example: 20 },
                      totalPages: { type: 'integer', example: 3 },
                      results: { type: 'array', items: { type: 'object' } },
                    },
                  },
                },
              },
            },
            500: { description: 'Filter failed' },
          },
        },
      },

      // ═══════════════════════════════════════════════════════════
      //  SPORTS (Node.js)
      // ═══════════════════════════════════════════════════════════
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
          { name: 'matchName', in: 'path', required: true, schema: { type: 'string' } },
        ], responses: { 200: { description: 'PPV stream results' } } },
      },

      // ═══════════════════════════════════════════════════════════
      //  LIVETV (Node.js)
      // ═══════════════════════════════════════════════════════════
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
        get: { tags: ['LiveTV'], summary: 'Resolve match streams', parameters: [
          { name: 'url', in: 'query', required: true, schema: { type: 'string' } },
          { name: 'resolve', in: 'query', schema: { type: 'string', enum: ['true', 'false'] } },
          { name: 'stream', in: 'query', schema: { type: 'string', enum: ['true', 'false'] }, description: 'Enable SSE mode' },
        ], responses: { 200: { description: 'Stream results (JSON or SSE)' } } },
      },

      // ═══════════════════════════════════════════════════════════
      //  SYSTEM
      // ═══════════════════════════════════════════════════════════
      '/api/status': { get: { tags: ['System'], summary: 'System metrics and provider status', responses: { 200: { description: 'Status info' } } } },
      '/api/health': { get: { tags: ['System'], summary: 'Health check', responses: { 200: { description: 'OK' } } } },
      '/api/metrics': { get: { tags: ['System'], summary: 'Prometheus metrics', responses: { 200: { description: 'Metrics text' } } } },
      '/api/admin/reload': { post: { tags: ['System'], summary: 'Reload configuration', responses: { 200: { description: 'Reloaded' } } } },
      '/api/admin/refresh-tokens': { post: { tags: ['System'], summary: 'Refresh proxy tokens', responses: { 200: { description: 'Tokens refreshed' } } } },

      // ═══════════════════════════════════════════════════════════
      //  PROXY
      // ═══════════════════════════════════════════════════════════
      '/m3u8-proxy': {
        get: {
          tags: ['Proxy'],
          summary: 'Proxy and rewrite HLS playlists',
          description: 'Decrypts h_token, fetches M3U8, rewrites segment URLs and key URIs, sanitizes content.',
          parameters: [
            { name: 'h_token', in: 'query', required: true, schema: { type: 'string' } },
          ],
          responses: { 200: { description: 'Rewritten M3U8 playlist' }, 400: { description: 'Missing h_token' } },
        },
      },
      '/ts-proxy': {
        get: {
          tags: ['Proxy'],
          summary: 'Proxy TS segments and MP4 files',
          parameters: [
            { name: 'h_token', in: 'query', required: true, schema: { type: 'string' } },
          ],
          responses: { 200: { description: 'Binary segment data' }, 400: { description: 'Missing h_token' } },
        },
      },
      '/sub-proxy': {
        get: {
          tags: ['Proxy'],
          summary: 'Proxy subtitle files',
          parameters: [
            { name: 'h_token', in: 'query', required: true, schema: { type: 'string' } },
          ],
          responses: { 200: { description: 'Subtitle content (SRT/VTT/ASS)' }, 400: { description: 'Missing h_token' } },
        },
      },

      // ═══════════════════════════════════════════════════════════
      //  AUTH (Node.js)
      // ═══════════════════════════════════════════════════════════
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
        ErrorResponse: {
          type: 'object',
          properties: {
            success: { type: 'boolean', example: false },
            error: { type: 'string' },
          },
        },
      },
      securitySchemes: {
        cookieAuth: { type: 'apiKey', in: 'cookie', name: 'session' },
      },
    },
  };
}

module.exports = buildOpenApiSpec;
