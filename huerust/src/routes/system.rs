//! System handlers for monitoring, health, and administration

use crate::app::{extract_client_ip, SystemState};
use axum::{
    extract::State,
    http::{Request, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use tracing::{info, warn};

pub async fn health_check() -> &'static str {
    "Huerust is Alive"
}

pub fn build_info() -> serde_json::Value {
    serde_json::json!({
        "service": "huerust",
        "version": env!("CARGO_PKG_VERSION"),
        "commit": env!("HUE_GIT_SHA"),
        "dirty": env!("HUE_GIT_DIRTY") == "true",
    })
}

pub async fn health_version() -> Json<serde_json::Value> {
    Json(build_info())
}

#[cfg(test)]
mod tests {
    use super::build_info;

    #[test]
    fn build_info_identifies_the_deployed_binary() {
        let info = build_info();

        assert_eq!(info["service"], "huerust");
        assert_eq!(info["version"], env!("CARGO_PKG_VERSION"));
        assert!(info["commit"]
            .as_str()
            .is_some_and(|value| !value.is_empty()));
    }
}

pub async fn metrics_handler(State(state): State<SystemState>) -> Response {
    let body = state.metrics.render_prometheus();
    (
        StatusCode::OK,
        [(
            axum::http::header::CONTENT_TYPE,
            "text/plain; charset=utf-8",
        )],
        body,
    )
        .into_response()
}

pub async fn hitv_library_handler(State(state): State<SystemState>) -> Response {
    let items = state.hitv.get_library().await;
    (StatusCode::OK, Json(items)).into_response()
}

pub async fn admin_reload_handler() -> Response {
    crate::user_config::HueUserConfig::invalidate();
    (
        StatusCode::OK,
        "{\"ok\":true,\"message\":\"Config cache invalidated — next request reloads from disk\"}",
    )
        .into_response()
}

pub async fn admin_refresh_tokens_handler(State(state): State<SystemState>) -> Response {
    let node_api_url =
        std::env::var("NODE_API_URL").unwrap_or_else(|_| "http://127.0.0.1:3003".to_string());
    let url = format!("{}/system/refresh-tokens", node_api_url);

    match state.client.post(&url).send().await {
        Ok(resp) => {
            crate::user_config::HueUserConfig::invalidate();
            let status = if resp.status().is_success() {
                StatusCode::OK
            } else {
                StatusCode::BAD_GATEWAY
            };
            let body = resp.text().await.unwrap_or_default();
            info!(
                "[admin] Token refresh response: status={}, body={}",
                status, body
            );
            (
                status,
                format!(
                    "{{\"ok\":true,\"message\":\"Token refresh triggered\",\"node_response\":{}}}",
                    body
                ),
            )
                .into_response()
        }
        Err(e) => {
            warn!("[admin] refresh-tokens call to Node.js failed: {}", e);
            (
                StatusCode::BAD_GATEWAY,
                format!("{{\"ok\":false,\"error\":\"Node.js unreachable: {}\"}}", e),
            )
                .into_response()
        }
    }
}

pub async fn status_handler(State(state): State<SystemState>) -> Response {
    let uptime_secs = state.start_time.elapsed().as_secs();

    let providers: Vec<serde_json::Value> = state
        .registry
        .list_all_arc()
        .iter()
        .map(|p| {
            serde_json::json!({
                "name": p.real_name(),
                "enabled": true
            })
        })
        .collect();

    let resp = serde_json::json!({
        "success": true,
        "version": "3.1.0",
        "uptime": uptime_secs,
        "metrics": {},
        "providers": providers
    });
    (StatusCode::OK, Json(resp)).into_response()
}

#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct AuthTokenResponse {
    /// Bearer token. Send in `Authorization: Bearer <token>`.
    pub token: String,
    /// Absolute expiry in ms since the Unix epoch.
    pub expires: u64,
    /// Lifetime in ms.
    pub expires_in: u64,
}

/// Mint a short-lived HMAC-signed Bearer token. The token is bound to the
/// caller's IP so it cannot be replayed from a different egress. TTL is 1
/// hour; clients refresh proactively.
#[utoipa::path(
    get,
    path = "/api/auth/token",
    tag = "system",
    responses(
        (status = 200, description = "Token minted", body = AuthTokenResponse),
    )
)]
pub async fn auth_token_handler(
    State(state): State<SystemState>,
    req: Request<axum::body::Body>,
) -> Response {
    let client_ip = extract_client_ip(&req);
    let ttl_ms: u64 = 60 * 60 * 1000;
    let (token, expires) =
        crate::security::sign_api_token(&state.config.secret, &client_ip, ttl_ms);
    info!(
        "[auth] minted token (ip={}, ttl_ms={}, expires_ms={})",
        client_ip, ttl_ms, expires
    );
    (
        StatusCode::OK,
        Json(AuthTokenResponse {
            token,
            expires,
            expires_in: ttl_ms,
        }),
    )
        .into_response()
}
