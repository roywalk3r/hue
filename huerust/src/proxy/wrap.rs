use std::collections::HashMap;
use url::Url;
use crate::models::StreamResult;
use crate::security::encrypt_url;

pub(crate) fn wrap_with_proxy(streams: Vec<StreamResult>, proxy_base: &str, secret: &str) -> Vec<StreamResult> {
    streams.into_iter().map(|mut s| {
        // Skip if already proxied by this same server
        if s.url.contains(proxy_base) && (s.url.contains("/m3u8-proxy") || s.url.contains("/ts-proxy")) {
            return s;
        }

        let is_hls = s.format == "hls" || s.url.contains(".m3u8") || s.provider == "Aura";

        // Cinema (MovieBox) MP4 streams come from a CDN that IP-blocks server/datacenter
        // ranges. Proxying them through our server always 403s. Return the raw URL with
        // headers so the client (browser/player) fetches directly from its own IP.
        if s.provider == "Cinema" && !is_hls {
            return s;
        }

        // Determine host for special provider routing
        let host = match Url::parse(&s.url) {
            Ok(u) => u.host_str().unwrap_or("").to_lowercase(),
            Err(_) => String::new(),
        };
        let is_vixsrc = host.contains("vixsrc.to");
        let is_moviebox_cdn = host.contains("hakunaymatata.com") || host.contains("aisports.mobi") || host.contains("poocloud.in");
        let is_pixeldrain = host.contains("pixeldrain.");
        let is_googleusercontent = host == "video-downloads.googleusercontent.com";
        let is_mp4_mkv = s.url.to_lowercase().contains(".mp4") || s.url.to_lowercase().contains(".mkv");

        // Force url mode for Vixsrc (signed URLs need query param inheritance)
        let use_url_mode = is_vixsrc
            || s.extra_metadata.as_ref()
                .map(|m| m.get("proxyMode").map(|v| v == "url").unwrap_or(false))
                .unwrap_or(false);

        // Extract headers from stream or from existing proxied URL
        let mut headers = s.headers.clone().unwrap_or_default();
        if headers.is_empty() && s.url.contains("headers=") {
            if let Ok(u) = Url::parse(&format!("http://localhost/{}", &s.url)) {
                if let Some(h_json) = u.query_pairs().find(|(k, _)| k == "headers").map(|(_, v)| v.to_string()) {
                    if let Ok(parsed) = serde_json::from_str::<HashMap<String, String>>(&h_json) {
                        headers = parsed;
                    }
                }
            }
        }

        let token = encrypt_url(&s.url, secret, if headers.is_empty() { None } else { Some(headers.clone()) });
        let h_param = if headers.is_empty() {
            String::new()
        } else {
            match serde_json::to_string(&headers) {
                Ok(h_json) => format!("&headers={}", urlencoding::encode(&h_json)),
                Err(_) => String::new(),
            }
        };

        // ── Determine proxy route and URL ──
        let proxied_url = if is_hls {
            if use_url_mode {
                format!("{}/m3u8-proxy?url={}{}&mode=url", proxy_base, urlencoding::encode(&s.url), h_param)
            } else {
                format!("{}/m3u8-proxy?h_token={}", proxy_base, token)
            }
        } else if is_pixeldrain || is_googleusercontent || is_moviebox_cdn {
            let synth_params = if is_moviebox_cdn { "&noSynth=1" } else { "" };
            format!("{}/ts-proxy?h_token={}{}", proxy_base, token, synth_params)
        } else if is_mp4_mkv {
            format!("{}/ts-proxy?h_token={}", proxy_base, token)
        } else if use_url_mode {
            // Force ts-proxy for mp4/video format, else m3u8-proxy
            let force_ts = s.format == "mp4" || s.format == "video" || s.url.to_lowercase().contains(".mp4") || s.url.to_lowercase().contains(".mkv");
            let route = if force_ts { "ts-proxy" } else { "m3u8-proxy" };
            format!("{}/{}?url={}{}{}", proxy_base, route, urlencoding::encode(&s.url), h_param,
                if route == "m3u8-proxy" { "&mode=url" } else { "" })
        } else if is_vixsrc {
            format!("{}/m3u8-proxy?url={}{}", proxy_base, urlencoding::encode(&s.url), h_param)
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

        // Pre-proxy subtitles
        if let Some(subs) = s.subtitles.take() {
            s.subtitles = Some(subs.into_iter().map(|mut sub| {
                let sub_token = encrypt_url(&sub.url, secret, None);
                sub.url = format!("{}/sub-proxy?h_token={}", proxy_base, sub_token);
                sub
            }).collect());
        }
        s
    }).collect()
}
