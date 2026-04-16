const express = require('express');
const router = express.Router();
const { config, saveConfigPatch, OVERRIDE_PATH, CONFIG_SCHEMA_VERSION } = require('../utils/config');
const { clearCache } = require('../utils/cache');
const { getCookieStats, listProviders } = require('../providers/registry');
const { updatePassword } = require('../utils/auth');

module.exports = function(metrics) {
    router.get('/config', (req, res) => {
        res.json({
            success: true,
            config,
            overridePath: OVERRIDE_PATH,
            schemaVersion: CONFIG_SCHEMA_VERSION
        });
    });

    router.post('/config', async (req, res) => {
        try {
            const patch = req.body;
            if (await saveConfigPatch(patch)) {
                res.json({ success: true, message: 'Configuration updated and reloaded' });
            } else {
                res.status(500).json({ success: false, error: 'Failed to save configuration' });
            }
        } catch (e) {
            res.status(500).json({ success: false, error: e.message });
        }
    });

    router.get('/cache/clear', (req, res) => {
        clearCache();
        res.json({ success: true, message: 'Global stream cache cleared' });
    });

    router.post('/cache/clear', (req, res) => {
        clearCache();
        res.json({ success: true, message: 'Global stream cache cleared' });
    });

    router.get('/status', (req, res) => {
        res.json({
            success: true,
            version: '3.1.0',
            uptime: Math.floor((Date.now() - (metrics?.uptime || Date.now())) / 1000),
            metrics: metrics || {},
            providers: listProviders().map(p => ({ name: p.name, enabled: p.enabled }))
        });
    });

    router.get('/health', (req, res) => res.json({ status: 'OK', timestamp: new Date().toISOString() }));

    router.get('/debug/env', (req,res) => {
        const cookieStats = getCookieStats ? getCookieStats() : null;
        res.json({
            port: config.port,
            defaultProviders: config.defaultProviders,
            febboxCookieCount: config.febboxCookies.length,
            showboxCacheDir: process.env.SHOWBOX_CACHE_DIR || '(os tmp)',
            nodeVersion: process.version,
            cookieStats
        });
    });

    router.post('/auth/password', async (req, res) => {
        const { username, currentPassword, newPassword } = req.body;
        if (!username || !currentPassword || !newPassword) {
            return res.status(400).json({ success: false, error: 'All fields required' });
        }
        try {
            const success = await updatePassword(username, currentPassword, newPassword);
            if (success) res.json({ success: true, message: 'Password updated successfully' });
            else res.status(401).json({ success: false, error: 'Invalid username or password' });
        } catch (e) {
            res.status(500).json({ success: false, error: e.message });
        }
    });

    return router;
};
