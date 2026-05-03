// In-memory cookie store
const cookieStore = new Map();

// Redis support (fallback to memory if Redis is unavailable)
let redis = null;
try {
  const Redis = require('ioredis');
  redis = new Redis(process.env.REDIS_URL || 'redis://127.0.0.1:6379');
  redis.on('error', (e) => {
    console.warn('[cookieWorker] Redis connection error, using memory fallback:', e.message);
    redis = null;
  });
} catch (e) {
  console.warn('[cookieWorker] ioredis not available, using memory store');
}

const COOKIE_TTL_SECS = 24 * 60 * 60; // 24h

// Cookie refresh strategies — just visit the site, scripts set cookies automatically
const COOKIE_FLOWS = {
  sflix: {
    url: 'https://sflix.film',
    cookieNames: ['sflix_token'],
    labelCookie: 'sflix_i18n_lang=en',
  },
  moviebox: {
    url: `https://${process.env.MOVIEBOX_HOST || 'moviebox.ph'}`,
    cookieNames: ['mb_token', 'uuid'],
    backupHosts: ['moviebox.ph', 'moviebox.id', 'moviebox.pk', 'movieboxapp.in'],
  },
  febbox: {
    url: 'https://febbox.com',
    cookieNames: [],
  },
};

async function refreshCookiesForProvider(provider) {
  const flow = COOKIE_FLOWS[provider];
  if (!flow) return null;

  let browser;
  try {
    const { getNewContext } = require('./browserManager');
    browser = await getNewContext();
  } catch (e) {
    console.warn(`[cookieWorker] Browser unavailable for ${provider}, using stored cookies`);
    return cookieStore.get(provider) || null;
  }

  const hostsToTry = flow.backupHosts
    ? [flow.url.replace('https://', ''), ...flow.backupHosts.filter(h => h !== flow.url.replace('https://', ''))]
    : [flow.url.replace('https://', '')];

  for (const host of hostsToTry) {
    const url = host.startsWith('http') ? host : `https://${host}`;
    let page;
    try {
      console.log(`[cookieWorker] ${provider}: trying ${url}`);
      page = await browser.newPage();

      // Block non-essential resources to speed up cookie acquisition
      await page.route('**/*', (route) => {
        const type = route.request().resourceType();
        if (['document', 'script', 'xhr', 'fetch'].includes(type)) return route.continue();
        return route.abort();
      });

      await page.goto(url, { waitUntil: 'networkidle', timeout: 60000 });
      await page.waitForTimeout(10000); // Let scripts set cookies

      const cookies = await page.context().cookies();
      const foundCookies = cookies.filter(c => flow.cookieNames.includes(c.name));

      if (foundCookies.length > 0 || flow.cookieNames.length === 0) {
        let cookieArr = foundCookies.map(c => `${c.name}=${c.value}`);
        if (flow.labelCookie) cookieArr.push(flow.labelCookie);
        const cookieString = cookieArr.join('; ');

        const expires = Date.now() + (COOKIE_TTL_SECS * 1000);
        const entry = { cookies: [cookieString], expires, host };
        cookieStore.set(provider, entry);

        if (redis) {
          try {
            await redis.setex(`cookies:${provider}`, COOKIE_TTL_SECS, JSON.stringify(entry));
          } catch (e) {
            console.warn(`[cookieWorker] Redis write failed for ${provider}`);
          }
        }

        console.log(`[cookieWorker] ${provider}: refreshed (${foundCookies.length} tokens) via ${host}`);
        return entry;
      }
    } catch (e) {
      console.warn(`[cookieWorker] ${provider}: ${host} failed — ${e.message}`);
    } finally {
      if (page) await page.close();
    }
  }

  console.warn(`[cookieWorker] ${provider}: all hosts failed, using fallback`);
  return cookieStore.get(provider) || null;
}

async function startCookieWorker() {
  console.log('[cookieWorker] Starting cookie rotation worker');

  const providers = Object.keys(COOKIE_FLOWS);

  // Initial refresh — do one at a time to avoid resource contention
  for (const provider of providers) {
    await refreshCookiesForProvider(provider);
  }

  // Rotate every 30 minutes
  setInterval(async () => {
    for (const provider of providers) {
      await refreshCookiesForProvider(provider);
    }
  }, 30 * 60 * 1000);
}

async function getCookies(provider) {
  // Try Redis first
  if (redis) {
    try {
      const data = await redis.get(`cookies:${provider}`);
      if (data) {
        const entry = JSON.parse(data);
        if (Date.now() < entry.expires) return entry;
      }
    } catch (e) {
      console.warn(`[cookieWorker] Redis read failed for ${provider}`);
    }
  }
  // Fallback to memory
  const entry = cookieStore.get(provider);
  if (!entry) return null;
  if (Date.now() > entry.expires) {
    cookieStore.delete(provider);
    return null;
  }
  return entry;
}

module.exports = { startCookieWorker, getCookies, refreshCookies: refreshCookiesForProvider };
