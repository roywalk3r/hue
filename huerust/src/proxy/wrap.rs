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
                || host.contains("poocloud.in")
                || host.contains("storrrrrrm.site");
            let is_pixeldrain = host.contains("pixeldrain.");
            let is_googleusercontent = host == "video-downloads.googleusercontent.com";
            let is_mp4_mkv =
                s.url.to_lowercase().contains(".mp4") || s.url.to_lowercase().contains(".mkv");

            let mut headers = s.headers.clone().unwrap_or_default();
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

            // Pre-proxy subtitles — evaluate each URL independently
            if let Some(subs) = s.subtitles.take() {
                s.subtitles = Some(
                    subs.into_iter()
                        .map(|mut sub| {
                            let sub_use_raw = is_whitelisted_raw_url(&sub.url);
                            let sub_url = if sub_use_raw && token_headers.is_none() {
                                let encoded_sub_url = urlencoding::encode(&sub.url);
                                format!("{}/sub-proxy?url={}&mode=url", proxy_base, encoded_sub_url)
                            } else {
                                let sub_token =
                                    encrypt_url(&sub.url, secret, token_headers.clone());
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Subtitle;
    use std::collections::HashMap;

    fn stream_with_subtitles(headers: Option<HashMap<String, String>>) -> StreamResult {
        StreamResult {
            url: "https://cdn.example.com/video.mp4".to_string(),
            title: "Test".to_string(),
            quality: "720p".to_string(),
            provider: "MovieBox".to_string(),
            format: "mp4".to_string(),
            subtitles: Some(vec![Subtitle {
                url: "https://cacdn.hakunaymatata.com/subtitle/test.srt".to_string(),
                lang: "en".to_string(),
                label: "English".to_string(),
            }]),
            headers,
            extra_metadata: None,
        }
    }

    #[test]
    fn wraps_header_required_subtitles_with_signed_token() {
        let mut headers = HashMap::new();
        headers.insert("Cookie".to_string(), "mb_token=secret".to_string());

        let wrapped = wrap_with_proxy(
            vec![stream_with_subtitles(Some(headers))],
            "https://api.vexli.online",
            "test-secret",
        );

        let subtitle_url = &wrapped[0].subtitles.as_ref().unwrap()[0].url;
        assert!(subtitle_url.starts_with("https://api.vexli.online/sub-proxy?h_token="));
        assert!(!subtitle_url.contains("mb_token"));
        assert!(!subtitle_url.contains("&headers="));
    }

    #[test]
    fn leaves_public_raw_subtitles_as_plain_proxy_urls_without_headers() {
        let wrapped = wrap_with_proxy(
            vec![stream_with_subtitles(None)],
            "https://api.vexli.online",
            "test-secret",
        );

        let subtitle_url = &wrapped[0].subtitles.as_ref().unwrap()[0].url;
        assert!(subtitle_url.starts_with("https://api.vexli.online/sub-proxy?url="));
        assert!(subtitle_url.ends_with("&mode=url"));
    }
}
