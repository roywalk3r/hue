/**
 * Hue Stream API
 * Professional Streaming Aggregator
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */

module.exports = {
  apps: [
    {
      name: 'hue-api',
      script: 'apiServer.js',
      autorestart: true,
      max_restarts: 10,
      min_uptime: '10s',
      max_memory_restart: '512M',
      env: { NODE_ENV: 'production', API_PORT: 24608 }
    },
    {
      name: 'hue-scraper',
      script: 'scraperServer.js',
      autorestart: true,
      max_restarts: 10,
      min_uptime: '5s',
      env: { SCRAPER_PORT: 24610 }
    },
    {
      name: 'hue-rust',
      script: './huerust/target/release/huerust',
      cwd: __dirname,
      autorestart: true,
      max_restarts: 5,
      min_uptime: '10s',
    }
  ]
};
