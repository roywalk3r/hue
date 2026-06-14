use metrics_exporter_prometheus::PrometheusBuilder;
use std::sync::Arc;

#[derive(Clone)]
pub struct Metrics {
    handle: Arc<metrics_exporter_prometheus::PrometheusHandle>,
}

impl Metrics {
    pub fn new(_names: &[&str]) -> Self {
        let recorder = PrometheusBuilder::new().build_recorder();
        let handle = Arc::new(recorder.handle());
        if let Err(e) = metrics::set_global_recorder(recorder) {
            tracing::warn!("Metrics recorder already installed: {}", e);
        }
        Self { handle }
    }

    pub fn record_request(&self, provider: &str, latency_ms: u64, success: bool, timed_out: bool) {
        let p = provider.to_string();
        metrics::counter!("huerust_provider_requests_total", "provider" => p.clone()).increment(1);
        metrics::counter!("huerust_provider_latency_ms_total", "provider" => p.clone())
            .increment(latency_ms);
        if timed_out {
            metrics::counter!("huerust_provider_timeouts_total", "provider" => p.clone())
                .increment(1);
            metrics::counter!("huerust_provider_failures_total", "provider" => p.clone())
                .increment(1);
        } else if success {
            metrics::counter!("huerust_provider_successes_total", "provider" => p.clone())
                .increment(1);
        } else {
            metrics::counter!("huerust_provider_failures_total", "provider" => p.clone())
                .increment(1);
        }
    }

    pub fn record_circuit_trip(&self, provider: &str) {
        metrics::counter!("huerust_provider_circuit_trips_total", "provider" => provider.to_string()).increment(1);
    }

    pub fn render_prometheus(&self) -> String {
        self.handle.render()
    }
}
