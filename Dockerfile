# ---------- Build Stage ----------
FROM node:20-alpine AS build
ARG VERSION=dev
WORKDIR /app

# Install only production dependencies first (leveraging cache)
COPY package.json package-lock.json ./
# Using npm install instead of npm ci because lock file appears out-of-sync
# If you later regenerate lock (npm install locally) you can revert to npm ci for reproducibility
RUN npm install --omit=dev

# Install Chrome for Puppeteer
RUN apk add --no-cache \
    chromium \
    nss \
    freetype \
    freetype-dev \
    harfbuzz \
    ca-certificates \
    ttf-freefont && \
    rm -rf /var/cache/apk/*

# Tell Puppeteer to skip installing Chromium since we installed it manually
ENV PUPPETEER_SKIP_CHROMIUM_DOWNLOAD=true \
    PUPPETEER_EXECUTABLE_PATH=/usr/bin/chromium-browser

# Copy only required source (avoid sending screenshots, node_modules already installed)
COPY apiServer.js ./
COPY providers ./providers
COPY public ./public
COPY utils ./utils
COPY README.md ./

# ---------- Runtime Stage ----------
FROM node:20-alpine AS runtime
ARG VERSION=dev
WORKDIR /app
ENV NODE_ENV=production \
    API_PORT=8787 \
    BIND_HOST=0.0.0.0 \
    APP_VERSION=${VERSION}

# Install Chrome for Puppeteer in runtime stage
RUN apk add --no-cache \
    chromium \
    nss \
    freetype \
    freetype-dev \
    harfbuzz \
    ca-certificates \
    ttf-freefont && \
    rm -rf /var/cache/apk/*

# Tell Puppeteer to skip installing Chromium since we installed it manually
ENV PUPPETEER_SKIP_CHROMIUM_DOWNLOAD=true \
    PUPPETEER_EXECUTABLE_PATH=/usr/bin/chromium-browser

# Create non-root user
RUN addgroup -S app && adduser -S app -G app

# Copy node_modules from build and necessary source
COPY --from=build /app/node_modules ./node_modules
COPY --from=build /app/apiServer.js ./
COPY --from=build /app/public ./public
COPY --from=build /app/providers ./providers
COPY --from=build /app/utils ./utils
COPY --from=build /app/package.json ./
COPY --from=build /app/README.md ./

# Expose port (documentational; runtime can override)
EXPOSE 8787

# Ensure runtime user owns app directory for writes (overrides, restart marker)
RUN chown -R app:app /app
USER app

# Labels / metadata
LABEL org.opencontainers.image.title="TMDB Embed API" \
    org.opencontainers.image.description="Streaming metadata + source aggregation API with multi-key TMDB rotation" \
    org.opencontainers.image.version="${VERSION}" \
    org.opencontainers.image.source="https://github.com/Inside4ndroid/TMDB-Embed-API" \
    org.opencontainers.image.licenses="MIT"

# Healthcheck (simple)
HEALTHCHECK --interval=30s --timeout=5s --start-period=20s CMD wget -qO- http://localhost:${API_PORT:-8787}/api/health || exit 1

CMD ["node","apiServer.js"]
