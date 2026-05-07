//! Hue Stream API
//! Professional Streaming Aggregator
//!
//! @author roywalk3r <roywalker440@gmail.com>
//! @repository https://github.com/roywalk3r/hue.git
//! @license MIT

//! HLS Manifest Ad-Surgery
//!
//! Detects and removes ad segments from `.m3u8` playlists.
//! Heuristics:
//! - Segment duration < 2.0s (suspiciously short)
//! - Known ad domains in segment URLs
//! - Segments bracketed by `#EXT-X-DISCONTINUITY` tags

const SUSPICIOUS_DURATION_THRESHOLD: f64 = 2.0;

const AD_KEYWORDS: &[&str] = &[
    "googleads",
    "googlesyndication",
    "doubleclick",
    "adsystem",
    "adserver",
    "google-analytics",
    "googletagmanager",
    "ads",
    "ad.",
    "pre-roll",
    "mid-roll",
    "post-roll",
    "cdn.ads",
];

/// Sanitize an HLS manifest by stripping ad segments.
/// Returns the cleaned manifest and the number of segments removed.
pub fn sanitize_manifest(input: &str) -> (String, usize) {
    let lines: Vec<&str> = input.lines().collect();
    let mut output: Vec<String> = Vec::with_capacity(lines.len());
    let mut removed = 0usize;

    let mut max_duration = 0.0f64;
    let mut target_duration_idx: Option<usize> = None;

    // First pass: identify targetduration line
    for (idx, line) in lines.iter().enumerate() {
        if line.starts_with("#EXT-X-TARGETDURATION:") {
            target_duration_idx = Some(idx);
        }
    }

    // Parse with a small state machine
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];

        // Track discontinuity boundaries
        if line.trim() == "#EXT-X-DISCONTINUITY" {
            // Peek ahead: if next segment is an ad, drop the discontinuity too
            if i + 2 < lines.len() {
                let next_line = lines[i + 1];
                let url_line = lines[i + 2];
                if is_ad_segment(next_line, url_line) {
                    // Skip discontinuity + extinf + url
                    i += 3;
                    removed += 1;
                    continue;
                }
            }
            output.push(line.to_string());
            i += 1;
            continue;
        }

        if line.starts_with("#EXTINF:") {
            let duration = parse_extinf_duration(line);
            let url_line = if i + 1 < lines.len() {
                lines[i + 1]
            } else {
                ""
            };

            if is_ad_segment(line, url_line) {
                // Skip this EXTINF + URL pair
                i += 2;
                removed += 1;
                continue;
            }

            // Keep it, track duration
            if duration > max_duration {
                max_duration = duration;
            }
            output.push(line.to_string());
            if !url_line.is_empty() && !url_line.starts_with('#') {
                output.push(url_line.to_string());
                i += 2;
                continue;
            }
            i += 1;
            continue;
        }

        // Non-EXTINF, non-discontinuity line
        if !line.starts_with('#') && !line.trim().is_empty() {
            // Standalone URL line without preceding EXTINF (edge case)
            if is_ad_url(line) {
                i += 1;
                removed += 1;
                continue;
            }
        }

        output.push(line.to_string());
        i += 1;
    }

    // Recompute target duration if we found the tag
    if let Some(idx) = target_duration_idx {
        if max_duration > 0.0 && idx < output.len() {
            let new_target = max_duration.ceil() as u32;
            output[idx] = format!("#EXT-X-TARGETDURATION:{}", new_target);
        }
    }

    (output.join("\n"), removed)
}

fn parse_extinf_duration(line: &str) -> f64 {
    // #EXTINF:10.000, or #EXTINF:10,
    line.strip_prefix("#EXTINF:")
        .and_then(|rest| rest.split(',').next())
        .and_then(|n| n.parse::<f64>().ok())
        .unwrap_or(0.0)
}

fn is_ad_segment(extinf_line: &str, url_line: &str) -> bool {
    let duration = parse_extinf_duration(extinf_line);
    if duration > 0.0 && duration < SUSPICIOUS_DURATION_THRESHOLD {
        return true;
    }
    is_ad_url(url_line)
}

fn is_ad_url(url: &str) -> bool {
    let lower = url.to_lowercase();
    for kw in AD_KEYWORDS {
        if lower.contains(kw) {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keep_clean_manifest() {
        let input = r#"#EXTM3U
#EXT-X-TARGETDURATION:10
#EXTINF:10.000,
segment1.ts
#EXTINF:10.000,
segment2.ts
#EXT-X-ENDLIST"#;
        let (out, removed) = sanitize_manifest(input);
        assert_eq!(removed, 0);
        assert!(out.contains("segment1.ts"));
        assert!(out.contains("segment2.ts"));
    }

    #[test]
    fn test_remove_short_ad_segments() {
        let input = r#"#EXTM3U
#EXT-X-TARGETDURATION:10
#EXTINF:10.000,
segment1.ts
#EXTINF:1.500,
ad.ts
#EXTINF:10.000,
segment2.ts
#EXT-X-ENDLIST"#;
        let (out, removed) = sanitize_manifest(input);
        assert_eq!(removed, 1);
        assert!(!out.contains("ad.ts"));
        assert!(out.contains("segment1.ts"));
        assert!(out.contains("segment2.ts"));
    }

    #[test]
    fn test_remove_ad_domain_segments() {
        let input = r#"#EXTM3U
#EXT-X-TARGETDURATION:10
#EXTINF:10.000,
segment1.ts
#EXTINF:5.000,
https://doubleclick.net/ad.ts
#EXTINF:10.000,
segment2.ts
#EXT-X-ENDLIST"#;
        let (out, removed) = sanitize_manifest(input);
        assert_eq!(removed, 1);
        assert!(!out.contains("doubleclick.net"));
    }

    #[test]
    fn test_remove_discontinuity_ad_block() {
        let input = r#"#EXTM3U
#EXT-X-TARGETDURATION:10
#EXTINF:10.000,
segment1.ts
#EXT-X-DISCONTINUITY
#EXTINF:1.000,
ad1.ts
#EXT-X-DISCONTINUITY
#EXTINF:10.000,
segment2.ts
#EXT-X-ENDLIST"#;
        let (out, removed) = sanitize_manifest(input);
        assert_eq!(removed, 1);
        assert!(!out.contains("ad1.ts"));
        // First discontinuity (before ad) removed; second (before segment2) kept
        assert_eq!(out.matches("#EXT-X-DISCONTINUITY").count(), 1);
        assert!(out.contains("segment1.ts"));
        assert!(out.contains("segment2.ts"));
    }

    #[test]
    fn test_recompute_target_duration_downward() {
        let input = r#"#EXTM3U
#EXT-X-TARGETDURATION:12
#EXTINF:12.000,
ads_long.ts
#EXTINF:8.000,
segment1.ts
#EXTINF:8.000,
segment2.ts
#EXT-X-ENDLIST"#;
        let (out, removed) = sanitize_manifest(input);
        assert_eq!(removed, 1);
        assert!(out.contains("#EXT-X-TARGETDURATION:8"));
        assert!(!out.contains("ads_long.ts"));
    }
}
