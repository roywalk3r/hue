use reqwest::{Client, ClientBuilder, Proxy};
use reqwest_middleware::{ClientBuilder as MiddlewareClientBuilder, ClientWithMiddleware};
use reqwest_retry::{policies::ExponentialBackoff, RetryTransientMiddleware};
use tracing::warn;

pub fn with_optional_proxy(builder: ClientBuilder, specific_env: &str) -> ClientBuilder {
    let proxy_url = std::env::var(specific_env)
        .ok()
        .filter(|v| !v.trim().is_empty())
        .or_else(|| {
            std::env::var("OUTBOUND_PROXY_URL")
                .ok()
                .filter(|v| !v.trim().is_empty())
        });

    let Some(proxy_url) = proxy_url else {
        return builder;
    };

    match Proxy::all(&proxy_url) {
        Ok(proxy) => builder.proxy(proxy),
        Err(e) => {
            warn!(
                "[http] Ignoring invalid proxy URL from {} / OUTBOUND_PROXY_URL: {}",
                specific_env, e
            );
            builder
        }
    }
}

#[allow(dead_code)]
pub fn build_client_with_retry(builder: ClientBuilder, specific_env: &str) -> ClientWithMiddleware {
    let client = with_optional_proxy(builder, specific_env)
        .build()
        .expect("Failed to build reqwest Client");
    let retry_policy = ExponentialBackoff::builder().build_with_max_retries(3);
    MiddlewareClientBuilder::new(client)
        .with(RetryTransientMiddleware::new_with_policy(retry_policy))
        .build()
}

/// Wrap an existing [`Client`] with retry middleware.
pub fn wrap_with_retry(client: Client) -> ClientWithMiddleware {
    let retry_policy = ExponentialBackoff::builder().build_with_max_retries(3);
    MiddlewareClientBuilder::new(client)
        .with(RetryTransientMiddleware::new_with_policy(retry_policy))
        .build()
}
