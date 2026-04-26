module.exports = {
  apps: [
    {
      name: 'hue-rust-api',
      script: 'cargo',
      args: 'run --release',
      cwd: './huerust',
      env: {
        RUST_PORT: 24608,
      }
    },
    {
      name: 'hue-scraper-microservice',
      script: 'scraperServer.js',
      env: {
        SCRAPER_PORT: 24610
      }
    }
  ]
};
