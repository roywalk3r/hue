//! Hue Stream API
//! Professional Streaming Aggregator
//!
//! @author roywalk3r <roywalker440@gmail.com>
//! @repository https://github.com/roywalk3r/hue.git
//! @license MIT

use utoipa::OpenApi;
use crate::models::{StreamResult, Subtitle};

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Huerust Engine API",
        version = "0.1.0",
        description = "High-performance Rust streaming aggregator engine. Exposes stream aggregation, HiTV discovery, drama search, anime streaming, and proxy endpoints."
    ),
    paths(
        crate::routes::streams::streams_handler,
        crate::routes::sse::sse_streams_handler,
        crate::routes::providers::providers_handler,
        crate::routes::play::play_handler,
        crate::routes::anime::search_handler,
        crate::routes::anime::latest_handler,
        crate::routes::anime::category_handler,
        crate::routes::anime::home_handler,
        crate::routes::anime::servers_handler,
        crate::routes::anime::stream_handler,
    ),
    components(schemas(StreamResult, Subtitle))
)]
pub struct ApiDoc;
