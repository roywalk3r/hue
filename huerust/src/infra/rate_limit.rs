use dashmap::DashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Clone)]
pub struct RateLimitState {
    pub store: Arc<DashMap<String, (usize, Instant)>>,
    pub max_requests: usize,
    pub window_secs: u64,
}

impl RateLimitState {
    pub fn new(max_requests: usize, window_secs: u64) -> Self {
        Self {
            store: Arc::new(DashMap::new()),
            max_requests,
            window_secs,
        }
    }

    pub fn check(&self, key: &str) -> bool {
        let now = Instant::now();
        let window = Duration::from_secs(self.window_secs);

        if let Some(mut entry) = self.store.get_mut(key) {
            let (count, start) = *entry.value();
            if now.duration_since(start) > window {
                // New window
                *entry.value_mut() = (1, now);
                return true;
            }
            if count >= self.max_requests {
                return false;
            }
            *entry.value_mut() = (count + 1, start);
            true
        } else {
            self.store.insert(key.to_string(), (1, now));
            true
        }
    }
}
