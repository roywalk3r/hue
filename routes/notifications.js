/**
 * Hue Stream API — Push Notifications
 *
 * Stores FCM device tokens and dispatches push notifications for
 * new content across all types (anime, movies, TV, drama).
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @license MIT
 */

const express = require('express');
const fs = require('fs');
const path = require('path');
const router = express.Router();

const TOKENS_FILE = process.env.FCM_TOKENS_FILE || path.join(__dirname, '..', 'data', 'fcm-tokens.json');
const CRON_SECRET = process.env.NOTIFICATION_CRON_SECRET || 'hue-cron-2026';

// ── Token store (file-backed — PM2 cluster safe) ─────────────────────

function readTokens() {
  try {
    if (fs.existsSync(TOKENS_FILE)) {
      return new Set(JSON.parse(fs.readFileSync(TOKENS_FILE, 'utf8')));
    }
  } catch (e) {
    console.error('[notifications] failed to read tokens', e.message);
  }
  return new Set();
}

function addToken(token) {
  const tokens = readTokens();
  tokens.add(token);
  try {
    fs.mkdirSync(path.dirname(TOKENS_FILE), { recursive: true });
    fs.writeFileSync(TOKENS_FILE, JSON.stringify([...tokens]), 'utf8');
  } catch (e) {
    console.error('[notifications] failed to save tokens', e.message);
  }
}

// ── FCM sender (lazy init) ─────────────────────────────────────────────

let fcmApp = null;
const FCM_KEY_PATH = process.env.FCM_KEY_PATH || path.join(__dirname, '..', 'vexli-3d15a-firebase-adminsdk-fbsvc-6c3a89a891.json');

function getFcm() {
  if (fcmApp) return fcmApp;
  const admin = require('firebase-admin');
  const keyPath = FCM_KEY_PATH;
  if (fs.existsSync(keyPath)) {
    fcmApp = admin.initializeApp({
      credential: admin.credential.cert(require(keyPath)),
    });
    console.log('[notifications] FCM initialized with key', keyPath);
  } else {
    console.error('[notifications] FCM key not found at', keyPath);
    fcmApp = admin.initializeApp(); // fallback
  }
  return fcmApp;
}

async function sendBatch(tokens, payload) {
  const messaging = getFcm().messaging();
  const batches = [];
  while (tokens.length) batches.push(tokens.splice(0, 500));
  for (const batch of batches) {
    try {
      await messaging.sendEachForMulticast({
        tokens: batch,
        notification: { title: payload.title, body: payload.body },
        data: payload.data,
        android: { priority: 'high', notification: { channelId: 'new_releases' } },
      });
    } catch (e) {
      console.error('[notifications] batch error', e.message);
    }
  }
}

// ── Routes ──────────────────────────────────────────────────────────────

module.exports = function (metrics) {
  /** Register a device token */
  router.post('/register', async (req, res) => {
    const { fcm_token } = req.body;
    if (!fcm_token) return res.status(400).json({ success: false, error: 'fcm_token required' });
    addToken(fcm_token);
    res.json({ success: true });
  });

  /** Cron endpoint — fetch latest from all sources and push  */
  router.get('/cron', async (req, res) => {
    if (req.query.secret !== CRON_SECRET) {
      return res.status(403).json({ error: 'invalid secret' });
    }

    const allTokens = [...readTokens()];
    if (!allTokens.length) {
      return res.json({ success: true, sent: 0, message: 'no tokens' });
    }

    const sent = [];
    const allTokens = [...tokens];
    const now = Date.now();
    const today = new Date().toISOString().slice(0, 10);

    // ── Anime schedule ────────────────────────────────────────────────
    try {
      const resp = await fetch(
        `${process.env.RUST_ENGINE_URL || 'http://127.0.0.1:24609'}/api/anime/schedule`,
        { signal: AbortSignal.timeout(15000) },
      );
      const body = await resp.json();
      const schedule = body.data || [];
      for (const entry of schedule) {
        const airsAt = (entry.airsAt || 0) * 1000;
        if (airsAt > 0 && now - airsAt > 0 && now - airsAt < 60 * 60 * 1000) {
          await sendBatch([...allTokens], {
            title: `New episode: ${entry.animeTitle}`,
            body: `Episode ${entry.episode} is now available!`,
            data: { type: 'anime', id: entry.slug || entry.animeId || '', slug: entry.slug || '' },
          });
          sent.push({ type: 'anime', title: entry.animeTitle, ep: entry.episode });
        }
      }
    } catch (e) {
      console.error('[notifications] anime schedule error', e.message);
    }

    // ── TMDB now-playing movies ───────────────────────────────────────
    try {
      const apiKey = process.env.TMDB_API_KEY || '';
      if (apiKey) {
        const resp = await fetch(
          `https://api.themoviedb.org/3/movie/now_playing?api_key=${apiKey}&region=US`,
          { signal: AbortSignal.timeout(10000) },
        );
        const body = await resp.json();
        for (const m of (body.results || []).slice(0, 6)) {
          if (m.release_date === today) {
            await sendBatch([...allTokens], {
              title: 'New in theaters',
              body: `${m.title} is now available!`,
              data: { type: 'movie', id: String(m.id) },
            });
            sent.push({ type: 'movie', title: m.title });
          }
        }
      }
    } catch (e) {
      console.error('[notifications] movies error', e.message);
    }

    // ── TMDB on-the-air TV ───────────────────────────────────────────
    try {
      const apiKey = process.env.TMDB_API_KEY || '';
      if (apiKey) {
        const resp = await fetch(
          `https://api.themoviedb.org/3/tv/on_the_air?api_key=${apiKey}`,
          { signal: AbortSignal.timeout(10000) },
        );
        const body = await resp.json();
        for (const s of (body.results || []).slice(0, 6)) {
          if (s.first_air_date === today) {
            await sendBatch([...allTokens], {
              title: 'New episodes tonight',
              body: `${s.name} airs tonight!`,
              data: { type: 'tv', id: String(s.id) },
            });
            sent.push({ type: 'tv', title: s.name });
          }
        }
      }
    } catch (e) {
      console.error('[notifications] TV error', e.message);
    }

    res.json({ success: true, sent: sent.length, items: sent });
  });

  return router;
};
