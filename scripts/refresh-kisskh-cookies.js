/**
 * KissKH Cloudflare Cookie Refresher
 *
 * Uses Playwright to visit kisskh.ovh, solve the Cloudflare challenge,
 * extract session cookies, and save them for the Rust backend to use.
 *
 * Usage:
 *   node scripts/refresh-kisskh-cookies.js
 *   # or via cron every 30 minutes
 *

 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */

const { chromium } = require('playwright');
const fs = require('fs');
const path = require('path');

const TARGET_URL = 'https://kisskh.ovh/';
const COOKIE_FILE = process.env.KISSKH_COOKIE_FILE || 'data/kisskh-cookies.json';
const HEADLESS = process.env.HEADLESS !== 'false';
const TIMEOUT_MS = parseInt(process.env.CF_TIMEOUT || '30000', 10);

async function refreshCookies() {
  console.log(`[KissKH-Cookies] Starting refresh via Playwright (headless=${HEADLESS})`);

  let browser;
  try {
    browser = await chromium.launch({ headless: HEADLESS });
    const context = await browser.newContext({
      userAgent: 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36',
      viewport: { width: 1920, height: 1080 },
    });

    const page = await context.newPage();

    // Navigate and wait for challenge to resolve
    await page.goto(TARGET_URL, { waitUntil: 'domcontentloaded', timeout: TIMEOUT_MS });

    // Cloudflare challenge pages have specific text indicators
    const challengeSelectors = [
      '#challenge-running',
      '#challenge-form',
      '.cf-challenge-running',
      'title:has-text("Just a moment")',
      'text=Just a moment',
      'text=Checking your browser',
    ];

    // Wait for challenge to disappear or content to appear
    const start = Date.now();
    let challengePassed = false;
    while (Date.now() - start < TIMEOUT_MS) {
      const title = await page.title().catch(() => '');
      const hasChallenge = challengeSelectors.some(async sel => {
        try { return await page.locator(sel).count() > 0; } catch { return false; }
      });

      if (title !== 'Just a moment...' && !hasChallenge) {
        challengePassed = true;
        break;
      }
      await page.waitForTimeout(1000);
    }

    if (!challengePassed) {
      console.error('[KissKH-Cookies] Cloudflare challenge did not resolve in time');
      await browser.close();
      process.exit(1);
    }

    // Extract cookies
    const cookies = await context.cookies(TARGET_URL);
    const cookieMap = {};
    for (const c of cookies) {
      cookieMap[c.name] = c.value;
    }

    const payload = {
      url: TARGET_URL,
      fetchedAt: new Date().toISOString(),
      cookies: cookieMap,
      cookieHeader: cookies.map(c => `${c.name}=${c.value}`).join('; '),
    };

    // Ensure directory exists
    const dir = path.dirname(COOKIE_FILE);
    if (!fs.existsSync(dir)) fs.mkdirSync(dir, { recursive: true });

    fs.writeFileSync(COOKIE_FILE, JSON.stringify(payload, null, 2));
    console.log(`[KissKH-Cookies] Saved ${cookies.length} cookies to ${COOKIE_FILE}`);
    console.log(`[KissKH-Cookies] Cookie header: ${payload.cookieHeader.substring(0, 120)}...`);

    await browser.close();
  } catch (err) {
    console.error('[KissKH-Cookies] Error:', err.message);
    if (browser) await browser.close().catch(() => {});
    process.exit(1);
  }
}

refreshCookies();
