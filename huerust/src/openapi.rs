//! Hue Stream API
//! Professional Streaming Aggregator
//!
//! @author roywalk3r <roywalker440@gmail.com>
//! @repository https://github.com/roywalk3r/hue.git
//! @license MIT

use crate::models::{StreamResult, Subtitle};
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Huerust Engine API",
        version = "0.1.0",
        description = "High-performance Rust streaming aggregator engine. Exposes stream aggregation, HiTV discovery, drama search, and proxy endpoints."
    ),
    paths(
        crate::routes::streams::streams_handler,
        crate::routes::streams::luna_streams_handler,
        crate::routes::sse::sse_streams_handler,
        crate::routes::providers::providers_handler,
        crate::routes::play::play_handler,
        crate::routes::sports::detail_handler,
        crate::routes::system::auth_token_handler,
    ),
    components(schemas(
        StreamResult,
        Subtitle,
        crate::clients::aisports::SportsStream,
        crate::routes::sports::SportsDetailResponse,
        crate::routes::system::AuthTokenResponse
    ))
)]
pub struct ApiDoc;
