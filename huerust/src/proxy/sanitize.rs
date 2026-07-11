//! Hue Stream API
//! Professional Streaming Aggregator
//!
//! @author roywalk3r <roywalker440@gmail.com>
//! @repository https://github.com/roywalk3r/hue.git
//! @license MIT

//! MPEG-TS Stream Sanitization
//!
//! Detects and strips injected ads/watermarks from `.ts` segments.
//! Currently handles:
//! - SCTE-35 ad marker packets (PID 0x0100)
//! - Null packets (PID 0x1FFF) — often used as padding for ad injection
//! - Known bad provider signatures

use std::collections::HashSet;

const TS_PACKET_SIZE: usize = 188;
const SYNC_BYTE: u8 = 0x47;

/// Known ad-related PIDs to strip
const DEFAULT_AD_PIDS: [u16; 2] = [0x0100, 0x1FFF];

/// Sanitize a TS segment buffer by stripping ad packets.
/// Returns the cleaned buffer and the number of packets removed.
pub fn sanitize_ts_segment(input: &[u8]) -> (Vec<u8>, usize) {
    let ad_pids: HashSet<u16> = DEFAULT_AD_PIDS.iter().copied().collect();
    let mut output = Vec::with_capacity(input.len());
    let mut stripped = 0;
    let mut offset = 0;

    while offset + TS_PACKET_SIZE <= input.len() {
        let packet = &input[offset..offset + TS_PACKET_SIZE];

        if packet[0] != SYNC_BYTE {
            // Not a valid TS packet — keep as-is to avoid corruption
            output.extend_from_slice(packet);
            offset += TS_PACKET_SIZE;
            continue;
        }

        let pid = extract_pid(packet);

        if ad_pids.contains(&pid) {
            stripped += 1;
            offset += TS_PACKET_SIZE;
            continue;
        }

        // Check for known ad signatures in payload
        if contains_ad_signature(packet) {
            stripped += 1;
            offset += TS_PACKET_SIZE;
            continue;
        }

        output.extend_from_slice(packet);
        offset += TS_PACKET_SIZE;
    }

    // Append any trailing bytes (shouldn't happen for well-formed TS)
    if offset < input.len() {
        output.extend_from_slice(&input[offset..]);
    }

    (output, stripped)
}

/// Extract 13-bit PID from TS packet header
fn extract_pid(packet: &[u8]) -> u16 {
    ((packet[1] as u16 & 0x1F) << 8) | (packet[2] as u16)
}

/// Check for known ad signatures in packet payload
fn contains_ad_signature(packet: &[u8]) -> bool {
    // Look for common ad-injection signatures
    let payload_start = if has_adaptation_field(packet) {
        let adaptation_length = packet[4] as usize;
        5 + adaptation_length
    } else {
        4
    };

    if payload_start >= packet.len() {
        return false;
    }

    let payload = &packet[payload_start..];

    // Check for SCTE-35 splice_info_section header (table_id 0xFC)
    if payload.len() > 3 && payload[0] == 0xFC {
        return true;
    }

    // Check for common ad server domains in payload (heuristic)
    let payload_str = String::from_utf8_lossy(payload);
    if payload_str.contains("googlesyndication")
        || payload_str.contains("doubleclick")
        || payload_str.contains("adsystem")
        || payload_str.contains("adserver")
    {
        return true;
    }

    false
}

fn has_adaptation_field(packet: &[u8]) -> bool {
    packet[3] & 0x20 != 0
}

/// Validate that a buffer looks like valid MPEG-TS
pub fn is_valid_ts(input: &[u8]) -> bool {
    if input.len() < TS_PACKET_SIZE {
        return false;
    }
    input[0] == SYNC_BYTE && input.len() % TS_PACKET_SIZE == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_ts_packet(pid: u16, payload: &[u8]) -> Vec<u8> {
        let mut packet = vec![0u8; TS_PACKET_SIZE];
        packet[0] = SYNC_BYTE;
        packet[1] = ((pid >> 8) & 0x1F) as u8;
        packet[2] = (pid & 0xFF) as u8;
        packet[3] = 0x10; // Payload only, no adaptation
        let copy_len = payload.len().min(TS_PACKET_SIZE - 4);
        packet[4..4 + copy_len].copy_from_slice(&payload[..copy_len]);
        packet
    }

    #[test]
    fn test_extract_pid() {
        let packet = make_ts_packet(0x0100, b"hello");
        assert_eq!(extract_pid(&packet), 0x0100);
    }

    #[test]
    fn test_sanitize_strips_scte35_pid() {
        let mut input = make_ts_packet(0x0100, b"ad_marker");
        input.extend(make_ts_packet(0x0101, b"real_video"));
        let (output, stripped) = sanitize_ts_segment(&input);
        assert_eq!(stripped, 1);
        assert_eq!(output.len(), TS_PACKET_SIZE);
    }

    #[test]
    fn test_sanitize_strips_null_packets() {
        let mut input = make_ts_packet(0x1FFF, &[0; 10]);
        input.extend(make_ts_packet(0x0101, b"real"));
        let (output, stripped) = sanitize_ts_segment(&input);
        assert_eq!(stripped, 1);
        assert_eq!(output.len(), TS_PACKET_SIZE);
    }

    #[test]
    fn test_sanitize_keeps_good_packets() {
        let mut input = make_ts_packet(0x0101, b"video");
        input.extend(make_ts_packet(0x0102, b"audio"));
        let (output, stripped) = sanitize_ts_segment(&input);
        assert_eq!(stripped, 0);
        assert_eq!(output.len(), input.len());
    }

    #[test]
    fn test_is_valid_ts() {
        assert!(!is_valid_ts(b"not ts"));
        assert!(is_valid_ts(&make_ts_packet(0x0101, b"v")));
    }

    #[test]
    fn test_ad_signature_scte35() {
        let payload = vec![0xFC, 0x00, 0x00, 0x00];
        let packet = make_ts_packet(0x0101, &payload);
        assert!(contains_ad_signature(&packet));
    }
}
