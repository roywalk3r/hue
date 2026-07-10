use reqwest::{Client, ClientBuilder, Proxy};
use reqwest_middleware::{ClientBuilder as MiddlewareClientBuilder, ClientWithMiddleware};
use reqwest_retry::{policies::ExponentialBackoff, RetryTransientMiddleware};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
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

/// Path to the proxy-pool file, if configured. One proxy per line.
fn cf_proxy_file() -> Option<String> {
    std::env::var("CF_PROXY_FILE")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Parse one proxy entry. Accepts, in order of preference:
///   - a full URL:            `http://user:pass@ip:port` / `socks5://ip:port` (used as-is)
///   - webshare export:       `ip:port:user:pass` → `http://user:pass@ip:port`
///   - bare host:port:        `ip:port` → `http://ip:port`
/// Blank lines and `#` comments return `None`.
fn parse_proxy_line(raw: &str) -> Option<String> {
    let line = raw.trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    if line.contains("://") {
        return Some(line.to_string());
    }
    let parts: Vec<&str> = line.split(':').collect();
    match parts.as_slice() {
        [ip, port, user, pass] => Some(format!("http://{user}:{pass}@{ip}:{port}")),
        [ip, port] => Some(format!("http://{ip}:{port}")),
        _ => None,
    }
}

/// Load the proxy pool. `CF_PROXY_FILE` (one proxy per line) takes precedence;
/// otherwise `CF_PROXY_URLS` (comma-separated). Both accept the webshare
/// `ip:port:user:pass` format or full proxy URLs (see [`parse_proxy_line`]).
pub fn load_cf_proxies() -> Vec<String> {
    if let Some(path) = cf_proxy_file() {
        match std::fs::read_to_string(&path) {
            Ok(contents) => return contents.lines().filter_map(parse_proxy_line).collect(),
            Err(e) => warn!("[http] CF_PROXY_FILE {} unreadable: {}", path, e),
        }
    }
    std::env::var("CF_PROXY_URLS")
        .ok()
        .map(|s| s.split(',').filter_map(parse_proxy_line).collect())
        .unwrap_or_default()
}

/// Route ONLY the Cloudflare-blocked hosts (see [`is_cf_blocked_host`]) through the
/// rotating proxy pool. Every other host — R2 video segments and all other providers —
/// stays direct. The pool comes from `CF_PROXY_FILE` (one per line, hot-reloaded every
/// ~30s so edits take effect without a restart) or `CF_PROXY_URLS`. Inert until a pool
/// is configured, so this is a no-op off the VPS.
pub fn with_cf_proxy_pool(builder: ClientBuilder) -> ClientBuilder {
    let file_backed = cf_proxy_file().is_some();
    let initial = load_cf_proxies();
    // Fully inert when nothing is configured (no file, no env) — preserves default behaviour.
    if initial.is_empty() && !file_backed {
        return builder;
    }
    let counter = Arc::new(AtomicUsize::new(0));
    // (pool, last_reload). Only re-read from disk when file-backed, at most every 30s.
    let state = Arc::new(Mutex::new((initial, Instant::now())));
    let proxy = Proxy::custom(move |url| {
        if !is_cf_blocked_host(url.host_str().unwrap_or("")) {
            return None;
        }
        let mut guard = state.lock().ok()?;
        if file_backed && guard.1.elapsed() >= Duration::from_secs(30) {
            let fresh = load_cf_proxies();
            if !fresh.is_empty() && fresh != guard.0 {
                warn!("[http] CF proxy pool reloaded: {} proxies", fresh.len());
                guard.0 = fresh;
            }
            guard.1 = Instant::now();
        }
        if guard.0.is_empty() {
            return None;
        }
        let i = counter.fetch_add(1, Ordering::Relaxed) % guard.0.len();
        reqwest::Url::parse(&guard.0[i]).ok()
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
