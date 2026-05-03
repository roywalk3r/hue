//! Hue Stream API
//! Professional Streaming Aggregator
//!
//! @author roywalk3r <roywalker440@gmail.com>
//! @repository https://github.com/roywalk3r/hue.git
//! @license MIT

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

/// Thread-safe per-provider metrics using atomics.
pub struct Metrics {
    providers: HashMap<String, ProviderMetrics>,
}

#[derive(Default)]
pub struct ProviderMetrics {
    pub total_requests: AtomicU64,
    pub successes: AtomicU64,
    pub failures: AtomicU64,
    pub timeouts: AtomicU64,
    pub total_latency_ms: AtomicU64,
    pub circuit_trips: AtomicU64,
}

impl Metrics {
    pub fn new(provider_names: &[&str]) -> Self {
        let mut providers = HashMap::new();
        for name in provider_names {
            providers.insert(name.to_string(), ProviderMetrics::default());
        }
        Self { providers }
    }

    pub fn record_request(&self, provider: &str, _latency_ms: u64, success: bool, timed_out: bool) {
        if let Some(m) = self.providers.get(provider) {
            m.total_requests.fetch_add(1, Ordering::Relaxed);
            if timed_out {
                m.timeouts.fetch_add(1, Ordering::Relaxed);
                m.failures.fetch_add(1, Ordering::Relaxed);
            } else if success {
                m.successes.fetch_add(1, Ordering::Relaxed);
            } else {
                m.failures.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    pub fn record_latency(&self, provider: &str, latency_ms: u64) {
        if let Some(m) = self.providers.get(provider) {
            m.total_latency_ms.fetch_add(latency_ms, Ordering::Relaxed);
        }
    }

    pub fn record_circuit_trip(&self, provider: &str) {
        if let Some(m) = self.providers.get(provider) {
            m.circuit_trips.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub fn render_prometheus(&self) -> String {
        let mut output = String::new();
        let providers: Vec<String> = self.providers.keys().cloned().collect();

        // Counter: total requests
        output.push_str("# TYPE huerust_provider_requests_total counter\n");
        output.push_str("# HELP huerust_provider_requests_total Total provider fetch requests\n");
        for name in &providers {
            let m = self.providers.get(name).unwrap();
            let val = m.total_requests.load(Ordering::Relaxed);
            output.push_str(&format!("huerust_provider_requests_total{{provider=\"{}\"}} {}\n", name, val));
        }

        // Counter: successes
        output.push_str("# TYPE huerust_provider_successes_total counter\n");
        output.push_str("# HELP huerust_provider_successes_total Successful provider fetches\n");
        for name in &providers {
            let m = self.providers.get(name).unwrap();
            let val = m.successes.load(Ordering::Relaxed);
            output.push_str(&format!("huerust_provider_successes_total{{provider=\"{}\"}} {}\n", name, val));
        }

        // Counter: failures
        output.push_str("# TYPE huerust_provider_failures_total counter\n");
        output.push_str("# HELP huerust_provider_failures_total Failed provider fetches\n");
        for name in &providers {
            let m = self.providers.get(name).unwrap();
            let val = m.failures.load(Ordering::Relaxed);
            output.push_str(&format!("huerust_provider_failures_total{{provider=\"{}\"}} {}\n", name, val));
        }

        // Counter: timeouts
        output.push_str("# TYPE huerust_provider_timeouts_total counter\n");
        output.push_str("# HELP huerust_provider_timeouts_total Timed-out provider fetches\n");
        for name in &providers {
            let m = self.providers.get(name).unwrap();
            let val = m.timeouts.load(Ordering::Relaxed);
            output.push_str(&format!("huerust_provider_timeouts_total{{provider=\"{}\"}} {}\n", name, val));
        }

        // Counter: total latency
        output.push_str("# TYPE huerust_provider_latency_ms_total counter\n");
        output.push_str("# HELP huerust_provider_latency_ms_total Sum of provider fetch latencies\n");
        for name in &providers {
            let m = self.providers.get(name).unwrap();
            let val = m.total_latency_ms.load(Ordering::Relaxed);
            output.push_str(&format!("huerust_provider_latency_ms_total{{provider=\"{}\"}} {}\n", name, val));
        }

        // Counter: circuit trips
        output.push_str("# TYPE huerust_provider_circuit_trips_total counter\n");
        output.push_str("# HELP huerust_provider_circuit_trips_total Circuit breaker trips to Open\n");
        for name in &providers {
            let m = self.providers.get(name).unwrap();
            let val = m.circuit_trips.load(Ordering::Relaxed);
            output.push_str(&format!("huerust_provider_circuit_trips_total{{provider=\"{}\"}} {}\n", name, val));
        }

        output
    }
}
