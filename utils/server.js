function resolveServerUrl(req) {
  const host = req.get('host') || 'localhost';
  const forwardedProto = (req.headers['x-forwarded-proto'] || '').split(',')[0].trim();
  let protocol = forwardedProto || req.protocol || 'http';
  // Force http for local access to avoid self-signed cert issues on localhost
  if (/^(localhost|127\.0\.0\.1|\[?::1\]?)(:\d+)?$/i.test(host)) {
    protocol = 'http';
  }
  return `${protocol}://${host}`;
}

module.exports = { resolveServerUrl };
