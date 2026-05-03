const crypto = require('crypto');

const COOKIE_PROVIDERS = ['febbox', 'sflix', 'moviebox'];

// In-memory cookie store (will be Redis-backed in Task 9)
const cookieStore = new Map();

const LOGIN_FLOWS = {
  febbox: {
    url: 'https://febbox.com/login',
    selectors: {
      username: 'input[name="email"]',
      password: 'input[name="password"]',
      submit: 'button[type="submit"]',
    },
    credentials: () => ({
      username: process.env.FEBBOX_USERNAME,
      password: process.env.FEBBOX_PASSWORD,
    }),
  },
  sflix: {
    url: 'https://sflix.to/login',
    selectors: {
      username: 'input[name="email"]',
      password: 'input[name="password"]',
      submit: 'button[type="submit"]',
    },
    credentials: () => ({
      username: process.env.SFLIX_USERNAME,
      password: process.env.SFLIX_PASSWORD,
    }),
  },
  moviebox: {
    url: 'https://moviebox.ph/login',
    selectors: {
      username: 'input[name="email"]',
      password: 'input[name="password"]',
      submit: 'button[type="submit"]',
    },
    credentials: () => ({
      username: process.env.MOVIEBOX_USERNAME,
      password: process.env.MOVIEBOX_PASSWORD,
    }),
  },
};

async function refreshCookiesForProvider(provider) {
  const flow = LOGIN_FLOWS[provider];
  if (!flow) return null;

  const creds = flow.credentials();
  if (!creds.username || !creds.password) {
    console.warn(`[cookieWorker] No credentials for ${provider}, skipping`);
    return null;
  }

  // Try Playwright; fall back to stored cookies
  let browser;
  try {
    const { getBrowser } = require('./browserManager');
    browser = await getBrowser();
  } catch (e) {
    console.warn(`[cookieWorker] Browser unavailable for ${provider}, using stored cookies only`);
    return cookieStore.get(provider) || null;
  }

  const page = await browser.newPage();

  try {
    await page.goto(flow.url, { waitUntil: 'networkidle', timeout: 30000 });
    await page.fill(flow.selectors.username, creds.username);
    await page.fill(flow.selectors.password, creds.password);
    await page.click(flow.selectors.submit);
    await page.waitForNavigation({ waitUntil: 'networkidle', timeout: 15000 });

    const cookies = await page.context().cookies();
    const cookieStrings = cookies.map(c => `${c.name}=${c.value}`);

    const expires = Date.now() + (24 * 60 * 60 * 1000); // 24h
    const entry = { cookies: cookieStrings, expires };
    cookieStore.set(provider, entry);

    console.log(`[cookieWorker] Refreshed ${cookieStrings.length} cookies for ${provider}`);
    return entry;
  } catch (e) {
    console.error(`[cookieWorker] Failed to refresh ${provider}:`, e.message);
    const fallback = cookieStore.get(provider);
    return fallback || null;
  } finally {
    await page.close();
  }
}

async function startCookieWorker() {
  console.log('[cookieWorker] Starting cookie rotation worker');

  // Initial refresh — do one at a time to avoid resource contention
  for (const provider of COOKIE_PROVIDERS) {
    await refreshCookiesForProvider(provider);
  }

  // Rotate every 30 minutes
  setInterval(async () => {
    for (const provider of COOKIE_PROVIDERS) {
      await refreshCookiesForProvider(provider);
    }
  }, 30 * 60 * 1000);
}

function getCookies(provider) {
  const entry = cookieStore.get(provider);
  if (!entry) return null;
  if (Date.now() > entry.expires) {
    cookieStore.delete(provider);
    return null;
  }
  return entry;
}

module.exports = { startCookieWorker, getCookies, refreshCookies: refreshCookiesForProvider };
