const { config } = require('./config');

function resolveServerUrl(req) {
  // 1. Priority: Explicitly configured SERVER_URL (useful for double-proxies/rewrites)
  if (config.serverUrl) return config.serverUrl.replace(/\/$/, '');

  // 2. Automatic Detection
  const forwardedHost = req.headers['x-forwarded-host'];
  const host = forwardedHost || req.get('host') || 'localhost';
  const forwardedProto = (req.headers['x-forwarded-proto'] || '').split(',')[0].trim();
  let protocol = forwardedProto || req.protocol || 'http';
  
  // Force http for local access to avoid self-signed cert issues on localhost
  if (/^(localhost|127\.0\.0\.1|\[?::1\]?)(:\d+)?$/i.test(host)) {
    protocol = 'http';
  }
  
  return `${protocol}://${host}`;
}

module.exports = { resolveServerUrl };
