/**
 * Hue Stream API
 * Professional Streaming Aggregator
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */

const CryptoJS = require('crypto-js');

function decodeVidzeeToken(token) {
    try {
        // Normalize base64
        let normalized = token
            .replace(/-/g, '+')
            .replace(/_/g, '/')
            .replace(/\s/g, '+') // Replace spaces with +
            .replace(/[^A-Za-z0-9+/=]/g, '');
        
        // Pad the string
        const padLength = 4 - (normalized.length % 4);
        if (padLength !== 4) {
            normalized += '='.repeat(padLength);
        }
        
        console.log('Normalized:', normalized.substring(0, 100) + '...');
        
        // Decode base64
        const raw = Buffer.from(normalized, 'base64').toString('binary');
        console.log('Raw decoded length:', raw.length);
        
        // Extract IV and cipher
        const colonIndex = raw.indexOf(':');
        if (colonIndex === -1) {
            console.log('No colon found, trying different format');
            return null;
        }
        
        const ivB64 = raw.substring(0, colonIndex);
        const cipherB64 = raw.substring(colonIndex + 1);
        
        console.log('IV length:', ivB64.length);
        console.log('Cipher length:', cipherB64.length);
        
        // Create key
        const key = CryptoJS.enc.Utf8.parse('qrincywincyspider'.padEnd(32, '\0'));
        
        // Decrypt
        const iv = CryptoJS.enc.Base64.parse(ivB64);
        const cipher = CryptoJS.enc.Base64.parse(cipherB64);
        
        const decrypted = CryptoJS.AES.decrypt(
            { ciphertext: cipher },
            key,
            { iv: iv, mode: CryptoJS.mode.CBC, padding: CryptoJS.pad.Pkcs7 }
        );
        
        const result = decrypted.toString(CryptoJS.enc.Utf8);
        console.log('Decrypted:', result);
        
        return result;
    } catch (e) {
        console.error('Decode error:', e.message);
        return null;
    }
}

// Test with the vidsrc hash
const testHash = "8gRy+i3edPABdONqmPrWyatBJeIA4vqnieEhIVqDwbQuHk-th65FbLxxQKLGpDhyRBcAuFuunJPaLuzFahqTKeYnNQshPSoKHPfcIZqtzuXpEnuSuOIMS-ez5qVXSJLJ8VJI-d-o6sVrX0ibwaB7xkPEUzJhhGY+xkX+KWfJydySyCDDgUTy5QxbDYcf68Uft";
const result = decodeVidzeeToken(testHash);
console.log('\nFinal result:', result);
