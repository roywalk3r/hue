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
      name: 'hue',
      script: 'pnpm',
      args: 'run start',
      env: {
        PORT: 24608,
      }
    },
    {
      name: 'hue-rust-api',
      cwd: './huerust',
      script: './target/release/huerust',
      env: {
        RUST_PORT: 24609,
      }
    },
    {
      name: 'hue-scraper-microservice',
      script: 'scraperServer.js',
      env: {
        SCRAPER_PORT: 24610
      }
    },
  ]
};
