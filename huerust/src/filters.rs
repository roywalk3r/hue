//! Hue Stream API
//! Professional Streaming Aggregator
//!
//! @author roywalk3r <roywalker440@gmail.com>
//! @repository https://github.com/roywalk3r/hue.git
//! @license MIT

use crate::models::StreamResult;

#[derive(Clone, Debug)]
pub struct FilterConfig {
    pub min_quality: Option<u32>,
    pub exclude_codecs: Vec<String>,
    pub exclude_providers: Vec<String>,
}

impl Default for FilterConfig {
    fn default() -> Self {
        Self {
            min_quality: None,
            exclude_codecs: vec![],
            exclude_providers: vec![],
        }
    }
}

/// Extract numeric quality from a string like "1080p" or "720p"
fn extract_quality(text: &str) -> Option<u32> {
    use regex::Regex;
    let re = Regex::new(r"(\d{3,4})[pP]?").ok()?;
    re.captures(text)?.get(1)?.as_str().parse::<u32>().ok()
}

/// Apply filters to a list of streams, returning only those that pass
pub fn apply_filters(streams: Vec<StreamResult>, config: &FilterConfig) -> Vec<StreamResult> {
    let mut results = Vec::new();

    for stream in streams {
        // Check minimum quality
        if let Some(min) = config.min_quality {
            match extract_quality(&stream.quality) {
                Some(q) if q >= min => {}
                _ => {
                    continue;
                }
            }
        }

        // Check excluded codecs
        let title_lower = stream.title.to_lowercase();
        let mut excluded = false;
        for codec in &config.exclude_codecs {
            if title_lower.contains(&codec.to_lowercase()) {
                excluded = true;
                break;
            }
        }
        if excluded {
            continue;
        }

        // Check excluded providers
        let provider_lower = stream.provider.to_lowercase();
        for prov in &config.exclude_providers {
            if provider_lower == prov.to_lowercase() {
                excluded = true;
                break;
            }
        }
        if excluded {
            continue;
        }

        results.push(stream);
    }

    results
}

/// Sanitize provider names in text (e.g., "vixsrc" -> "Aura", "moviebox" -> "Cinema")
#[allow(dead_code)]
pub fn sanitize_provider_name(text: &str) -> String {
    let replacements = [
        ("vixsrc", "Aura"),
        ("moviebox", "Cinema"),
        ("sflix", "Nova"),
        ("kisskh", "Zenith"),
        ("dramacool", "Orient"),
        ("9anime", "Saber"),
        ("mp4hydra", "Hydra"),
        ("vidrock", "Luna"),
        ("vidzee", "VidZee"),
        ("videasy", "Vortex"),
        ("vidsrc-embed", "Nebula"),
    ];

    let mut result = text.to_string();
    for (from, to) in &replacements {
        result = result.replace(&from.to_lowercase(), to);
        result = result.replace(&from.to_uppercase(), to);
    }

    result
}

/// Sort streams by quality descending (highest quality first)
pub fn sort_by_quality(streams: &mut [StreamResult]) {
    streams.sort_by(|a, b| {
        let qa = extract_quality(&a.quality).unwrap_or(0);
        let qb = extract_quality(&b.quality).unwrap_or(0);
        qb.cmp(&qa) // descending
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stream(title: &str, quality: &str, provider: &str) -> StreamResult {
        StreamResult {
            url: "http://example.com".to_string(),
            title: title.to_string(),
            quality: quality.to_string(),
            provider: provider.to_string(),
            format: "hls".to_string(),
            subtitles: None,
            headers: None,
            extra_metadata: None,
        }
    }

    #[test]
    fn test_extract_quality() {
        assert_eq!(extract_quality("1080p"), Some(1080));
        assert_eq!(extract_quality("720P"), Some(720));
        assert_eq!(extract_quality("Auto"), None);
        assert_eq!(extract_quality(""), None);
    }

    #[test]
    fn test_apply_filters_min_quality() {
        let config = FilterConfig {
            min_quality: Some(720),
            ..Default::default()
        };
        let streams = vec![
            stream("A", "1080p", "Nova"),
            stream("B", "480p", "Nova"),
            stream("C", "Auto", "Nova"),
        ];

        let filtered = apply_filters(streams, &config);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].quality, "1080p");
    }

    #[test]
    fn test_apply_filters_exclude_provider() {
        let config = FilterConfig {
            exclude_providers: vec!["Nova".to_string()],
            ..Default::default()
        };
        let streams = vec![stream("A", "1080p", "Nova"), stream("B", "1080p", "Zenith")];

        let filtered = apply_filters(streams, &config);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].provider, "Zenith");
    }

    #[test]
    fn test_sanitize_provider_name() {
        assert_eq!(sanitize_provider_name("vixsrc stream"), "Aura stream");
        assert_eq!(sanitize_provider_name("moviebox HD"), "Cinema HD");
        assert_eq!(sanitize_provider_name("sflix 1080p"), "Nova 1080p");
    }

    #[test]
    fn test_sort_by_quality() {
        let mut streams = vec![
            stream("A", "480p", "P"),
            stream("B", "1080p", "P"),
            stream("C", "720p", "P"),
        ];
        sort_by_quality(&mut streams);
        assert_eq!(streams[0].quality, "1080p");
        assert_eq!(streams[1].quality, "720p");
        assert_eq!(streams[2].quality, "480p");
    }
}
