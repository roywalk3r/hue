#!/usr/bin/env node

const fs = require('fs');
const path = require('path');
const { exec } = require('child_process');

console.log('[setup] Checking for Chrome installation on Plesk...');

// Check if Chrome exists in common Plesk/shared hosting locations
const chromePaths = [
  '/usr/bin/google-chrome',
  '/usr/bin/chromium-browser', 
  '/usr/bin/chromium',
  '/opt/google/chrome/chrome',
  '/usr/local/bin/chrome',
  '/usr/local/bin/chromium',
  '/home/chrome/chrome',
  '/var/www/chrome/chrome'
];

let chromeExists = false;
for (const chromePath of chromePaths) {
  if (fs.existsSync(chromePath)) {
    console.log(`[setup] Chrome found at: ${chromePath}`);
    chromeExists = true;
    break;
  }
}

if (!chromeExists) {
  console.log('[setup] Chrome not found on Plesk server');
  console.log('[setup] For Plesk hosting, you may need to:');
  console.log('[setup] 1. Contact your hosting provider to install Chrome/Chromium');
  console.log('[setup] 2. Or use a provider that doesn\'t require Puppeteer (like vixsrc only)');
  console.log('[setup] 3. Or switch to a VPS/dedicated server where you can install Chrome');
  
  // Don't exit with error, just warn and continue
  console.log('[setup] Continuing startup - vidsrccc-puppeteer may fail without Chrome');
} else {
  console.log('[setup] Chrome is available on Plesk server');
}

console.log('[setup] Starting application...');
