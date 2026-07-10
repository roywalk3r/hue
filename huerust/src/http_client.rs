use reqwest::{Client, ClientBuilder, Proxy};
use reqwest_middleware::{ClientBuilder as MiddlewareClientBuilder, ClientWithMiddleware};
use reqwest_retry::{policies::ExponentialBackoff, RetryTransientMiddleware};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tracing::warn;

/// Hosts that Cloudflare-challenge datacenter/VPS IPs and therefore must be fetched
/// through a clean-IP proxy pool. kisskh's manifest+subtitle CDN (`*.cdnvideo11.shop`)
/// and its API (`kisskh.ovh` / `kisskh.dk`) 403 the production VPS; vixsrc likewise.
///
/// The heavy video lives on Cloudflare R2 (`*.r2.cloudflarestorage.com`) with signed
/// URLs that serve the VPS DIRECTLY — it is deliberately NOT matched here, so only the
/// tiny manifest/API bytes ever hit the paid pool.
fn is_cf_blocked_host(host: &str) -> bool {
    host.ends_with("cdnvideo11.shop")
        || host == "kisskh.ovh"
        || host == "kisskh.dk"
        || host == "vixsrc.to"
        || host.ends_with("vix-content.net")
}

/// Route ONLY the Cloudflare-blocked hosts (see [`is_cf_blocked_host`]) through the
/// rotating proxy pool in `CF_PROXY_URLS` — a comma-separated list of
/// `http://user:pass@ip:port` proxies. Every other host — R2 video segments and all
/// other providers — stays direct. No-op when `CF_PROXY_URLS` is unset, so this is
/// inert until a pool is configured (i.e. on the VPS).
pub fn with_cf_proxy_pool(builder: ClientBuilder) -> ClientBuilder {
    let pool: Vec<String> = std::env::var("CF_PROXY_URLS")
        .ok()
        .map(|s| {
            s.split(',')
                .map(|p| p.trim().to_string())
                .filter(|p| !p.is_empty())
                .collect()
        })
        .unwrap_or_default();
    if pool.is_empty() {
        return builder;
    }
    let n = pool.len();
    let counter = Arc::new(AtomicUsize::new(0));
    let proxy = Proxy::custom(move |url| {
        if is_cf_blocked_host(url.host_str().unwrap_or("")) {
            let i = counter.fetch_add(1, Ordering::Relaxed) % n;
            reqwest::Url::parse(&pool[i]).ok()
        } else {
            None
        }
    });
    builder.proxy(proxy)
}

/// One retry, with a tight backoff. The default (3 retries, exponential
/// backoff to ~3 s) stacks ~5 s of sleeps *inside* a caller's timeout — which
/// made reachability probes drop slow-but-alive streams as "timed out" and
/// dragged every provider round. A streaming aggregator with 4-8 s deadlines
/// wants fast-fail: a single quick retry for a transient blip, no more.
fn retry_policy() -> ExponentialBackoff {
    ExponentialBackoff::builder()
        .retry_bounds(Duration::from_millis(150), Duration::from_millis(600))
        .build_with_max_retries(1)
}

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
    MiddlewareClientBuilder::new(client)
        .with(RetryTransientMiddleware::new_with_policy(retry_policy()))
        .build()
}

/// Wrap an existing [`Client`] with retry middleware.
pub fn wrap_with_retry(client: Client) -> ClientWithMiddleware {
    MiddlewareClientBuilder::new(client)
        .with(RetryTransientMiddleware::new_with_policy(retry_policy()))
        .build()
}
