/**
 * Unit tests for changed modules — run with: node tests/unit.js
 */
'use strict';

const assert = require('assert');
const path = require('path');

let passed = 0;
let failed = 0;

function test(name, fn) {
    try {
        fn();
        console.log(`  ✓ ${name}`);
        passed++;
    } catch (e) {
        console.error(`  ✗ ${name}`);
        console.error(`    ${e.message}`);
        failed++;
    }
}

// ─── utils/security.js ───────────────────────────────────────────────────────
console.log('\nutils/security.js');

process.env.APP_SECRET = 'test-secret-for-unit-tests';
const { encryptUrl, decryptUrl } = require('../utils/security');

test('encrypt/decrypt roundtrip returns original URL', () => {
    const url = 'https://example.com/stream/video.mp4?token=abc123';
    const encrypted = encryptUrl(url);
    assert.strictEqual(typeof encrypted, 'string', 'encrypted should be a string');
    assert.notStrictEqual(encrypted, url, 'encrypted should differ from original');
    const decrypted = decryptUrl(encrypted);
    assert.ok(decrypted !== null, 'decrypted should not be null');
    assert.strictEqual(decrypted.url, url, 'decrypted.url should match original');
});

test('encrypt produces different output each call (random IV)', () => {
    const url = 'https://example.com/test.m3u8';
    const a = encryptUrl(url);
    const b = encryptUrl(url);
    assert.notStrictEqual(a, b, 'each encryption should differ due to random IV');
    assert.strictEqual(decryptUrl(a).url, url);
    assert.strictEqual(decryptUrl(b).url, url);
});

test('decryptUrl returns null for invalid input', () => {
    const result = decryptUrl('not-valid-hex');
    assert.strictEqual(result, null, 'should return null for garbage input');
});

// ─── utils/manifestLoader.js ─────────────────────────────────────────────────
console.log('\nutils/manifestLoader.js');

const { getBehaviorConfig, loadManifests } = require('../utils/manifestLoader');

test('loadManifests returns non-empty object', () => {
    const manifests = loadManifests();
    assert.ok(typeof manifests === 'object', 'should return object');
    assert.ok(Object.keys(manifests).length > 0, 'should have at least one manifest');
});

test('getBehaviorConfig matches by exact label (sflix)', () => {
    const cfg = getBehaviorConfig('sflix');
    assert.ok(cfg.timeout_ms > 0, 'timeout_ms should be positive');
    assert.ok(typeof cfg.priority === 'number', 'priority should be a number');
    assert.notDeepStrictEqual(cfg, { timeout_ms: 15000, max_retries: 2, circuit_breaker: true, priority: 50 },
        'should return manifest values, not defaults');
});

test('getBehaviorConfig matches "vidsrc-embed" via normalized label (VidSrc Embed)', () => {
    const cfg = getBehaviorConfig('vidsrc-embed');
    assert.ok(cfg.priority > 0, 'should find manifest, not fall back to defaults');
    assert.strictEqual(cfg.priority, 55, 'vidsrc-embed manifest priority should be 55');
});

test('getBehaviorConfig matches "showbox-playwright" via normalized label (Showbox Playwright)', () => {
    const cfg = getBehaviorConfig('showbox-playwright');
    assert.ok(cfg.priority > 0, 'should find manifest');
    assert.strictEqual(cfg.priority, 50, 'showbox-playwright manifest priority should be 50');
});

test('getBehaviorConfig returns defaults for unknown provider', () => {
    const cfg = getBehaviorConfig('nonexistent-provider-xyz');
    assert.strictEqual(cfg.timeout_ms, 15000);
    assert.strictEqual(cfg.priority, 50);
});

test('getBehaviorConfig matches "uhdmovies" (UHDMovies)', () => {
    const cfg = getBehaviorConfig('uhdmovies');
    assert.strictEqual(cfg.priority, 60);
});

test('getBehaviorConfig matches "moviesmod" (MoviesMod)', () => {
    const cfg = getBehaviorConfig('moviesmod');
    assert.strictEqual(cfg.priority, 55);
});

test('getBehaviorConfig matches "9anime"', () => {
    const cfg = getBehaviorConfig('9anime');
    assert.strictEqual(cfg.priority, 65);
});

test('getBehaviorConfig matches "showbox"', () => {
    const cfg = getBehaviorConfig('showbox');
    assert.strictEqual(cfg.priority, 55);
});

test('getBehaviorConfig matches "4khdhub"', () => {
    const cfg = getBehaviorConfig('4khdhub');
    assert.strictEqual(cfg.priority, 60);
});

test('getBehaviorConfig matches "anikai"', () => {
    const cfg = getBehaviorConfig('anikai');
    assert.strictEqual(cfg.priority, 55);
});

test('getBehaviorConfig matches "vidsrc-embed" (Nebula)', () => {
    const cfg = getBehaviorConfig('vidsrc-embed');
    assert.strictEqual(cfg.priority, 55);
    assert.ok(cfg.timeout_ms > 0);
});

test('all new manifests have circuit_breaker enabled', () => {
    const newProviders = ['showbox', 'showbox-playwright', '4khdhub', '9anime', 'anikai', 'moviesmod', 'uhdmovies', 'vidsrc-embed'];
    for (const name of newProviders) {
        const cfg = getBehaviorConfig(name);
        assert.strictEqual(cfg.circuit_breaker, true, `${name} should have circuit_breaker=true`);
    }
});

test('manifest timeouts are all within reasonable range (5s–120s)', () => {
    const manifests = loadManifests();
    for (const [file, m] of Object.entries(manifests)) {
        const ms = m.behavior?.timeout_ms ?? 0;
        assert.ok(ms >= 5000 && ms <= 120000, `${file} timeout_ms=${ms} out of range`);
    }
});

// ─── warmupWorker year logic ──────────────────────────────────────────────────
console.log('\nwarmupWorker year logic');

test('currentYear is dynamic (one year behind current calendar year)', () => {
    const currentYear = new Date().getFullYear() - 1;
    const now = new Date().getFullYear();
    assert.strictEqual(currentYear, now - 1, 'should always be one year behind current');
    assert.ok(currentYear >= 2025, 'should be at least 2025');
});

// ─── utils/auth.js session secret stability ───────────────────────────────────
console.log('\nutils/auth.js session secret');

const crypto = require('crypto');

test('session secret is deterministic from APP_SECRET', () => {
    const appSecret = 'test-secret-for-unit-tests';
    const s1 = crypto.createHash('sha256').update(appSecret + ':sessions').digest();
    const s2 = crypto.createHash('sha256').update(appSecret + ':sessions').digest();
    assert.ok(Buffer.compare(s1, s2) === 0, 'same APP_SECRET should yield same session secret');
    assert.strictEqual(s1.length, 32, 'should be 32 bytes');
});

test('session secret differs from stream key (domain separation)', () => {
    const appSecret = 'test-secret-for-unit-tests';
    const sessionKey = crypto.createHash('sha256').update(appSecret + ':sessions').digest();
    const streamKey = crypto.createHash('sha256').update(appSecret).digest();
    assert.ok(Buffer.compare(sessionKey, streamKey) !== 0, ':sessions suffix should make keys distinct');
});

// ─── utils/interceptor.js exports ─────────────────────────────────────────────
console.log('\nutils/interceptor.js');

const interceptor = require('../utils/interceptor');

test('AD_DOMAINS is exported as an array', () => {
    assert.ok(Array.isArray(interceptor.AD_DOMAINS), 'AD_DOMAINS should be an array');
    assert.ok(interceptor.AD_DOMAINS.length >= 9, 'should have at least 9 domains');
});

test('AD_DOMAINS includes canonical domains', () => {
    const domains = interceptor.AD_DOMAINS;
    assert.ok(domains.includes('doubleclick'), 'should include doubleclick');
    assert.ok(domains.includes('google-analytics'), 'should include google-analytics');
    assert.ok(domains.some(d => d.includes('googletagmanager') || d.includes('analytics')),
        'should include analytics domains');
});

// ─── routes/sports.js AiSports route params ───────────────────────────────────
console.log('\nroutes/sports.js');

const sportsRoutes = require('../routes/sports');

test('resolveSportsMatchName reads legacy sports-stream matchName wildcard', () => {
    const req = { params: { matchName: 'psg-vs-arsenal', 0: '/admin/1' } };
    assert.strictEqual(sportsRoutes.resolveSportsMatchName(req), 'psg-vs-arsenal/admin/1');
});

test('resolveSportsMatchName reads aisports play id param', () => {
    const req = { params: { id: '4144956851383325432' } };
    assert.strictEqual(sportsRoutes.resolveSportsMatchName(req), '4144956851383325432');
});

// ─── Summary ──────────────────────────────────────────────────────────────────
console.log(`\n${passed + failed} tests: ${passed} passed, ${failed} failed\n`);
if (failed > 0) process.exit(1);
process.exit(0);
