const { HttpsProxyAgent } = require('https-proxy-agent');

// List of proxies from proxyscrape
const PROXY_LIST = [
    '209.50.163.24:3129',
    '154.213.166.159:3129',
    '216.26.248.102:3129',
    '216.26.229.134:3129',
    '209.50.165.30:3129',
    '104.207.59.204:3129',
    '104.207.54.79:3129',
    '216.26.239.220:3129',
    '154.213.163.46:3129',
    '45.3.55.77:3129',
    '65.111.23.85:3129',
    '216.26.235.113:3129',
    '45.3.50.53:3129',
    '65.111.23.19:3129',
    '65.111.15.88:3129',
    '209.50.169.51:3129',
    '216.26.240.96:3129',
    '216.26.238.166:3129',
    '45.3.34.20:3129',
    '65.111.2.127:3129',
    '104.207.49.33:3129',
    '45.3.49.6:3129',
    '216.26.226.104:3129',
    '209.50.160.73:3129',
    '104.207.62.157:3129',
    '209.50.183.27:3129',
    '216.26.245.223:3129',
    '104.207.51.171:3129',
    '104.207.58.205:3129',
    '216.26.239.63:3129',
    '216.26.252.87:3129',
    '209.50.191.135:3129',
    '104.207.58.81:3129',
    '209.50.189.70:3129',
    '65.111.24.134:3129',
    '216.26.244.247:3129',
    '216.26.247.84:3129',
    '65.111.21.23:3129',
    '65.111.12.16:3129',
    '209.50.168.50:3129',
    '104.207.61.250:3129',
    '216.26.239.157:3129',
    '216.26.235.91:3129',
    '104.207.34.158:3129',
    '104.207.51.201:3129',
    '65.111.28.220:3129',
    '216.26.243.136:3129',
    '209.50.161.179:3129',
    '65.111.23.177:3129',
    '216.26.227.106:3129',
    '216.26.229.66:3129',
    '209.50.191.180:3129',
    '45.3.44.139:3129',
    '65.111.6.241:3129',
    '104.207.40.56:3129',
    '209.50.190.147:3129',
    '65.111.9.104:3129',
    '193.56.28.151:3129',
    '209.50.164.196:3129',
    '154.213.162.138:3129',
    '65.111.5.163:3129',
    '65.111.22.167:3129',
    '45.3.47.210:3129',
    '216.26.248.92:3129',
    '45.3.33.27:3129',
    '216.26.237.107:3129',
    '45.3.36.194:3129',
    '45.3.34.210:3129',
    '209.50.168.152:3129',
    '216.26.248.209:3129',
    '65.111.5.137:3129',
    '104.207.57.155:3129',
    '45.3.41.194:3129',
    '65.111.23.215:3129',
    '216.26.246.184:3129',
    '209.50.184.83:3129',
    '104.207.43.8:3129',
    '209.50.162.172:3129',
    '104.207.54.246:3129',
    '65.111.15.204:3129',
    '65.111.25.109:3129',
    '104.167.19.143:3129',
    '209.50.188.166:3129',
    '45.3.41.179:3129',
    '209.50.178.198:3129',
    '216.26.238.56:3129',
    '216.26.252.101:3129',
    '104.207.51.215:3129',
    '65.111.7.61:3129',
    '65.111.2.109:3129',
    '209.50.164.59:3129',
    '65.111.15.137:3129',
    '104.207.58.4:3129',
    '65.111.30.91:3129',
    '45.3.47.246:3129',
    '209.50.189.166:3129',
    '65.111.4.219:3129',
    '216.26.231.138:3129',
    '104.207.34.119:3129',
    '104.207.41.218:3129'
];

let currentProxyIndex = 0;
let failedProxies = new Set();

// Get next working proxy
function getNextProxy() {
    const attempts = 0;
    while (attempts < PROXY_LIST.length) {
        const proxy = PROXY_LIST[currentProxyIndex];
        currentProxyIndex = (currentProxyIndex + 1) % PROXY_LIST.length;
        
        if (!failedProxies.has(proxy)) {
            return proxy;
        }
    }
    // If all proxies failed, reset and try again
    failedProxies.clear();
    return PROXY_LIST[0];
}

// Mark proxy as failed
function markProxyFailed(proxy) {
    failedProxies.add(proxy);
    console.log(`[proxy] Marked proxy as failed: ${proxy} (${failedProxies.size}/${PROXY_LIST.length} failed)`);
}

// Create axios config with proxy
function createAxiosConfig(baseConfig = {}) {
    const useProxy = process.env.USE_PROXIES !== 'false';
    
    if (!useProxy) {
        return baseConfig;
    }
    
    const proxy = getNextProxy();
    const proxyUrl = `http://${proxy}`;
    
    return {
        ...baseConfig,
        httpsAgent: new HttpsProxyAgent(proxyUrl),
        httpAgent: new HttpsProxyAgent(proxyUrl),
        proxy: false // Disable axios's built-in proxy
    };
}

// Retry request with different proxy
async function requestWithRetry(requestFn, maxRetries = 3) {
    let lastError;
    
    for (let i = 0; i < maxRetries; i++) {
        try {
            return await requestFn();
        } catch (error) {
            lastError = error;
            
            // If it's a proxy-related error or 403, try next proxy
            if (error.code === 'ECONNRESET' || 
                error.code === 'ECONNREFUSED' || 
                error.response?.status === 403 ||
                error.response?.status === 429) {
                
                if (i < maxRetries - 1) {
                    console.log(`[proxy] Request failed, retrying with new proxy...`);
                    continue;
                }
            }
            break;
        }
    }
    
    throw lastError;
}

module.exports = {
    createAxiosConfig,
    requestWithRetry,
    getNextProxy,
    markProxyFailed
};
