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

/// Same heuristic as the Vue version: valid JSON object/array => JSON,
/// more than 10% non-printable bytes => binary, else text.
pub fn detect_format(bytes: &[u8]) -> DetectedFormat {
    if let Ok(s) = std::str::from_utf8(bytes) {
        let trimmed = s.trim();
        if ((trimmed.starts_with('{') && trimmed.ends_with('}'))
            || (trimmed.starts_with('[') && trimmed.ends_with(']')))
            && serde_json::from_str::<serde_json::Value>(trimmed).is_ok()
        {
            return DetectedFormat::Json;
        }
    }
    if !bytes.is_empty() {
        let non_printable = bytes
            .iter()
            .filter(|&&b| !(32..=126).contains(&b) && b != 9 && b != 10 && b != 13)
            .count();
        // Multi-byte UTF-8 text (e.g. Chinese) is not binary.
        let is_utf8 = std::str::from_utf8(bytes).is_ok();
        if !is_utf8 && non_printable * 10 > bytes.len() {
            return DetectedFormat::Binary;
        }
        if is_utf8
            && bytes
                .iter()
                .filter(|&&b| b < 32 && b != 9 && b != 10 && b != 13)
                .count()
                * 10
                > bytes.len()
        {
            return DetectedFormat::Binary;
        }
    }
    DetectedFormat::Text
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
        return Err("HEX 格式不正确".to_string());
    }
    if !cleaned.len().is_multiple_of(2) {
        return Err("HEX 长度必须为偶数".to_string());
    }
    hex::decode(&cleaned).map_err(|e| format!("HEX解码失败: {e}"))
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
            .map_err(|e| format!("JSON 格式不正确: {e}")),
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
    fn validates_payloads() {
        assert!(validate_payload("{\"a\":1}", PayloadFormat::Json).is_ok());
        assert!(validate_payload("", PayloadFormat::Json).is_ok());
        assert!(validate_payload("{", PayloadFormat::Json).is_err());
        assert!(validate_payload("{", PayloadFormat::Text).is_ok());
        assert_eq!(encode_payload("01 02", PayloadFormat::Hex).unwrap(), vec![1, 2]);
    }
}
