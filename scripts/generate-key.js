const fs = require('fs');
const path = require('path');
const crypto = require('crypto');

const envPath = path.join(__dirname, '../.env');

function generateKey() {
    // Generate a secure 64-character hex string (32 bytes)
    const key = crypto.randomBytes(32).toString('hex');
    
    let content = '';
    if (fs.existsSync(envPath)) {
        content = fs.readFileSync(envPath, 'utf8');
    }

    if (content.includes('APP_SECRET=')) {
        // If it exists and has a value, don't overwrite unless forced (Laravel style)
        const match = content.match(/APP_SECRET=([^\n]*)/);
        if (match && match[1].trim().length > 0) {
            console.log('[Security] APP_SECRET already exists. Skipping generation.');
            return;
        }
        // Update empty value
        content = content.replace(/APP_SECRET=([^\n]*)/, `APP_SECRET=${key}`);
    } else {
        // Append to file
        content += `\nAPP_SECRET=${key}\n`;
    }

    fs.writeFileSync(envPath, content.trim() + '\n');
    console.log(`[Security] Generated new APP_SECRET and saved to .env`);
}

generateKey();
