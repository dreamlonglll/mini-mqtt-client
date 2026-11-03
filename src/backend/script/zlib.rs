//! Compression behind the `zlib.*` script API: gzip, zlib (RFC 1950) and raw
//! deflate (RFC 1951), named as in Node.js.

use flate2::Compression;
use flate2::read::{DeflateDecoder, DeflateEncoder, GzEncoder, MultiGzDecoder, ZlibDecoder, ZlibEncoder};
use std::io::Read;

/// Decompressed data is capped so that a few kilobytes of crafted input
/// cannot take gigabytes of memory; the same as the largest MQTT packet.
pub const MAX_OUTPUT: usize = 64 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Gzip,
    Zlib,
    Raw,
}

impl Format {
    /// gzip or zlib, told apart by the gzip magic bytes (`zlib.unzip`).
    pub fn detect(data: &[u8]) -> Format {
        if data.starts_with(&[0x1f, 0x8b]) {
            Format::Gzip
        } else {
            Format::Zlib
        }
    }

    fn name(self) -> &'static str {
        match self {
            Format::Gzip => "gzip",
            Format::Zlib => "zlib",
            Format::Raw => "deflate",
        }
    }
}

/// `level` is 0 (store) to 9 (smallest).
pub fn compress(data: &[u8], format: Format, level: u32) -> Result<Vec<u8>, String> {
    let level = Compression::new(level);
    let mut out = Vec::new();
    match format {
        Format::Gzip => GzEncoder::new(data, level).read_to_end(&mut out),
        Format::Zlib => ZlibEncoder::new(data, level).read_to_end(&mut out),
        Format::Raw => DeflateEncoder::new(data, level).read_to_end(&mut out),
    }
    .map_err(|e| tr!("zlib.compress_failed", format = format.name(), error = e).to_string())?;
    Ok(out)
}

pub fn decompress(data: &[u8], format: Format) -> Result<Vec<u8>, String> {
    decompress_limited(data, format, MAX_OUTPUT)
}

/// A cut-off stream is an error ("incomplete deflate stream"), not a
/// shortened result; reading one byte past `limit` proves the output too large.
fn decompress_limited(data: &[u8], format: Format, limit: usize) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    let limited = |reader: &mut dyn Read, out: &mut Vec<u8>| reader.take(limit as u64 + 1).read_to_end(out);
    match format {
        // Concatenated members decode as one, like the gunzip tool.
        Format::Gzip => limited(&mut MultiGzDecoder::new(data), &mut out),
        Format::Zlib => limited(&mut ZlibDecoder::new(data), &mut out),
        Format::Raw => limited(&mut DeflateDecoder::new(data), &mut out),
    }
    .map_err(|e| tr!("zlib.decompress_failed", format = format.name(), error = e).to_string())?;
    if out.len() > limit {
        return Err(tr!("zlib.too_large", limit = format!("{} MB", limit / (1024 * 1024))).to_string());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEXT: &[u8] = "{\"msg\":\"hello mqtt 你好\",\"seq\":1}".as_bytes();

    #[test]
    fn every_format_round_trips() {
        let large: Vec<u8> = (0..200_000u32).flat_map(|i| (i % 251).to_le_bytes()).collect();
        for format in [Format::Gzip, Format::Zlib, Format::Raw] {
            for data in [TEXT, &[][..], &large[..]] {
                for level in [0, 6, 9] {
                    let packed = compress(data, format, level).unwrap();
                    assert_eq!(decompress(&packed, format).unwrap(), data, "{format:?} level {level}");
                }
            }
        }
    }

    #[test]
    fn formats_are_told_apart() {
        let gz = compress(TEXT, Format::Gzip, 6).unwrap();
        let zl = compress(TEXT, Format::Zlib, 6).unwrap();
        assert_eq!(&gz[..2], &[0x1f, 0x8b]);
        assert_eq!(Format::detect(&gz), Format::Gzip);
        assert_eq!(Format::detect(&zl), Format::Zlib);
        assert!(decompress(&gz, Format::Zlib).is_err());
        assert!(decompress(&zl, Format::Gzip).is_err());
        assert!(decompress(b"not compressed", Format::Gzip).is_err());
        assert!(decompress(b"not compressed", Format::Zlib).is_err());
    }

    #[test]
    fn concatenated_gzip_members_are_all_decoded() {
        let mut gz = compress(b"ab", Format::Gzip, 6).unwrap();
        gz.extend(compress(b"cd", Format::Gzip, 6).unwrap());
        assert_eq!(decompress(&gz, Format::Gzip).unwrap(), b"abcd");
    }

    #[test]
    fn truncated_input_is_an_error() {
        let large: Vec<u8> = (0..50_000u32).flat_map(|i| i.to_le_bytes()).collect();
        for format in [Format::Gzip, Format::Zlib, Format::Raw] {
            let packed = compress(&large, format, 6).unwrap();
            for cut in [1, 4, packed.len() / 2] {
                let result = decompress(&packed[..packed.len() - cut], format);
                assert!(result.is_err(), "{format:?} cut {cut}: {} bytes", result.unwrap().len());
            }
            assert!(decompress(&[], format).is_err(), "{format:?} empty");
        }
    }

    #[test]
    fn output_is_capped() {
        let zeros = vec![0u8; 100_000];
        for format in [Format::Gzip, Format::Zlib, Format::Raw] {
            let packed = compress(&zeros, format, 9).unwrap();
            assert_eq!(decompress_limited(&packed, format, 100_000).unwrap().len(), 100_000);
            let e = decompress_limited(&packed, format, 99_999).unwrap_err();
            assert!(e.contains("MB"), "{format:?}: {e}");
        }
    }
}
