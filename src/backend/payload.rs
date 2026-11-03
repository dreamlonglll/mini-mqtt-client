//! Payload helpers: format detection, HEX encode/decode, JSON formatting.

use super::models::PayloadFormat;

/// Format detected for display in the message list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectedFormat {
    Json,
    Binary,
    Text,
}

impl DetectedFormat {
    pub fn label(self) -> &'static str {
        match self {
            DetectedFormat::Json => "JSON",
            DetectedFormat::Binary => "BIN",
            DetectedFormat::Text => "TEXT",
        }
    }

    /// Format used when copying a message into the publish panel.
    pub fn as_publish_format(self) -> PayloadFormat {
        match self {
            DetectedFormat::Json => PayloadFormat::Json,
            DetectedFormat::Binary => PayloadFormat::Hex,
            DetectedFormat::Text => PayloadFormat::Text,
        }
    }
}

/// Payloads above this size are classified from a sample instead of being
/// fully parsed, so a multi-megabyte message does not stall the UI thread.
const FULL_DETECTION_LIMIT: usize = 1024 * 1024;
const SAMPLE_SIZE: usize = 64 * 1024;

/// Same heuristic as the Vue version: valid JSON object/array => JSON,
/// more than 10% non-printable bytes => binary, else text.
pub fn detect_format(bytes: &[u8]) -> DetectedFormat {
    let large = bytes.len() > FULL_DETECTION_LIMIT;
    let sample = if large { &bytes[..SAMPLE_SIZE] } else { bytes };
    // A cut-off sample may end inside a multi-byte character.
    let utf8 = match std::str::from_utf8(sample) {
        Ok(s) => Some(s),
        Err(e) if large && e.error_len().is_none() => std::str::from_utf8(&sample[..e.valid_up_to()]).ok(),
        Err(_) => None,
    };
    if let Some(s) = utf8 {
        let trimmed = s.trim_start();
        let looks_json = trimmed.starts_with('{') || trimmed.starts_with('[');
        if looks_json {
            if large {
                // Too big to validate cheaply: trust the delimiters.
                let end = bytes.iter().rev().find(|b| !b.is_ascii_whitespace());
                if matches!((trimmed.as_bytes()[0], end), (b'{', Some(b'}')) | (b'[', Some(b']'))) {
                    return DetectedFormat::Json;
                }
            } else {
                let trimmed = s.trim();
                if ((trimmed.starts_with('{') && trimmed.ends_with('}'))
                    || (trimmed.starts_with('[') && trimmed.ends_with(']')))
                    && serde_json::from_str::<serde_json::Value>(trimmed).is_ok()
                {
                    return DetectedFormat::Json;
                }
            }
        }
    }
    if !sample.is_empty() {
        let control = sample
            .iter()
            .filter(|&&b| b < 32 && b != 9 && b != 10 && b != 13)
            .count();
        if utf8.is_some() {
            // Multi-byte UTF-8 text (e.g. Chinese) is not binary.
            if control * 10 > sample.len() {
                return DetectedFormat::Binary;
            }
        } else {
            let non_printable = sample
                .iter()
                .filter(|&&b| !(32..=126).contains(&b) && b != 9 && b != 10 && b != 13)
                .count();
            if non_printable * 10 > sample.len() {
                return DetectedFormat::Binary;
            }
        }
    }
    DetectedFormat::Text
}

/// Lossy UTF-8 text of at most `max_bytes` bytes of the payload (cut on a
/// character boundary), plus whether it was cut.
pub fn text_prefix(bytes: &[u8], max_bytes: usize) -> (String, bool) {
    if bytes.len() <= max_bytes {
        return (bytes_to_text(bytes), false);
    }
    let mut end = max_bytes;
    // Do not split a UTF-8 sequence: back off over continuation bytes.
    while end > 0 && (bytes[end] & 0b1100_0000) == 0b1000_0000 {
        end -= 1;
    }
    (bytes_to_text(&bytes[..end]), true)
}

pub fn bytes_to_text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// `48 65 6C 6C 6F`
pub fn to_hex_spaced(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(" ")
}

/// Decode a HEX string, ignoring whitespace (and an optional `0x` prefix per byte).
pub fn parse_hex(input: &str) -> Result<Vec<u8>, String> {
    let cleaned: String = input
        .split_whitespace()
        .map(|part| part.trim_start_matches("0x").trim_start_matches("0X"))
        .collect();
    if !cleaned.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(tr!("payload.hex_invalid").to_string());
    }
    if !cleaned.len().is_multiple_of(2) {
        return Err(tr!("payload.hex_odd").to_string());
    }
    hex::decode(&cleaned).map_err(|e| tr!("payload.hex_decode_failed", error = e).to_string())
}

/// Put a `↵` before every line break (`\n` or `\r\n`), as the previous
/// version's detail view did.
pub fn mark_line_breaks(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + text.len() / 16);
    let mut rest = text;
    while let Some(ix) = rest.find('\n') {
        let line = &rest[..ix];
        match line.strip_suffix('\r') {
            Some(line) => {
                out.push_str(line);
                out.push_str("↵\r\n");
            }
            None => {
                out.push_str(line);
                out.push_str("↵\n");
            }
        }
        rest = &rest[ix + 1..];
    }
    out.push_str(rest);
    out
}

/// One row of a classic hex dump.
pub struct HexRow {
    pub offset: String,
    pub hex: String,
    pub ascii: String,
}

pub fn hex_dump(bytes: &[u8]) -> Vec<HexRow> {
    bytes
        .chunks(16)
        .enumerate()
        .map(|(i, chunk)| {
            let mut hex = chunk.iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(" ");
            let pad = 16 - chunk.len();
            hex.push_str(&"   ".repeat(pad));
            let ascii = chunk
                .iter()
                .map(|&b| if (32..=126).contains(&b) { b as char } else { '.' })
                .collect();
            HexRow {
                offset: format!("{:08X}", i * 16),
                hex,
                ascii,
            }
        })
        .collect()
}

pub fn pretty_json(text: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(text.trim()).ok()?;
    serde_json::to_string_pretty(&value).ok()
}

pub fn compact_json(text: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(text.trim()).ok()?;
    serde_json::to_string(&value).ok()
}

/// Validate a payload typed by the user for the given format.
pub fn validate_payload(payload: &str, format: PayloadFormat) -> Result<(), String> {
    match format {
        PayloadFormat::Hex => parse_hex(payload).map(|_| ()),
        PayloadFormat::Json if !payload.trim().is_empty() => serde_json::from_str::<serde_json::Value>(payload)
            .map(|_| ())
            .map_err(|e| tr!("payload.json_invalid", error = e).to_string()),
        _ => Ok(()),
    }
}

/// Encode the (script-processed) payload text into bytes to send.
pub fn encode_payload(payload: &str, format: PayloadFormat) -> Result<Vec<u8>, String> {
    match format {
        PayloadFormat::Hex => parse_hex(payload),
        _ => Ok(payload.as_bytes().to_vec()),
    }
}

/// Text preview of at most `max_chars` characters.
pub fn truncate(text: &str, max_chars: usize) -> String {
    if text.chars().count() > max_chars {
        let mut s: String = text.chars().take(max_chars).collect();
        s.push('…');
        s
    } else {
        text.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_formats() {
        assert_eq!(detect_format(br#"{"a":1}"#), DetectedFormat::Json);
        assert_eq!(detect_format(b"[1,2]"), DetectedFormat::Json);
        assert_eq!(detect_format(b"{not json}"), DetectedFormat::Text);
        assert_eq!(detect_format("你好，世界".as_bytes()), DetectedFormat::Text);
        assert_eq!(detect_format(&[0, 1, 2, 3, 0xff, 0xfe]), DetectedFormat::Binary);
    }

    #[test]
    fn large_payloads_are_classified_from_a_sample() {
        let mut json = b"{\"data\":\"".to_vec();
        json.extend(std::iter::repeat_n(b'x', 3 * 1024 * 1024));
        json.extend(b"\"}");
        assert_eq!(detect_format(&json), DetectedFormat::Json);
        let text = "汉".repeat(1024 * 1024); // 3 MiB, sample cut mid-character
        assert_eq!(detect_format(text.as_bytes()), DetectedFormat::Text);
        let binary: Vec<u8> = (0..2 * 1024 * 1024).map(|i| (i % 256) as u8).collect();
        assert_eq!(detect_format(&binary), DetectedFormat::Binary);
        let (prefix, cut) = text_prefix(text.as_bytes(), 10);
        assert!(cut);
        assert_eq!(prefix, "汉汉汉");
        assert_eq!(text_prefix(b"short", 10), ("short".to_string(), false));
    }

    #[test]
    fn hex_roundtrip() {
        assert_eq!(parse_hex("48 65 6c 6C 6f").unwrap(), b"Hello");
        assert_eq!(parse_hex("0x48 0x65").unwrap(), b"He");
        assert!(parse_hex("4").is_err());
        assert!(parse_hex("zz").is_err());
        assert_eq!(to_hex_spaced(b"Hi"), "48 69");
        let rows = hex_dump(b"Hello, world! 123456");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1].offset, "00000010");
    }

    #[test]
    fn marks_line_breaks() {
        assert_eq!(mark_line_breaks("a\nb\r\nc"), "a↵\nb↵\r\nc");
        assert_eq!(mark_line_breaks("end\n"), "end↵\n");
        assert_eq!(mark_line_breaks("none"), "none");
    }

    #[test]
    fn validates_payloads() {
        assert!(validate_payload("{\"a\":1}", PayloadFormat::Json).is_ok());
        assert!(validate_payload("", PayloadFormat::Json).is_ok());
        assert!(validate_payload("{", PayloadFormat::Json).is_err());
        assert!(validate_payload("{", PayloadFormat::Text).is_ok());
        assert_eq!(encode_payload("01 02", PayloadFormat::Hex).unwrap(), vec![1, 2]);
    }
}
