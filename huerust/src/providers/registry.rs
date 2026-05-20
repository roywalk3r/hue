//! Hue Stream API
//! Professional Streaming Aggregator
//!
//! @author roywalk3r <roywalker440@gmail.com>
//! @repository https://github.com/roywalk3r/hue.git
//! @license MIT

use crate::infra::circuit_breaker::CircuitBreaker;
use crate::providers::StreamProvider;
use std::sync::Arc;

pub struct ProviderRegistry {
    providers: Vec<Arc<dyn StreamProvider>>,
    circuit_breaker: Arc<CircuitBreaker>,
}

impl ProviderRegistry {
    pub fn new(circuit_breaker: Arc<CircuitBreaker>) -> Self {
        Self {
            providers: Vec::new(),
            circuit_breaker,
        }
    }

    pub fn register(&mut self, provider: Arc<dyn StreamProvider>) {
        self.providers.push(provider);
    }

    pub fn list_all_arc(&self) -> Vec<Arc<dyn StreamProvider>> {
        self.providers.clone()
    }

    pub fn list_all(&self) -> Vec<&dyn StreamProvider> {
        self.providers
            .iter()
            .filter(|p| self.circuit_breaker.is_healthy(p.real_name()))
            .map(|p| p.as_ref())
            .collect()
    }

    pub fn circuit_breaker(&self) -> &Arc<CircuitBreaker> {
        &self.circuit_breaker
    }
}
