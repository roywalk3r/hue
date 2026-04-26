use aes::Aes256;
use cbc::{Decryptor, Encryptor}; // Note: Typo in 'Encryptor' often occurs, check correct trait
use cipher::{block_padding::Pkcs7, BlockDecryptMut, BlockEncryptMut, KeyIvInit};
use scrypt::{scrypt, Params};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use rand::RngCore;

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
    scrypt(secret.as_bytes(), b"salt", &params, &mut key).unwrap();
    key
}

pub fn encrypt_url(url: &str, secret: &str, headers: Option<HashMap<String, String>>) -> String {
    let expires = (chrono::Utc::now().timestamp_millis() as u64) + (4 * 60 * 60 * 1000);
    let payload = serde_json::to_vec(&DecryptedToken {
        url: url.to_string(),
        expires,
        headers,
    }).unwrap();

    let mut iv = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut iv);

    let key = get_key(secret);
    let cipher = Aes256CbcEnc::new(&key.into(), &iv.into());
    let ct = cipher.encrypt_padded_vec_mut::<Pkcs7>(&payload);

    format!("{}:{}", hex::encode(iv), hex::encode(ct))
}

pub fn decrypt_url(token: &str, secret: &str) -> Option<DecryptedToken> {
    let parts: Vec<&str> = token.split(':').collect();
    if parts.len() != 2 { return None; }

    let iv = hex::decode(parts[0]).ok()?;
    let encrypted_data = hex::decode(parts[1]).ok()?;

    let key = get_key(secret);
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
