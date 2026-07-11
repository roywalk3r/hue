#!/usr/bin/env node

/**
 * Hue Stream API
 * Professional Streaming Aggregator
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */


const fs = require('fs');
const path = require('path');

console.log('[setup] Checking for browser installation...');

// Check if Chrome/Chromium exists in common locations
const browserPaths = [
  '/usr/bin/google-chrome',
  '/usr/bin/chromium-browser',
  '/usr/bin/chromium',
  '/opt/google/chrome/chrome',
  '/usr/local/bin/chrome',
  '/usr/local/bin/chromium',
  '/home/chrome/chrome',
  '/var/www/chrome/chrome'
];

let browserExists = false;
for (const browserPath of browserPaths) {
  if (fs.existsSync(browserPath)) {
    console.log(`[setup] Browser found at: ${browserPath}`);
    browserExists = true;
    // Set environment variable if not already set
    if (!process.env.BROWSER_EXECUTABLE_PATH && !process.env.PUPPETEER_EXECUTABLE_PATH) {
        process.env.BROWSER_EXECUTABLE_PATH = browserPath;
    }
    break;
  }
}

// Official Playwright images ship browsers under /ms-playwright instead of a system Chromium path.
if (!browserExists && fs.existsSync('/ms-playwright')) {
  console.log('[setup] Playwright browser cache found at: /ms-playwright');
  browserExists = true;
}

if (!browserExists) {
  console.log('[setup] Chrome/Chromium not found in standard paths');
  console.log('[setup] You may need to:');
  console.log('[setup] 1. Install Chromium via your package manager (e.g., apt-get install chromium-browser)');
  console.log('[setup] 2. Run "npx playwright install chromium" if you have write access');
  console.log('[setup] 3. Use providers that don\'t require browser automation');
  
  // Don't exit with error, just warn and continue
  console.log('[setup] Continuing startup - automation providers may fail without a browser');
} else {
  console.log('[setup] Browser is available');
}

console.log('[setup] Starting application...');
