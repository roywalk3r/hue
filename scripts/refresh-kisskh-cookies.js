/**
 * KissKH Cloudflare Cookie Refresher
 *
 * Uses Playwright to visit kisskh.ovh, wait out the Cloudflare challenge,
 * extract session cookies, and save them for the Rust backend to use.
 *
 * Usage:
 *   node scripts/refresh-kisskh-cookies.js
 *   HEADLESS=false node scripts/refresh-kisskh-cookies.js
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
const TIMEOUT_MS = parseInt(process.env.CF_TIMEOUT || '60000', 10);
const PROXY_SERVER = process.env.KISSKH_PROXY || ''; // e.g. socks5://127.0.0.1:1080

async function hasChallenge(page) {
  const title = await page.title().catch(() => '');
  if (title === 'Just a moment...') return true;

  const selectors = [
    '#challenge-running',
    '#challenge-form',
    '.cf-challenge-running',
    'div:has-text("Checking your browser")',
  ];
  for (const sel of selectors) {
    try {
      const count = await page.locator(sel).count();
      if (count > 0) return true;
    } catch { /* ignore */ }
  }
  return false;
}

async function refreshCookies() {
  console.log(`[KissKH-Cookies] Starting refresh via Playwright (headless=${HEADLESS}, timeout=${TIMEOUT_MS}ms)`);

  let browser;
  try {
    const launchOpts = { headless: HEADLESS };
    if (PROXY_SERVER) {
      launchOpts.proxy = { server: PROXY_SERVER };
      console.log(`[KissKH-Cookies] Using proxy: ${PROXY_SERVER}`);
    }
    browser = await chromium.launch(launchOpts);
    const context = await browser.newContext({
      userAgent: 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36',
      viewport: { width: 1920, height: 1080 },
    });

    const page = await context.newPage();

    // Extra evasion: mask webdriver
    await page.addInitScript(() => {
      Object.defineProperty(navigator, 'webdriver', { get: () => undefined });
      window.chrome = { runtime: {} };
    });

    // Navigate — use domcontentloaded so we don't wait forever on blocked assets
    await page.goto(TARGET_URL, { waitUntil: 'domcontentloaded', timeout: TIMEOUT_MS });

    // Wait for challenge to resolve — poll aggressively
    const start = Date.now();
    let challengePassed = false;
    let lastHtml = '';
    while (Date.now() - start < TIMEOUT_MS) {
      const html = await page.content().catch(() => '');
      lastHtml = html;
      if (!html.includes('challenge') && !html.includes('Checking your browser') && !html.includes('Just a moment')) {
        challengePassed = true;
        break;
      }
      const title = await page.title().catch(() => '');
      if (title !== 'Just a moment...') {
        challengePassed = true;
        break;
      }
      process.stdout.write('.');
      await page.waitForTimeout(2000);
    }
    process.stdout.write('\n');

    if (!challengePassed) {
      const title = await page.title().catch(() => '');
      console.error(`[KissKH-Cookies] Challenge failed. Page title: "${title}"`);
      console.error(`[KissKH-Cookies] HTML snippet (first 800 chars):\n${lastHtml.substring(0, 800)}`);
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
      cookie_header: cookies.map(c => `${c.name}=${c.value}`).join('; '),
    };

    // Ensure directory exists
    const dir = path.dirname(COOKIE_FILE);
    if (!fs.existsSync(dir)) fs.mkdirSync(dir, { recursive: true });

    fs.writeFileSync(COOKIE_FILE, JSON.stringify(payload, null, 2));
    console.log(`[KissKH-Cookies] Saved ${cookies.length} cookies to ${COOKIE_FILE}`);
    console.log(`[KissKH-Cookies] Cookie header: ${payload.cookie_header.substring(0, 120)}...`);

    await browser.close();
  } catch (err) {
    console.error('[KissKH-Cookies] Error:', err.message);
    if (browser) await browser.close().catch(() => {});
    process.exit(1);
  }
}

refreshCookies();
