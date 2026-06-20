use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use tracing::info;

use crate::infra::fcm;

use crate::app::AppState;

#[derive(Deserialize)]
pub struct RegisterPayload {
    fcm_token: String,
    #[serde(default)]
    platform: Option<String>,
}

pub async fn register_handler(
    State(state): State<AppState>,
    Json(payload): Json<RegisterPayload>,
) -> Response {
    if payload.fcm_token.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "success": false, "error": "fcm_token required" })),
        )
            .into_response();
    }
    match &state.redis_pool {
        Some(pool) => match fcm::store_token(pool, &payload.fcm_token).await {
            Ok(()) => {
                info!("[notifications] token registered");
                (
                    StatusCode::OK,
                    Json(serde_json::json!({ "success": true })),
                )
                    .into_response()
            }
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "success": false, "error": e.to_string() })),
            )
                .into_response(),
        },
        None => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(
                serde_json::json!({ "success": false, "error": "Redis not configured" }),
            ),
        )
            .into_response(),
    }
}

async fn success_json(data: impl serde::Serialize) -> Response {
    (
        StatusCode::OK,
        Json(serde_json::json!({ "success": true, "data": data })),
    )
        .into_response()
}

#[derive(Deserialize)]
pub struct ScheduleQuery {
    pub secret: Option<String>,
}

/// Cron endpoint — fetch today's anime schedule and push notifications
/// for episodes airing within the last hour.
pub async fn anime_schedule_handler(
    State(state): State<AppState>,
    Query(q): Query<ScheduleQuery>,
) -> Response {
    let expected = std::env::var("NOTIFICATION_CRON_SECRET")
        .unwrap_or_else(|_| "hue-cron-2026".to_string());
    if q.secret.as_deref().unwrap_or("") != expected {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({ "success": false, "error": "invalid secret" })),
        )
            .into_response();
    }

    let fcm = match &state.fcm {
        Some(c) => c,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({ "success": false, "error": "FCM not configured" })),
            )
                .into_response();
        }
    };

    let redis = match &state.redis_pool {
        Some(p) => p,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({ "success": false, "error": "Redis not configured" })),
            )
                .into_response();
        }
    };

    let schedule = state.anime_agg.schedule(None, None).await;
    if schedule.is_empty() {
        return success_json(serde_json::json!({ "sent": 0, "message": "no airings today" })).await;
    }

    let now = chrono::Utc::now().timestamp() as u64;
    let mut sent = 0u32;

    let tokens = match fcm::all_tokens(redis).await {
        Ok(t) => t,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(
                    serde_json::json!({ "success": false, "error": format!("Redis error: {}", e) }),
                ),
            )
                .into_response();
        }
    };

    if tokens.is_empty() {
        return success_json(serde_json::json!({ "sent": 0, "message": "no registered tokens" })).await;
    }

    for entry in &schedule {
        if entry.airs_at > 0 && now > entry.airs_at && now - entry.airs_at < 60 * 60 {
            let data = serde_json::json!({
                "type": "anime",
                "id": entry.slug.as_deref().unwrap_or(&entry.anime_id),
                "slug": entry.slug,
            });
            let title = format!("New episode: {}", entry.anime_title);
            let body = format!("Episode {} is now available!", entry.episode);
            if let Err(e) = fcm.send_batch(&tokens, &title, &body, &data).await {
                tracing::warn!("[notifications] FCM send failed for {}: {}", entry.anime_title, e);
            } else {
                sent += 1;
            }
        }
    }

    success_json(serde_json::json!({ "sent": sent, "total_airings": schedule.len() })).await
}
