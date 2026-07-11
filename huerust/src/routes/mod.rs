//! Hue Stream API
//! Professional Streaming Aggregator
//!
//! @author roywalk3r <roywalker440@gmail.com>
//! @repository https://github.com/roywalk3r/hue.git
//! @license MIT

pub mod anime;
pub mod donghua;
pub mod download;
pub mod drama;
pub mod hitv;
pub mod moviebox;
pub mod play;
pub mod providers;
pub mod sports;
pub mod sse;
pub mod streams;
// re-export report-failure types so app.rs can wire the route without
// knowing the request body shape.
pub mod subtitle;
pub mod system;
pub mod tmdb;
