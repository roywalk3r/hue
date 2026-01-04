/**
 * Test Livesport API endpoints
 */

const axios = require('axios');

const baseURL = 'https://livesport.su/api';

async function testEndpoint(endpoint, description) {
    console.log(`\n=== Testing ${description} ===`);
    console.log(`URL: ${baseURL}${endpoint}`);
    
    try {
        const response = await axios.get(`${baseURL}${endpoint}`, {
            timeout: 10000
        });
        
        console.log(`✅ Status: ${response.status}`);
        console.log(`Data type: ${typeof response.data}`);
        console.log(`Is array: ${Array.isArray(response.data)}`);
        
        if (Array.isArray(response.data)) {
            console.log(`Array length: ${response.data.length}`);
            
            if (response.data.length > 0) {
                console.log(`First item keys: ${Object.keys(response.data[0]).join(', ')}`);
            }
        } else if (response.data && typeof response.data === 'object') {
            console.log(`Object keys: ${Object.keys(response.data).join(', ')}`);
        }
        
        console.log(`Response (first 200 chars): ${JSON.stringify(response.data).substring(0, 200)}...`);
        
    } catch (error) {
        console.log(`❌ Error: ${error.message}`);
        if (error.response) {
            console.log(`Status: ${error.response.status}`);
            console.log(`Data: ${JSON.stringify(error.response.data)}`);
        }
    }
}

async function main() {
    console.log('Testing Livesport API endpoints...\n');
    
    // Test various endpoints
    await testEndpoint('/sports', 'Sports');
    await testEndpoint('/matches', 'All Matches');
    await testEndpoint('/matches/live', 'Live Matches');
    await testEndpoint('/matches/live/popular', 'Popular Live Matches');
    await testEndpoint('/matches/all-today', "Today's Matches");
    await testEndpoint('/matches/football', 'Football Matches');
    
    // Test a specific match detail (using a known match ID from the HTML)
    await testEndpoint('/matches/2267251/detail', 'Match Details (Sunderland vs Leeds)');
}

main().catch(console.error);
