const crypto = require('crypto');

// Use a persistent secret from ENV or generate a random one on startup
const SECRET = process.env.APP_SECRET || 'super-secret-hue-key-change-me';

// 1. Generate a Signed URL Token
// Contains: Original URL, Expiry, IP (optional), Headers (optional)
function encryptUrl(url, ip = null, headers = null) {
    const expires = Date.now() + (4 * 60 * 60 * 1000); // 4 Hour Expiry
    const payload = JSON.stringify({ u: url, e: expires, i: ip, h: headers });
    
    const iv = crypto.randomBytes(16);
    const cipher = crypto.createCipheriv('aes-256-cbc', crypto.scryptSync(SECRET, 'salt', 32), iv);
    let encrypted = cipher.update(payload);
    encrypted = Buffer.concat([encrypted, cipher.final()]);
    
    // Return IV:Encrypted (Base64url safe)
    return iv.toString('hex') + ':' + encrypted.toString('hex');
}

// 2. Decrypt and Verify
function decryptUrl(token, reqIp = null) {
    try {
        if (!token) return null;
        const parts = token.split(':');
        if (parts.length !== 2) {
            console.error('[Security] Invalid token format (missing colon)');
            return null;
        }
        
        const iv = Buffer.from(parts[0], 'hex');
        const encryptedText = Buffer.from(parts[1], 'hex');
        
        // Block size check to avoid "wrong final block length"
        if (encryptedText.length % 16 !== 0) {
            console.error(`[Security] Decrypt failed: Token data length (${encryptedText.length}) is not a multiple of block size. Likely truncated.`);
            return null;
        }

        const decipher = crypto.createDecipheriv('aes-256-cbc', crypto.scryptSync(SECRET, 'salt', 32), iv);
        let decrypted = decipher.update(encryptedText);
        decrypted = Buffer.concat([decrypted, decipher.final()]);
        
        const data = JSON.parse(decrypted.toString());
        
        // Check Expiry
        if (data.e < Date.now()) {
            console.warn('[Security] Token expired');
            return null;
        }

        // Return object with url and headers
        return { url: data.u, headers: data.h };
    } catch (e) {
        if (e.message.includes('wrong final block length') || e.message.includes('bad decrypt')) {
            console.error('[Security] Decrypt failed: Secret mismatch or malformed data. Refresh the stream URL.');
        } else {
            console.error('[Security] Decrypt failed:', e.message);
        }
        return null;
    }
}

// 3. API Key / Referer Middleware
function authMiddleware(req, res, next) {
    const apiKey = req.headers['x-api-key'];
    const referer = req.headers['referer'] || req.headers['origin'];
    
    // Allow localhost for dev
    if (req.ip === '127.0.0.1' || req.ip === '::1') return next();

    // 1. Check API Key (if configured)
    if (process.env.API_KEY && apiKey !== process.env.API_KEY) {
        return res.status(403).json({ error: 'Invalid API Key' });
    }

    // 2. Check Origin/Referer (Prevent direct browser access from other sites)
    // const allowed = ['http://localhost', 'https://your-domain.com'];
    // if (referer && !allowed.some(d => referer.startsWith(d))) {
    //    return res.status(403).json({ error: 'Origin not allowed' });
    // }

    next();
}

module.exports = { encryptUrl, decryptUrl, authMiddleware };
