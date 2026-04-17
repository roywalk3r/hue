/**
 * Hue Stream API — Comprehensive OpenAPI 3.1.0 Specification
 */
const pkg = require('../package.json');

function buildOpenApiSpec() {
  return {
    openapi: '3.1.0',
    info: {
      title: 'Hue Stream API',
      version: pkg.version,
      description: 'Professional streaming aggregator for Movies, TV Series, Asian Drama, and Live Sports. ' +
        'Most endpoints are public. Config management (POST /api/config, POST /api/auth/password) requires session auth via POST /auth/login. ' +
        'When enableProxy is active, all stream URLs are rewritten through /api/proxy for CORS and region bypass.',
      contact: { name: 'Developer Support' },
    },
    servers: [{ url: '/', description: 'Default Server' }],
    tags: [
      { name: 'Streams', description: 'Universal stream aggregation by TMDB ID.' },
      { name: 'HiTV Discovery', description: 'Browse and play KDrama/CDrama via HiTV.' },
      { name: 'MovieBox', description: 'Curated catalog, search, and direct MP4 streams.' },
      { name: 'Drama', description: 'KissKH and Dramacool extractors.' },
      { name: 'Sports', description: 'Live sports, AI sports, and PPV streams.' },
      { name: 'LiveTV', description: 'Live TV channels and match resolution.' },
      { name: 'System', description: 'Config, health, metrics, and cache management.' },
      { name: 'Proxy', description: 'HTTP proxy for M3U8, TS, and subtitle segments.' },
      { name: 'Auth', description: 'Authentication endpoints.' },
    ],
    paths: {
      // ── Streams ──────────────────────────────────────────────
      '/api/streams/{type}/{tmdbId}': {
        get: {
          tags: ['Streams'],
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
      '/api/streams/drama/{type}/{tmdbId}': {
        get: {
          tags: ['Streams'],
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

      // ── HiTV Discovery ────────────────────────────────────────
      '/api/hitv/browse': {
        get: {
          tags: ['HiTV Discovery'],
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
        get: { tags: ['HiTV Discovery'], summary: 'Latest dramas', parameters: [
          { name: 'page', in: 'query', schema: { type: 'integer', default: 1 } },
          { name: 'country', in: 'query', schema: { type: 'string', default: 'Korea' } },
          { name: 'genre', in: 'query', schema: { type: 'string', default: 'All' } },
        ], responses: { 200: { description: 'Latest results' } } },
      },
      '/api/hitv/trending': {
        get: { tags: ['HiTV Discovery'], summary: 'Trending dramas', parameters: [
          { name: 'page', in: 'query', schema: { type: 'integer', default: 1 } },
          { name: 'country', in: 'query', schema: { type: 'string', default: 'Korea' } },
          { name: 'genre', in: 'query', schema: { type: 'string', default: 'All' } },
        ], responses: { 200: { description: 'Trending results' } } },
      },
      '/api/hitv/search': {
        get: { tags: ['HiTV Discovery'], summary: 'Search HiTV', parameters: [
          { name: 'q', in: 'query', required: true, schema: { type: 'string' }, description: 'Search query' },
          { name: 'type', in: 'query', schema: { type: 'integer', default: 2 } },
          { name: 'page', in: 'query', schema: { type: 'integer', default: 1 } },
        ], responses: { 200: { description: 'Search results' }, 400: { description: 'Missing query' } } },
      },
      '/api/hitv/library': { get: { tags: ['HiTV Discovery'], summary: 'HiTV library data', responses: { 200: { description: 'Library items' } } } },
      '/api/hitv/hero': { get: { tags: ['HiTV Discovery'], summary: 'Hero slider data', responses: { 200: { description: 'Hero items' } } } },
      '/api/hitv/drama': {
        get: { tags: ['HiTV Discovery'], summary: 'Browse dramas with filters', parameters: [
          { name: 'page', in: 'query', schema: { type: 'integer', default: 1 } },
          { name: 'country', in: 'query', schema: { type: 'string', default: 'Korea' } },
          { name: 'genre', in: 'query', schema: { type: 'string', default: 'All' } },
          { name: 'sort', in: 'query', schema: { type: 'string', default: 'Hottest' } },
          { name: 'year', in: 'query', schema: { type: 'integer' } },
        ], responses: { 200: { description: 'Drama browse results' } } },
      },
      '/api/hitv/info/{id}': {
        get: { tags: ['HiTV Discovery'], summary: 'Show details', parameters: [
          { name: 'id', in: 'path', required: true, schema: { type: 'string' }, description: 'HiTV show ID' },
        ], responses: { 200: { description: 'Show details' }, 404: { description: 'Not found' } } },
      },
      '/api/hitv/countries': { get: { tags: ['HiTV Discovery'], summary: 'Available countries', responses: { 200: { description: 'Country list' } } } },
      '/api/hitv/categories': { get: { tags: ['HiTV Discovery'], summary: 'Available categories', responses: { 200: { description: 'Category list' } } } },
      '/api/hitv/genres': { get: { tags: ['HiTV Discovery'], summary: 'Available genres', responses: { 200: { description: 'Genre list' } } } },
      '/api/hitv/sorts': { get: { tags: ['HiTV Discovery'], summary: 'Available sort options', responses: { 200: { description: 'Sort options' } } } },
      '/api/hitv/play/{id}/{episode}': {
        get: { tags: ['HiTV Discovery'], summary: 'Smart Play — aggregate streams', parameters: [
          { name: 'id', in: 'path', required: true, schema: { type: 'string' }, description: 'HiTV ID or slug' },
          { name: 'episode', in: 'path', required: true, schema: { type: 'integer' } },
          { name: 'title', in: 'query', schema: { type: 'string' } },
        ], responses: { 200: { description: 'Stream results' } } },
      },
      '/api/hitv/{country}/{sort}/{genre}': {
        get: { tags: ['HiTV Discovery'], summary: 'Dynamic discovery', parameters: [
          { name: 'country', in: 'path', required: true, schema: { type: 'string' } },
          { name: 'sort', in: 'path', schema: { type: 'string' } },
          { name: 'genre', in: 'path', schema: { type: 'string' } },
          { name: 'page', in: 'query', schema: { type: 'integer', default: 1 } },
          { name: 'year', in: 'query', schema: { type: 'integer' } },
        ], responses: { 200: { description: 'Discovery results' } } },
      },

      // ── MovieBox ──────────────────────────────────────────────
      '/api/moviebox/home': { get: { tags: ['MovieBox'], summary: 'Home page data', responses: { 200: { description: 'Home sections' } } } },
      '/api/moviebox/discover': {
        get: { tags: ['MovieBox'], summary: 'Discover content', parameters: [
          { name: 'channelId', in: 'query', schema: { type: 'integer' } },
        ], responses: { 200: { description: 'Discover sections or filtered data' } } },
      },
      '/api/moviebox/section': {
        get: { tags: ['MovieBox'], summary: 'Get catalog row', parameters: [
          { name: 'title', in: 'query', required: true, schema: { type: 'string' }, example: 'Popular Series' },
        ], responses: { 200: { description: 'Section items' }, 400: { description: 'Missing title' } } },
      },
      '/api/moviebox/trending': { get: { tags: ['MovieBox'], summary: 'Trending content', responses: { 200: { description: 'Trending items' } } } },
      '/api/moviebox/popular-movie': { get: { tags: ['MovieBox'], summary: 'Popular movies', responses: { 200: { description: 'Popular movies' } } } },
      '/api/moviebox/popular-series': { get: { tags: ['MovieBox'], summary: 'Popular series', responses: { 200: { description: 'Popular series' } } } },
      '/api/moviebox/hot-shorts': { get: { tags: ['MovieBox'], summary: 'Hot shorts', responses: { 200: { description: 'Hot shorts' } } } },
      '/api/moviebox/sa': { get: { tags: ['MovieBox'], summary: 'SA Drama', responses: { 200: { description: 'SA Drama items' } } } },
      '/api/moviebox/thai': { get: { tags: ['MovieBox'], summary: 'Thai Drama', responses: { 200: { description: 'Thai Drama items' } } } },
      '/api/moviebox/turkish': { get: { tags: ['MovieBox'], summary: 'Turkish Drama', responses: { 200: { description: 'Turkish Drama items' } } } },
      '/api/moviebox/western': { get: { tags: ['MovieBox'], summary: 'Western TV', responses: { 200: { description: 'Western TV items' } } } },
      '/api/moviebox/black-shows': { get: { tags: ['MovieBox'], summary: 'Must-watch Black Shows', responses: { 200: { description: 'Black shows' } } } },
      '/api/moviebox/hero': { get: { tags: ['MovieBox'], summary: 'Hero slider', responses: { 200: { description: 'Hero items with trailers' } } } },
      '/api/moviebox/bollywood': {
        get: { tags: ['MovieBox'], summary: 'Bollywood content', parameters: [
          { name: 'type', in: 'query', schema: { type: 'string', default: 'movie' } },
          { name: 'page', in: 'query', schema: { type: 'integer', default: 1 } },
        ], responses: { 200: { description: 'Bollywood items' } } },
      },
      '/api/moviebox/shorts': {
        get: { tags: ['MovieBox'], summary: 'Shorts', parameters: [
          { name: 'page', in: 'query', schema: { type: 'integer', default: 1 } },
        ], responses: { 200: { description: 'Shorts' } } },
      },
      '/api/moviebox/filter': {
        get: { tags: ['MovieBox'], summary: 'Advanced filter search', parameters: [
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
        get: { tags: ['MovieBox'], summary: 'Content detail info', parameters: [
          { name: 'path', in: 'query', required: true, schema: { type: 'string' }, description: 'MovieBox content path' },
        ], responses: { 200: { description: 'Content details' }, 400: { description: 'Missing path' } } },
      },
      '/api/moviebox/search': {
        get: { tags: ['MovieBox'], summary: 'Search catalog', parameters: [
          { name: 'keyword', in: 'query', required: true, schema: { type: 'string' } },
          { name: 'page', in: 'query', schema: { type: 'integer', default: 1 } },
          { name: 'type', in: 'query', schema: { type: 'integer', default: 0 } },
        ], responses: { 200: { description: 'Search results' }, 400: { description: 'Missing keyword' } } },
      },
      '/api/moviebox/play/{id}/{path}': {
        get: { tags: ['MovieBox'], summary: 'Extract direct streams', description: 'Wildcard path after ID. Supports season/episode for series.', parameters: [
          { name: 'id', in: 'path', required: true, schema: { type: 'string' } },
          { name: 'path', in: 'path', required: true, schema: { type: 'string' }, description: 'Remaining path (can contain slashes)' },
          { name: 'season', in: 'query', schema: { type: 'integer' } },
          { name: 'episode', in: 'query', schema: { type: 'integer' } },
        ], responses: { 200: { description: 'Stream results with captions' } } },
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