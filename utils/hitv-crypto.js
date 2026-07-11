/**
 * HUE ENGINE ❄
 * System Designation: The Architecture of Aggregation
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */

/**
 * HiTV AES-128-CBC decryption
 *
 * Key derivation:
 *   inner = MD5(did + ts)
 *   outer = MD5(inner + SALT)
 *   key   = outer.slice(0, 16)
 *   iv    = outer.slice(16)
 */

const crypto = require('crypto');

const SALT = 'Wcb26arWkvkcAZc378eR';

function md5(s) {
  return crypto.createHash('md5').update(String(s)).digest('hex');
}

function decrypt(data, ts, did) {
  const inner = md5(did + ts);
  const outer = md5(inner + SALT);
  const key = outer.slice(0, 16);
  const iv = outer.slice(16);
  try {
    const decipher = crypto.createDecipheriv('aes-128-cbc', key, iv);
    let decrypted = decipher.update(Buffer.from(data, 'base64'));
    decrypted = Buffer.concat([decrypted, decipher.final()]);
    return decrypted.toString('utf8');
  } catch (e) {
    return null;
  }
}

function parseEncryptedResponse(body) {
  try {
    const json = typeof body === 'string' ? JSON.parse(body) : body;
    if (!json.ts || !json.data) return null;
    return { ts: json.ts, data: json.data };
  } catch {
    return null;
  }
}

module.exports = { SALT, md5, decrypt, parseEncryptedResponse };
