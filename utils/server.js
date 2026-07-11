/**
 * Hue Stream API
 * Professional Streaming Aggregator
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */

const { config } = require('./config');

function resolveServerUrl(req) {
  // 1. Priority: Explicitly configured SERVER_URL (useful for double-proxies/rewrites)
  if (config.serverUrl) {
    console.log('[resolveServerUrl] Using config.serverUrl:', config.serverUrl);
    return config.serverUrl.replace(/\/$/, '');
  }

  // 2. Automatic Detection
  const forwardedHost = req.headers['x-forwarded-host'];
  const host = forwardedHost || req.get('host') || 'localhost';
  const forwardedProto = (req.headers['x-forwarded-proto'] || '').split(',')[0].trim();
  let protocol = forwardedProto || req.protocol || 'http';

  console.log('[resolveServerUrl] forwardedHost:', forwardedHost, 'host:', host, 'protocol:', protocol, 'req.get(host):', req.get('host'));

  // Force http for local access to avoid self-signed cert issues on localhost
  if (/^(localhost|127\.0\.0\.1|\[?::1\]?)(:\d+)?$/i.test(host)) {
    protocol = 'http';
  }

  return `${protocol}://${host}`;
}

module.exports = { resolveServerUrl };
