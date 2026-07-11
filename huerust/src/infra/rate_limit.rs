use governor::{clock::DefaultClock, state::keyed::DashMapStateStore, Quota, RateLimiter};
use std::num::NonZeroU32;
use std::sync::Arc;

#[derive(Clone)]
pub struct RateLimitState {
    limiter: Arc<RateLimiter<String, DashMapStateStore<String>, DefaultClock>>,
}

impl RateLimitState {
    pub fn new(max_requests: u32, _window_secs: u64) -> Self {
        let quota = Quota::per_minute(NonZeroU32::new(max_requests).unwrap());
        Self {
            limiter: Arc::new(RateLimiter::keyed(quota)),
        }
    }

    pub fn check(&self, key: &str) -> bool {
        self.limiter.check_key(&key.to_string()).is_ok()
    }
}
