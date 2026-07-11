//! Hue Stream API
//! Professional Streaming Aggregator
//!
//! @author roywalk3r <roywalker440@gmail.com>
//! @repository https://github.com/roywalk3r/hue.git
//! @license MIT

use axum::{
    Json,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;
use utoipa::ToSchema;

use crate::app::ProvidersState;

#[derive(Serialize, ToSchema)]
pub struct ProviderInfo {
    name: &'static str,
    real_name: &'static str,
    is_fast: bool,
    timeout_secs: u64,
    circuit: &'static str,
}

#[utoipa::path(
    get,
    path = "/api/providers",
    responses(
        (status = 200, description = "List of registered providers with circuit state"),
    ),
)]
pub async fn providers_handler(State(state): State<ProvidersState>) -> Response {
    let cb = state.registry.circuit_breaker();
    let providers: Vec<ProviderInfo> = state
        .registry
        .list_all()
        .iter()
        .map(|p| {
            let state_str = match cb.get_state(p.real_name()) {
                crate::infra::circuit_breaker::CircuitState::Closed => "closed",
                crate::infra::circuit_breaker::CircuitState::Open => "open",
                crate::infra::circuit_breaker::CircuitState::HalfOpen => "half_open",
            };
            ProviderInfo {
                name: p.name(),
                real_name: p.real_name(),
                is_fast: p.is_fast(),
                timeout_secs: p.timeout_secs(),
                circuit: state_str,
            }
        })
        .collect();

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "count": providers.len(),
            "providers": providers
        })),
    )
        .into_response()
}
