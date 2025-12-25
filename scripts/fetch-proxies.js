const axios = require('axios');
const fs = require('fs');
const path = require('path');

// Fetch proxies from ProxyScrape
async function fetchProxies() {
    try {
        console.log('[proxy-fetch] Fetching fresh proxies from ProxyScrape...');
        
        const response = await axios.get('https://api.proxyscrape.com/v4/free-proxy-list/get?request=displayproxies&protocol=http&timeout=10000&country=all&ssl=all&anonymity=all&skip=0&limit=2000', {
            timeout: 30000
        });
        
        if (!response.data) {
            throw new Error('No data received from ProxyScrape');
        }
        
        // Parse proxies - they come as newline separated "ip:port"
        const proxyLines = response.data.split('\n').filter(line => line.trim());
        const proxies = proxyLines.map(line => line.trim()).filter(line => line.includes(':'));
        
        console.log(`[proxy-fetch] Fetched ${proxies.length} proxies`);
        
        // Update the proxy helper file
        const proxyHelperPath = path.join(__dirname, '..', 'utils', 'proxyHelper.js');
        let proxyHelperContent = fs.readFileSync(proxyHelperPath, 'utf8');
        
        // Replace the PROXY_LIST array
        const proxyListString = JSON.stringify(proxies, null, 8);
        const regex = /const PROXY_LIST = \[[\s\S]*?\];/;
        
        proxyHelperContent = proxyHelperContent.replace(regex, `const PROXY_LIST = ${proxyListString};`);
        
        fs.writeFileSync(proxyHelperPath, proxyHelperContent);
        console.log(`[proxy-fetch] Updated proxy list with ${proxies.length} proxies`);
        
        // Reset failed proxies to try fresh ones
        // Note: markProxyFailed is not available here, will be reset on server restart
        // Clear failed proxies by restarting the app or adding a reset function
        
        return proxies;
        
    } catch (error) {
        console.error('[proxy-fetch] Failed to fetch proxies:', error.message);
        return [];
    }
}

// Export for use in API
module.exports = { fetchProxies };

// Run if called directly
if (require.main === module) {
    fetchProxies().then(proxies => {
        console.log(`Successfully updated ${proxies.length} proxies`);
        process.exit(0);
    }).catch(err => {
        console.error('Failed to update proxies:', err);
        process.exit(1);
    });
}
