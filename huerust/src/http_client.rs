use reqwest::{ClientBuilder, Proxy};
use tracing::warn;

pub fn with_optional_proxy(builder: ClientBuilder, specific_env: &str) -> ClientBuilder {
    let proxy_url = std::env::var(specific_env)
        .ok()
        .filter(|v| !v.trim().is_empty())
        .or_else(|| std::env::var("OUTBOUND_PROXY_URL").ok().filter(|v| !v.trim().is_empty()));

    let Some(proxy_url) = proxy_url else {
        return builder;
    };

    match Proxy::all(&proxy_url) {
        Ok(proxy) => builder.proxy(proxy),
        Err(e) => {
            warn!("[http] Ignoring invalid proxy URL from {} / OUTBOUND_PROXY_URL: {}", specific_env, e);
            builder
        }
    }
}
