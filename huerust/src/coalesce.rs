//! Hue Stream API
//! Professional Streaming Aggregator
//!
//! @author roywalk3r <roywalker440@gmail.com>
//! @repository https://github.com/roywalk3r/hue.git
//! @license MIT

use std::sync::Arc;
use dashmap::DashMap;
use tokio::sync::Notify;
use crate::cache::CacheKey;
use crate::models::StreamResult;
use crate::cache::StreamCache;

/// Coalesces concurrent fetch requests for the same content.
/// First request performs the fetch; subsequent requests wait and read from cache.
pub struct FetchCoalescer {
    in_flight: Arc<DashMap<CacheKey, Arc<Notify>>>,
}

impl FetchCoalescer {
    pub fn new() -> Self {
        Self {
            in_flight: Arc::new(DashMap::new()),
        }
    }

    /// Execute `fetch` if no other request is in-flight for this key.
    /// Otherwise wait for the in-flight request to complete and return its cached result.
    pub async fn execute<F, Fut>(
        &self,
        key: &CacheKey,
        cache: &StreamCache,
        fetch: F,
    ) -> Option<Vec<StreamResult>>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Vec<StreamResult>>,
    {
        // Fast path: already cached
        if let Some(cached) = cache.get(key).await {
            return Some(cached);
        }

        // If another request is already fetching, wait for it
        let existing_notify = self.in_flight.get(key).map(|entry| entry.clone());
        if let Some(notify) = existing_notify {
            notify.notified().await;
            return cache.get(key).await;
        }

        // We are the first request — start fetching
        let notify = Arc::new(Notify::new());
        self.in_flight.insert(key.clone(), notify.clone());

        let result = fetch().await;

        // Store in cache
        if !result.is_empty() {
            cache.set(key, result.clone()).await;
        }

        // Notify waiters and remove in-flight marker
        self.in_flight.remove(key);
        notify.notify_waiters();

        if result.is_empty() {
            None
        } else {
            Some(result)
        }
    }
}

impl Default for FetchCoalescer {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for FetchCoalescer {
    fn clone(&self) -> Self {
        Self {
            in_flight: self.in_flight.clone(),
        }
    }
}
