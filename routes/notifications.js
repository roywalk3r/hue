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

const TOKENS_FILE = path.join(__dirname, '..', 'utils', 'fcm-tokens.json');
const CRON_SECRET = process.env.NOTIFICATION_CRON_SECRET || 'hue-cron-2026';

// ── Token store (in-memory + file-backed) ──────────────────────────────

let tokens = new Set();

function loadTokens() {
  try {
    if (fs.existsSync(TOKENS_FILE)) {
      const raw = fs.readFileSync(TOKENS_FILE, 'utf8');
      tokens = new Set(JSON.parse(raw));
    }
  } catch (e) {
    console.error('[notifications] failed to load tokens', e.message);
  }
}

function saveTokens() {
  try {
    fs.mkdirSync(path.dirname(TOKENS_FILE), { recursive: true });
    fs.writeFileSync(TOKENS_FILE, JSON.stringify([...tokens]), 'utf8');
  } catch (e) {
    console.error('[notifications] failed to save tokens', e.message);
  }
}

loadTokens();

// ── FCM sender (lazy init) ─────────────────────────────────────────────

let fcmApp = null;
function getFcm() {
  if (fcmApp) return fcmApp;
  const admin = require('firebase-admin');
  const sa = process.env.FCM_SERVICE_ACCOUNT;
  if (sa) {
    fcmApp = admin.initializeApp({
      credential: admin.credential.cert(JSON.parse(sa)),
    });
  } else {
    // Fallback: Application Default Credentials
    fcmApp = admin.initializeApp();
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
    tokens.add(fcm_token);
    saveTokens();
    res.json({ success: true });
  });

  /** Cron endpoint — fetch latest from all sources and push  */
  router.get('/cron', async (req, res) => {
    if (req.query.secret !== CRON_SECRET) {
      return res.status(403).json({ error: 'invalid secret' });
    }

    if (!tokens.size) {
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
