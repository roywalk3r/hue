//! Firebase Cloud Messaging sender and Redis-backed token store.
//!
//! On first use, authenticates via OAuth2 using the service account JSON
//! from `FCM_SERVICE_ACCOUNT`. Tokens are cached in Redis under the key
//! `fcm:tokens` (a set of `<token>` strings).

use anyhow::{anyhow, Result};
use base64::Engine;
use deadpool_redis::Pool as RedisPool;
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

const OAUTH_URL: &str = "https://oauth2.googleapis.com/token";
const FCM_URL: &str = "https://fcm.googleapis.com/v1/projects";

#[derive(Deserialize)]
struct ServiceAccount {
    client_email: String,
    private_key: String,
    #[serde(rename = "project_id")]
    project_id: String,
}

#[derive(Serialize)]
struct JwtClaims {
    iss: String,
    scope: String,
    aud: String,
    iat: u64,
    exp: u64,
}

#[derive(Deserialize)]
struct OauthResponse {
    access_token: String,
}

/// Thread-safe FCM client. Authenticates lazily and reuses the access token
/// for up to 50 minutes (tokens live 1h).
#[derive(Clone)]
pub struct FcmClient {
    inner: Arc<FcmInner>,
}

struct FcmInner {
    client: Client,
    sa: ServiceAccount,
    token: Mutex<(String, Instant)>,
}

impl FcmClient {
    /// Build an FcmClient from a service-account JSON string.
    /// Returns `None` when FCM is not configured (empty or invalid SA).
    pub fn new(service_account_json: Option<String>) -> Option<Self> {
        let raw: String = service_account_json?.trim().to_string();
        if raw.is_empty() {
            return None;
        }
        let sa: ServiceAccount =
            serde_json::from_str(&raw).expect("invalid FCM_SERVICE_ACCOUNT JSON");
        Some(Self {
            inner: Arc::new(FcmInner {
                client: Client::builder()
                    .timeout(Duration::from_secs(30))
                    .build()
                    .unwrap(),
                sa,
                token: Mutex::new((String::new(), Instant::now())),
            }),
        })
    }

    /// Send a push notification to a single device token.
    pub async fn send(
        &self,
        fcm_token: &str,
        title: &str,
        body: &str,
        data: &Value,
    ) -> Result<()> {
        let access_token = self.inner.access_token().await?;
        let url = format!("{}/{}/messages:send", FCM_URL, self.inner.sa.project_id);
        let payload = serde_json::json!({
            "message": {
                "token": fcm_token,
                "notification": { "title": title, "body": body },
                "data": data,
                "android": {
                    "priority": "high",
                    "notification": { "channelId": "new_releases" }
                }
            }
        });
        let resp = self
            .inner
            .client
            .post(&url)
            .bearer_auth(&access_token)
            .json(&payload)
            .send()
            .await?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow!("FCM send failed {}: {}", status, body));
        }
        Ok(())
    }

    /// Send a multicast (up to 500 tokens at once).
    pub async fn send_batch(
        &self,
        tokens: &[String],
        title: &str,
        body: &str,
        data: &Value,
    ) -> Result<()> {
        let access_token = self.inner.access_token().await?;
        let url = format!("{}/{}/messages:send", FCM_URL, self.inner.sa.project_id);
        let chunks = tokens.chunks(500);
        for chunk in chunks {
            let payload = serde_json::json!({
                "message": {
                    "tokens": chunk,
                    "notification": { "title": title, "body": body },
                    "data": data,
                    "android": {
                        "priority": "high",
                        "notification": { "channelId": "new_releases" }
                    }
                }
            });
            let resp = self
                .inner
                .client
                .post(&url)
                .bearer_auth(&access_token)
                .json(&payload)
                .send()
                .await?;
            let status = resp.status();
            if !status.is_success() {
                let body = resp.text().await.unwrap_or_default();
                tracing::warn!("[FCM] batch error {}: {}", status, body);
            }
        }
        Ok(())
    }
}

impl FcmInner {
    async fn access_token(&self) -> Result<String> {
        {
            let guard = self.token.lock().await;
            // Tokens live 1h; refresh 10 min early.
            if guard.0.len() > 0 && guard.1.elapsed() < Duration::from_secs(50 * 60) {
                return Ok(guard.0.clone());
            }
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let claims = JwtClaims {
            iss: self.sa.client_email.clone(),
            scope: "https://www.googleapis.com/auth/firebase.messaging".into(),
            aud: OAUTH_URL.to_string(),
            iat: now,
            exp: now + 3600,
        };
        let key =
            EncodingKey::from_rsa_pem(self.sa.private_key.as_bytes())?;
        let jwt = encode(&Header::new(Algorithm::RS256), &claims, &key)?;
        let resp = self
            .client
            .post(OAUTH_URL)
            .form(&[
                ("grant_type", "urn:ietf:params:oauth:grant-type:jwt-bearer"),
                ("assertion", &jwt),
            ])
            .send()
            .await?;
        let oauth: OauthResponse = resp.json().await?;
        let mut guard = self.token.lock().await;
        *guard = (oauth.access_token.clone(), Instant::now());
        Ok(oauth.access_token)
    }
}

// ── Redis token store ──────────────────────────────────────────────────────

const TOKENS_KEY: &str = "fcm:tokens";

/// Store an FCM device token in Redis.
pub async fn store_token(pool: &RedisPool, token: &str) -> Result<()> {
    let mut conn = pool.get().await?;
    deadpool_redis::redis::cmd("SADD")
        .arg(TOKENS_KEY)
        .arg(token)
        .query_async::<_, ()>(&mut conn)
        .await?;
    Ok(())
}

/// Retrieve all known FCM device tokens from Redis.
pub async fn all_tokens(pool: &RedisPool) -> Result<Vec<String>> {
    let mut conn = pool.get().await?;
    let tokens: Vec<String> = deadpool_redis::redis::cmd("SMEMBERS")
        .arg(TOKENS_KEY)
        .query_async(&mut conn)
        .await?;
    Ok(tokens)
}
