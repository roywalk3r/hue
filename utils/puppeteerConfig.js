const DEFAULT_ARGS = [
    '--no-sandbox',
    '--disable-setuid-sandbox',
    '--disable-dev-shm-usage',
    '--disable-accelerated-2d-canvas',
    '--no-first-run',
    '--no-zygote',
    '--single-process',
    '--disable-gpu',
    '--disable-blink-features=AutomationControlled',
    '--disable-features=VizDisplayCompositor'
];

function parseEnvArgs(raw) {
    if (!raw || typeof raw !== 'string') return [];
    // Simple split on whitespace; trim empty tokens
    return raw.split(/\s+/).filter(Boolean);
}

function buildLaunchOptions(overrides = {}) {
    const envArgs = parseEnvArgs(process.env.PUPPETEER_ARGS);
    const mergedArgs = Array.from(new Set([...(overrides.args || DEFAULT_ARGS), ...envArgs]));

    const executablePath = process.env.PUPPETEER_EXECUTABLE_PATH || overrides.executablePath;
    const headlessFlag = process.env.PUPPETEER_HEADLESS === 'false' ? false : true;

    const opts = {
        headless: headlessFlag,
        args: mergedArgs
    };

    if (executablePath) {
        opts.executablePath = executablePath;
    }

    return opts;
}

module.exports = { buildLaunchOptions };
