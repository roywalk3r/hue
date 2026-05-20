use crate::models::StreamResult;
use crate::security::encrypt_url;
use url::Url;

use super::is_whitelisted_raw_url;

fn looks_like_hls_playlist_url(target_url: &str) -> bool {
    let lower = target_url.to_lowercase();
    if lower.contains(".m3u8") {
        return true;
    }

    if let Ok(url) = Url::parse(target_url) {
        let host = url.host_str().unwrap_or("").to_lowercase();
        let path = url.path().to_lowercase();
        if host.contains("vixsrc.to") && path.starts_with("/playlist/") {
            return true;
        }
    }

    false
}

pub(crate) fn wrap_with_proxy(
    streams: Vec<StreamResult>,
    proxy_base: &str,
    secret: &str,
) -> Vec<StreamResult> {
    streams
        .into_iter()
        .map(|mut s| {
            // Skip if already proxied by this same server
            if s.url.contains(proxy_base)
                && (s.url.contains("/m3u8-proxy") || s.url.contains("/ts-proxy"))
            {
                return s;
            }

            // Browser-native embeds must remain direct iframe URLs. Proxying them as HLS/TS
            // turns a playable embed document into an invalid media manifest.
            if s.format == "iframe" {
                return s;
            }

            let is_hls = s.format == "hls" || looks_like_hls_playlist_url(&s.url);

            // Determine host for special provider routing
            let host = match Url::parse(&s.url) {
                Ok(u) => u.host_str().unwrap_or("").to_lowercase(),
                Err(_) => String::new(),
            };
            let is_moviebox_cdn = host.contains("hakunaymatata.com")
                || host.contains("aisports.mobi")
                || host.contains("poocloud.in");
            let is_pixeldrain = host.contains("pixeldrain.");
            let is_googleusercontent = host == "video-downloads.googleusercontent.com";
            let is_mp4_mkv =
                s.url.to_lowercase().contains(".mp4") || s.url.to_lowercase().contains(".mkv");

            let headers = s.headers.clone().unwrap_or_default();
            let use_raw_url = is_whitelisted_raw_url(&s.url);
            let token_headers = if headers.is_empty() {
                None
            } else {
                Some(headers.clone())
            };

            // ── Build proxy URL ──
            let proxied_url = if use_raw_url {
                let encoded_url = urlencoding::encode(&s.url);
                let header_part = if let Some(ref h) = token_headers {
                    format!(
                        "&headers={}",
                        urlencoding::encode(&serde_json::to_string(h).unwrap_or_default())
                    )
                } else {
                    String::new()
                };
                let route = if is_hls { "m3u8-proxy" } else { "ts-proxy" };
                format!(
                    "{}/{}?url={}{}&mode=url",
                    proxy_base, route, encoded_url, header_part
                )
            } else {
                let token = encrypt_url(&s.url, secret, token_headers.clone());
                if is_hls {
                    format!("{}/m3u8-proxy?h_token={}", proxy_base, token)
                } else if is_pixeldrain || is_googleusercontent || is_moviebox_cdn {
                    format!("{}/ts-proxy?h_token={}", proxy_base, token)
                } else if is_mp4_mkv {
                    format!("{}/ts-proxy?h_token={}", proxy_base, token)
                } else {
                    // Default: hls → m3u8-proxy, everything else → ts-proxy
                    if is_hls {
                        format!("{}/m3u8-proxy?h_token={}", proxy_base, token)
                    } else {
                        format!("{}/ts-proxy?h_token={}", proxy_base, token)
                    }
                }
            };

            s.url = proxied_url;
            s.headers = None;

            // Pre-proxy subtitles — same raw/token logic
            if let Some(subs) = s.subtitles.take() {
                s.subtitles = Some(
                    subs.into_iter()
                        .map(|mut sub| {
                            let sub_url = if use_raw_url {
                                let encoded_sub_url = urlencoding::encode(&sub.url);
                                let sub_header_part = if let Some(ref h) = token_headers {
                                    format!(
                                        "&headers={}",
                                        urlencoding::encode(
                                            &serde_json::to_string(h).unwrap_or_default()
                                        )
                                    )
                                } else {
                                    String::new()
                                };
                                format!(
                                    "{}/sub-proxy?url={}{}&mode=url",
                                    proxy_base, encoded_sub_url, sub_header_part
                                )
                            } else {
                                let sub_token = encrypt_url(&sub.url, secret, token_headers.clone());
                                format!("{}/sub-proxy?h_token={}", proxy_base, sub_token)
                            };
                            sub.url = sub_url;
                            sub
                        })
                        .collect(),
                );
            }
            s
        })
        .collect()
}
