//! Hue Stream API
//! Professional Streaming Aggregator
//!
//! @author roywalk3r <roywalker440@gmail.com>
//! @repository https://github.com/roywalk3r/hue.git
//! @license MIT

use aes::Aes256;
use cbc::{Decryptor, Encryptor};
use cipher::{block_padding::Pkcs7, BlockDecryptMut, BlockEncryptMut, KeyIvInit};
use rand::RngCore;
use scrypt::{scrypt, Params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

type Aes256CbcDec = Decryptor<Aes256>;
type Aes256CbcEnc = Encryptor<Aes256>;

#[derive(Debug, Serialize, Deserialize)]
pub struct DecryptedToken {
    #[serde(rename = "u")]
    pub url: String,
    #[serde(rename = "e")]
    pub expires: u64,
    #[serde(rename = "h")]
    pub headers: Option<HashMap<String, String>>,
}

fn get_key(secret: &str) -> [u8; 32] {
    let mut key = [0u8; 32];
    let params = Params::new(14, 8, 1, 32).unwrap();
    let salt: [u8; 16] = Sha256::digest(secret.as_bytes())[..16].try_into().unwrap();
    scrypt(secret.as_bytes(), &salt, &params, &mut key).unwrap();
    key
}

// Memoize the expensive scrypt derivation — called once per unique secret per process.
static KEY_CACHE: OnceLock<Mutex<HashMap<String, [u8; 32]>>> = OnceLock::new();

fn get_key_memoized(secret: &str) -> [u8; 32] {
    let cache = KEY_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    {
        let map = cache.lock().unwrap();
        if let Some(&k) = map.get(secret) {
            return k;
        }
    }
    let k = get_key(secret);
    cache.lock().unwrap().insert(secret.to_string(), k);
    k
}

/// Returns true if a raw string containing a JWT has an expired `exp` claim.
/// Accepts raw JWTs, URL-encoded cookie strings, or JSON objects containing a JWT.
pub fn jwt_is_expired(raw: &str) -> bool {
    // All JWTs start with the base64url encoding of '{"' which is 'eyJ'
    let start = match raw.find("eyJ") {
        Some(i) => i,
        None => return true,
    };
    let rest = &raw[start..];
    // JWT ends at any character that cannot appear in a base64url-encoded JWT
    let end = rest
        .find(|c: char| c == '%' || c == '"' || c == ';' || c == '\'')
        .unwrap_or(rest.len());
    let jwt_str = &rest[..end];

    let parts: Vec<&str> = jwt_str.splitn(3, '.').collect();
    if parts.len() < 2 {
        return true;
    }

    let payload = parts[1];
    let rem = payload.len() % 4;
    let padded = if rem > 0 {
        format!("{}{}", payload, "=".repeat(4 - rem))
    } else {
        payload.to_string()
    };

    use base64::{engine::general_purpose, Engine as _};
    let bytes = match general_purpose::URL_SAFE.decode(&padded) {
        Ok(b) => b,
        Err(_) => return true,
    };

    let json: serde_json::Value = match serde_json::from_slice(&bytes) {
        Ok(v) => v,
        Err(_) => return true,
    };

    let exp = json["exp"].as_u64().unwrap_or(0);
    let now = chrono::Utc::now().timestamp() as u64;
    exp < now
}

pub fn encrypt_url(url: &str, secret: &str, headers: Option<HashMap<String, String>>) -> String {
    let expires = (chrono::Utc::now().timestamp_millis() as u64) + (4 * 60 * 60 * 1000);
    let payload = serde_json::to_vec(&DecryptedToken {
        url: url.to_string(),
        expires,
        headers,
    })
    .unwrap();

    let mut iv = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut iv);

    let key = get_key_memoized(secret);
    let cipher = Aes256CbcEnc::new(&key.into(), &iv.into());
    let ct = cipher.encrypt_padded_vec_mut::<Pkcs7>(&payload);

    format!("{}:{}", hex::encode(iv), hex::encode(ct))
}

pub fn decrypt_url(token: &str, secret: &str) -> Option<DecryptedToken> {
    let parts: Vec<&str> = token.split(':').collect();
    if parts.len() != 2 {
        return None;
    }

    let iv = hex::decode(parts[0]).ok()?;
    let encrypted_data = hex::decode(parts[1]).ok()?;

    let key = get_key_memoized(secret);
    let pt = Aes256CbcDec::new(&key.into(), iv.as_slice().into())
        .decrypt_padded_vec_mut::<Pkcs7>(&encrypted_data)
        .ok()?;

    let token: DecryptedToken = serde_json::from_slice(&pt).ok()?;
    let now = chrono::Utc::now().timestamp_millis() as u64;
    if token.expires < now {
        return None;
    }
    Some(token)
}

// ── Kkey response encryption (KissKH-style obfuscation) ─────────────────────

/// Hourly-rotating XOR cipher for response URL obfuscation.
/// Format: "{epoch_hour}:{base64(xor(url, key))}"
pub fn kkey_encrypt(url: &str, kkey: &str) -> String {
    let epoch_hour = chrono::Utc::now().timestamp() / 3600;
    let seed = format!("{}{}", kkey, epoch_hour);
    let key = sha2::Sha256::digest(seed.as_bytes());
    let mut out: Vec<u8> = url.bytes().map(|b| b).collect();
    for (i, byte) in out.iter_mut().enumerate() {
        *byte ^= key[i % key.len()];
    }
    format!("{}:{}", epoch_hour, hex::encode(&out))
}

#[allow(dead_code)]
pub fn kkey_decrypt(token: &str, kkey: &str) -> Option<String> {
    let parts: Vec<&str> = token.splitn(2, ':').collect();
    if parts.len() != 2 {
        return None;
    }
    let epoch_hour: i64 = parts[0].parse().ok()?;
    let encrypted = hex::decode(parts[1]).ok()?;
    let seed = format!("{}{}", kkey, epoch_hour);
    let key = sha2::Sha256::digest(seed.as_bytes());
    let mut out = encrypted;
    for (i, byte) in out.iter_mut().enumerate() {
        *byte ^= key[i % key.len()];
    }
    String::from_utf8(out).ok()
}

/// Encrypt all stream URLs and subtitle URLs in a response using the kkey.
/// Call this AFTER wrap_with_proxy so the h_token URLs are encrypted again.
pub fn encrypt_stream_result_urls(streams: &mut [crate::models::StreamResult], kkey: &str) {
    for s in streams.iter_mut() {
        if !s.url.is_empty() {
            s.url = kkey_encrypt(&s.url, kkey);
        }
        if let Some(ref mut subs) = s.subtitles {
            for sub in subs.iter_mut() {
                if !sub.url.is_empty() {
                    sub.url = kkey_encrypt(&sub.url, kkey);
                }
            }
        }
    }
}

/// Re-encrypt cached stream URLs with the current epoch hour so clients
/// can always decrypt them — avoids the double-encryption bug where
/// `encrypt_stream_result_urls` is called on already-encrypted cache entries.
pub fn re_encrypt_cached_urls(streams: &mut [crate::models::StreamResult], kkey: &str) {
    for s in streams.iter_mut() {
        if s.url.contains(':') {
            if let Some(dec) = kkey_decrypt(&s.url, kkey) {
                s.url = kkey_encrypt(&dec, kkey);
            }
        }
        if let Some(ref mut subs) = s.subtitles {
            for sub in subs.iter_mut() {
                if sub.url.contains(':') {
                    if let Some(dec) = kkey_decrypt(&sub.url, kkey) {
                        sub.url = kkey_encrypt(&dec, kkey);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let secret = "test-secret-12345678901234567890";
        let url = "https://example.com/stream.m3u8";
        let mut headers = HashMap::new();
        headers.insert("Referer".to_string(), "https://example.com/".to_string());

        let token = encrypt_url(url, secret, Some(headers.clone()));
        let decrypted = decrypt_url(&token, secret).unwrap();

        assert_eq!(decrypted.url, url);
        assert_eq!(decrypted.headers, Some(headers));
    }

    #[test]
    fn test_decrypt_invalid_token() {
        let secret = "test-secret";
        assert!(decrypt_url("badtoken", secret).is_none());
        assert!(decrypt_url("", secret).is_none());
    }

    #[test]
    fn test_kkey_roundtrip() {
        let kkey = "my-secret-kkey";
        let url = "https://example.com/stream.m3u8?token=abc123";
        let encrypted = kkey_encrypt(url, kkey);
        let decrypted = kkey_decrypt(&encrypted, kkey).unwrap();
        assert_eq!(decrypted, url);
    }

    #[test]
    fn test_kkey_decrypt_invalid() {
        assert!(kkey_decrypt("badtoken", "kkey").is_none());
        assert!(kkey_decrypt("", "kkey").is_none());
    }

    #[test]
    fn test_kkey_wrong_key_fails() {
        let url = "https://example.com/stream.m3u8";
        let encrypted = kkey_encrypt(url, "correct-key");
        let decrypted = kkey_decrypt(&encrypted, "wrong-key");
        // Wrong key produces garbage, not the original URL
        assert!(decrypted.is_none() || decrypted.unwrap() != url);
    }
}
