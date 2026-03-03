# ---------- Build Stage ----------
FROM node:20-bookworm-slim AS build
ARG VERSION=dev
WORKDIR /app

ENV PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD=1 \
    PUPPETEER_SKIP_CHROMIUM_DOWNLOAD=true

# Install only production dependencies first (leveraging cache)
COPY package.json package-lock.json ./
# Using npm install instead of npm ci because lock file appears out-of-sync
RUN npm install --omit=dev

# Copy only required source
COPY apiServer.js ./
COPY providers ./providers
COPY proxy ./proxy
COPY public ./public
COPY utils ./utils
COPY README.md ./
COPY setup-browser.js ./

# ---------- Runtime Stage ----------
FROM node:20-bookworm-slim AS runtime
ARG VERSION=dev
WORKDIR /app
ENV NODE_ENV=production \
    API_PORT=24608 \
    BIND_HOST=0.0.0.0 \
    APP_VERSION=${VERSION} \
    PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD=1 \
    PUPPETEER_SKIP_CHROMIUM_DOWNLOAD=true \
    BROWSER_EXECUTABLE_PATH=/usr/bin/chromium \
    BROWSER_ARGS="--no-sandbox --disable-setuid-sandbox --disable-dev-shm-usage --no-zygote --single-process --disable-gpu"

# Install a system Chromium so automation providers keep working
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    chromium \
    ffmpeg \
    fonts-liberation \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN groupadd --system app && useradd --system --gid app --create-home app

# Copy node_modules from build and necessary source
COPY --from=build /app/node_modules ./node_modules
COPY --from=build /app/apiServer.js ./
COPY --from=build /app/public ./public
COPY --from=build /app/providers ./providers
COPY --from=build /app/proxy ./proxy
COPY --from=build /app/utils ./utils
COPY --from=build /app/package.json ./
COPY --from=build /app/README.md ./
COPY --from=build /app/setup-browser.js ./

# Seed a writable config directory so a fresh named volume starts with the expected file.
RUN mkdir -p /app/data && printf "{}\n" > /app/data/user-config.json

# Expose port
EXPOSE 24608

# Ensure runtime user owns app directory
RUN chown -R app:app /app
USER app

# Labels / metadata
LABEL org.opencontainers.image.title="TMDB Embed API" \
    org.opencontainers.image.description="Streaming metadata + source aggregation API with Playwright support" \
    org.opencontainers.image.version="${VERSION}" \
    org.opencontainers.image.source="https://github.com/Inside4ndroid/TMDB-Embed-API" \
    org.opencontainers.image.licenses="MIT"

# Healthcheck
HEALTHCHECK --interval=30s --timeout=5s --start-period=20s CMD node -e "fetch('http://127.0.0.1:' + (process.env.API_PORT || 24608) + '/api/health').then((res) => process.exit(res.ok ? 0 : 1)).catch(() => process.exit(1))"

CMD ["node","apiServer.js"]
