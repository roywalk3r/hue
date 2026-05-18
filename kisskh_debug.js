const { chromium } = require('playwright');

(async () => {
  const browser = await chromium.launch({ headless: true });
  const page = await browser.newPage();

  page.on('request', req => {
    console.log('REQ:', req.method(), req.url());
  });
  page.on('response', resp => {
    const url = resp.url();
    if (url.includes('Episode')) {
      console.log('RESP:', resp.status(), url);
    }
  });

  await page.goto('https://kisskh.ovh/Drama/My-Demon?id=8268&ep=1&page=0&pageSize=100', {
    waitUntil: 'networkidle',
    timeout: 30000
  });
  await page.waitForTimeout(5000);

  await browser.close();
})();
