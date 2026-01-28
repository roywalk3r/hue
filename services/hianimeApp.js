const { pathToFileURL } = require('url');
const path = require('path');

let hianimeAppPromise;

async function getHianimeApp() {
  if (typeof fetch === 'undefined') {
    const fetchImpl = require('node-fetch');
    global.fetch = fetchImpl;
    global.Headers = fetchImpl.Headers;
    global.Request = fetchImpl.Request;
    global.Response = fetchImpl.Response;
  }
  if (!hianimeAppPromise) {
    const appPath = pathToFileURL(
      path.join(__dirname, '../vendor/hianime-v1/src/app.js')
    ).href;
    hianimeAppPromise = import(appPath).then((mod) => mod.default);
  }
  return hianimeAppPromise;
}

module.exports = {
  getHianimeApp,
};
