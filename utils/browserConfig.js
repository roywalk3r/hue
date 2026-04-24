const DEFAULT_ARGS = [
    '--no-sandbox',
    '--disable-setuid-sandbox',
    '--disable-dev-shm-usage',
    '--disable-accelerated-2d-canvas',
    '--no-first-run',
    '--no-zygote',
    '--disable-gpu',
    '--disable-blink-features=AutomationControlled'
];

function parseEnvArgs(raw) {
    if (!raw || typeof raw !== 'string') return [];
    // Simple split on whitespace; trim empty tokens
    return raw.split(/\s+/).filter(Boolean);
}

function buildLaunchOptions(overrides = {}) {
    const envArgs = parseEnvArgs(process.env.BROWSER_ARGS || process.env.PUPPETEER_ARGS);
    const mergedArgs = Array.from(new Set([...DEFAULT_ARGS, ...(overrides.args || [])]));
    const executablePath = process.env.BROWSER_EXECUTABLE_PATH || process.env.PUPPETEER_EXECUTABLE_PATH || overrides.executablePath;
    const headlessFlag = process.env.BROWSER_HEADLESS === 'false' || process.env.PUPPETEER_HEADLESS === 'false' ? false : true;

    const opts = {
        headless: headlessFlag,
        args: [
            ...mergedArgs,
            '--log-level=3', // Fatal only
            '--silent-launch',
            '--disable-logging',
            '--no-report-upload'
        ],
        dumpio: false 
    };

    if (executablePath) {
        opts.executablePath = executablePath;
    }

    return opts;
}

module.exports = { buildLaunchOptions };