use crate::models::StreamResult;
use crate::security::encrypt_url;
use url::Url;

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

            // Extract headers from stream. Public raw url+headers proxy mode is disabled;
            // headers must travel inside h_token.
            let headers = s.headers.clone().unwrap_or_default();

            let token = encrypt_url(
                &s.url,
                secret,
                if headers.is_empty() {
                    None
                } else {
                    Some(headers.clone())
                },
            );

            // ── Determine proxy route and URL ──
            let proxied_url = if is_hls {
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
            };

            s.url = proxied_url;
            s.headers = None; // headers are now baked into the token or url query

            // Pre-proxy subtitles — inherit stream headers so the proxy can inject
            // provider-specific Referer / Origin / User-Agent for the subtitle CDN.
            let sub_headers = if headers.is_empty() {
                None
            } else {
                Some(headers.clone())
            };
            if let Some(subs) = s.subtitles.take() {
                s.subtitles = Some(
                    subs.into_iter()
                        .map(|mut sub| {
                            let sub_token = encrypt_url(&sub.url, secret, sub_headers.clone());
                            sub.url = format!("{}/sub-proxy?h_token={}", proxy_base, sub_token);
                            sub
                        })
                        .collect(),
                );
            }
            s
        })
        .collect()
}
