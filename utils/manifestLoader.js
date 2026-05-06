const fs = require('fs');
const path = require('path');
const TOML = require('@iarna/toml');

const MANIFESTS_DIR = path.join(__dirname, '..', 'providers', 'manifests');

let _cache = null;

function loadManifests() {
  if (_cache) return _cache;
  const result = {};
  if (!fs.existsSync(MANIFESTS_DIR)) return result;
  for (const file of fs.readdirSync(MANIFESTS_DIR)) {
    if (!file.endsWith('.toml')) continue;
    const raw = fs.readFileSync(path.join(MANIFESTS_DIR, file), 'utf8');
    const manifest = TOML.parse(raw);
    result[manifest.provider.name] = manifest;
  }
  _cache = result;
  return result;
}

function getManifestByLabel(label) {
  const manifests = loadManifests();
  return Object.values(manifests).find(
    m => m.provider.label.toLowerCase() === label.toLowerCase()
  ) || null;
}

function getBehaviorConfig(providerRealName) {
  const manifests = loadManifests();
  const manifest = Object.values(manifests).find(
    m => m.provider.name === providerRealName || m.provider.label.toLowerCase() === providerRealName.toLowerCase()
  );
  return manifest ? manifest.behavior : { timeout_ms: 15000, max_retries: 2, circuit_breaker: true, priority: 50 };
}

function getNodeProviders() {
  const manifests = loadManifests();
  return Object.values(manifests).filter(
    m => m.provider.runtime === 'node' || m.provider.runtime === 'both'
  );
}

module.exports = { loadManifests, getManifestByLabel, getBehaviorConfig, getNodeProviders };
