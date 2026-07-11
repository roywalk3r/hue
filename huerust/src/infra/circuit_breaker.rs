//! Hue Stream API
//! Professional Streaming Aggregator
//!
//! @author roywalk3r <roywalker440@gmail.com>
//! @repository https://github.com/roywalk3r/hue.git
//! @license MIT

use dashmap::DashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{info, warn};

use crate::infra::metrics::Metrics;

const FAILURE_THRESHOLD: usize = 2;
const MIN_RECOVERY_TIMEOUT: Duration = Duration::from_secs(180); // 3 min
const MAX_RECOVERY_TIMEOUT: Duration = Duration::from_secs(900); // 15 min

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    Closed,   // Normal operation
    Open,     // Failing, skip requests
    HalfOpen, // Testing if recovered
}

#[derive(Debug)]
struct CircuitEntry {
    state: CircuitState,
    failures: AtomicUsize,
    consecutive_failures: AtomicUsize,
    last_failure: std::sync::Mutex<Option<Instant>>,
}

impl CircuitEntry {
    fn new() -> Self {
        Self {
            state: CircuitState::Closed,
            failures: AtomicUsize::new(0),
            consecutive_failures: AtomicUsize::new(0),
            last_failure: std::sync::Mutex::new(None),
        }
    }

    #[allow(dead_code)]
    fn record_success(&self) {
        self.consecutive_failures.store(0, Ordering::SeqCst);
    }

    #[allow(dead_code)]
    fn record_failure(&self) -> bool {
        self.failures.fetch_add(1, Ordering::SeqCst);
        let count = self.consecutive_failures.fetch_add(1, Ordering::SeqCst) + 1;
        *self.last_failure.lock().unwrap() = Some(Instant::now());

        if count >= FAILURE_THRESHOLD {
            return true; // Should open
        }
        false
    }

    fn get_recovery_timeout(&self) -> Duration {
        let count = self.consecutive_failures.load(Ordering::SeqCst);
        if count < FAILURE_THRESHOLD {
            return MIN_RECOVERY_TIMEOUT;
        }

        let trip_count = (count - FAILURE_THRESHOLD) as u32;
        let backoff = MIN_RECOVERY_TIMEOUT.as_secs() * 2u64.pow(trip_count.min(3));
        Duration::from_secs(backoff.min(MAX_RECOVERY_TIMEOUT.as_secs()))
    }

    #[allow(dead_code)]
    fn can_attempt(&self) -> bool {
        match self.state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                let last = *self.last_failure.lock().unwrap();
                match last {
                    Some(t) if t.elapsed() > self.get_recovery_timeout() => true,
                    _ => false,
                }
            }
            CircuitState::HalfOpen => true,
        }
    }
}

pub struct CircuitBreaker {
    states: DashMap<String, CircuitEntry>,
    metrics: std::sync::RwLock<Option<Arc<Metrics>>>,
}

impl CircuitBreaker {
    pub fn new() -> Self {
        Self {
            states: DashMap::new(),
            metrics: std::sync::RwLock::new(None),
        }
    }

    pub fn set_metrics(&self, metrics: Arc<Metrics>) {
        *self.metrics.write().unwrap() = Some(metrics);
    }

    /// Check if a provider is healthy enough to attempt a request
    pub fn is_healthy(&self, provider: &str) -> bool {
        let mut entry = self
            .states
            .entry(provider.to_string())
            .or_insert_with(CircuitEntry::new);

        match entry.state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                let last = *entry.last_failure.lock().unwrap();
                if let Some(t) = last {
                    if t.elapsed() > entry.get_recovery_timeout() {
                        info!("[CircuitBreaker] {} transitioning to HalfOpen", provider);
                        entry.state = CircuitState::HalfOpen;
                        true
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            CircuitState::HalfOpen => true,
        }
    }

    /// Record a successful request
    pub fn record_success(&self, provider: &str) {
        if let Some(entry) = self.states.get(provider) {
            entry.record_success();
            if entry.state == CircuitState::HalfOpen {
                drop(entry);
                if let Some(mut entry) = self.states.get_mut(provider) {
                    info!("[CircuitBreaker] {} recovered, closing circuit", provider);
                    entry.state = CircuitState::Closed;
                    entry.failures.store(0, Ordering::SeqCst);
                }
            }
        }
    }

    /// Record a failed request
    pub fn record_failure(&self, provider: &str) {
        let mut entry = self
            .states
            .entry(provider.to_string())
            .or_insert_with(CircuitEntry::new);
        let should_open = entry.record_failure();
        let count = entry.consecutive_failures.load(Ordering::SeqCst);

        if should_open && entry.state != CircuitState::Open {
            let timeout = entry.get_recovery_timeout();
            warn!(
                "[CircuitBreaker] {} opened after {} consecutive failures. Cooldown: {}min",
                provider,
                count,
                timeout.as_secs() / 60
            );
            entry.state = CircuitState::Open;
            if let Ok(guard) = self.metrics.read() {
                if let Some(ref m) = *guard {
                    m.record_circuit_trip(provider);
                }
            }
        }
    }

    #[allow(dead_code)]
    pub fn get_state(&self, provider: &str) -> CircuitState {
        self.states
            .get(provider)
            .map(|e| e.state)
            .unwrap_or(CircuitState::Closed)
    }
}

impl Default for CircuitBreaker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_circuit_closed_by_default() {
        let cb = CircuitBreaker::new();
        assert!(cb.is_healthy("test"));
    }

    #[tokio::test]
    async fn test_circuit_opens_after_failures() {
        let cb = CircuitBreaker::new();

        for _ in 0..FAILURE_THRESHOLD {
            assert!(cb.is_healthy("test"));
            cb.record_failure("test");
        }

        assert!(!cb.is_healthy("test"));
        assert_eq!(cb.get_state("test"), CircuitState::Open);
    }

    #[tokio::test]
    async fn test_circuit_recover() {
        let cb = CircuitBreaker::new();

        for _ in 0..FAILURE_THRESHOLD {
            cb.record_failure("test");
        }
        assert!(!cb.is_healthy("test"));

        // Simulate recovery time passing
        if let Some(entry) = cb.states.get_mut("test") {
            *entry.last_failure.lock().unwrap() =
                Some(Instant::now() - MIN_RECOVERY_TIMEOUT - Duration::from_secs(1));
        }

        assert!(cb.is_healthy("test"));
        assert_eq!(cb.get_state("test"), CircuitState::HalfOpen);

        cb.record_success("test");
        assert_eq!(cb.get_state("test"), CircuitState::Closed);
    }
}
