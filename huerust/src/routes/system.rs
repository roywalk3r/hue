//! System handlers for monitoring, health, and administration

use axum::{
    extract::State,
    response::{IntoResponse, Response},
    http::StatusCode,
};
use tracing::{info, warn};
use crate::AppState;

pub async fn health_check() -> &'static str { "Huerust is Alive" }

pub async fn metrics_handler(State(state): State<AppState>) -> Response {
    let body = state.metrics.render_prometheus();
    (
        StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        body,
    ).into_response()
}

pub async fn hitv_library_handler(State(state): State<AppState>) -> Response {
    let items = state.hitv.get_library().await;
    (StatusCode::OK, serde_json::to_string(&items).unwrap()).into_response()
}

pub async fn admin_reload_handler() -> Response {
    crate::user_config::HueUserConfig::invalidate();
    (StatusCode::OK, "{\"ok\":true,\"message\":\"Config cache invalidated — next request reloads from disk\"}").into_response()
}

pub async fn admin_refresh_tokens_handler(State(state): State<AppState>) -> Response {
    let node_api_url = std::env::var("NODE_API_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:3003".to_string());
    let url = format!("{}/system/refresh-tokens", node_api_url);

    match state.client.post(&url).send().await {
        Ok(resp) => {
            crate::user_config::HueUserConfig::invalidate();
            let status = if resp.status().is_success() { StatusCode::OK } else { StatusCode::BAD_GATEWAY };
            let body = resp.text().await.unwrap_or_default();
            info!("[admin] Token refresh response: status={}, body={}", status, body);
            (status, format!("{{\"ok\":true,\"message\":\"Token refresh triggered\",\"node_response\":{}}}", body)).into_response()
        }
        Err(e) => {
            warn!("[admin] refresh-tokens call to Node.js failed: {}", e);
            (StatusCode::BAD_GATEWAY, format!("{{\"ok\":false,\"error\":\"Node.js unreachable: {}\"}}", e)).into_response()
        }
    }
}

pub async fn status_handler(State(state): State<AppState>) -> Response {
    let uptime_secs = state.start_time.elapsed().as_secs();

    let providers: Vec<serde_json::Value> = state.registry
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
    (StatusCode::OK, serde_json::to_string(&resp).unwrap()).into_response()
}
