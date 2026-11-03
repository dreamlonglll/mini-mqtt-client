//! "Check for updates": ask GitHub for the latest release and compare it with
//! the running version.
//!
//! The request is a single HTTPS GET made with the rustls (ring) stack the
//! MQTT client already uses, so no HTTP client crate (and no aws-lc / cmake /
//! NASM build dependency) is needed. It blocks, so callers run it on a
//! background thread.

use std::cmp::Ordering;
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::Arc;
use std::time::Duration;
use tokio_rustls::rustls;

pub const LATEST_RELEASE_API: &str = "https://api.github.com/repos/dreamlonglll/mini-mqtt-client/releases/latest";
/// Page opened by "Download".
pub const RELEASES_PAGE: &str = "https://github.com/dreamlonglll/mini-mqtt-client/releases/latest";

const TIMEOUT: Duration = Duration::from_secs(10);
const MAX_RESPONSE: usize = 2 * 1024 * 1024;
const MAX_REDIRECTS: usize = 3;

pub fn current_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Result of a successful check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateInfo {
    /// Latest released version, without the leading `v`.
    pub latest: String,
    pub has_update: bool,
}

/// `MINI_MQTT_NO_UPDATE_CHECK=1` turns the automatic check at start-up off
/// (used by the end-to-end tests, which must not depend on the network).
pub fn auto_check_disabled() -> bool {
    std::env::var("MINI_MQTT_NO_UPDATE_CHECK").is_ok_and(|v| !v.is_empty() && v != "0")
}

/// Ask GitHub for the latest release (blocking, up to ~10 s per step).
pub fn check() -> Result<UpdateInfo, String> {
    let body = fetch(LATEST_RELEASE_API)?;
    let latest = parse_latest_tag(&body)?;
    let has_update = compare_versions(&latest, current_version()) == Ordering::Greater;
    Ok(UpdateInfo { latest, has_update })
}

/// `tag_name` of a GitHub release (`v2.1.0` -> `2.1.0`).
pub fn parse_latest_tag(body: &[u8]) -> Result<String, String> {
    let json: serde_json::Value = serde_json::from_slice(body).map_err(|e| e.to_string())?;
    let tag = json
        .get("tag_name")
        .and_then(|t| t.as_str())
        .map(|t| t.trim().trim_start_matches(['v', 'V']).to_string())
        .filter(|t| !t.is_empty())
        .ok_or("no tag_name in the release")?;
    Ok(tag)
}

/// Compare dotted version numbers (`2.10.0` > `2.9.1`; missing parts count
/// as 0). A pre-release (`2.1.0-beta.1`) sorts before its release.
pub fn compare_versions(a: &str, b: &str) -> Ordering {
    fn split(v: &str) -> (Vec<u64>, bool) {
        let v = v.trim().trim_start_matches(['v', 'V']);
        let (core, pre) = match v.split_once(['-', '+']) {
            Some((core, rest)) => (core, v[core.len()..].starts_with('-') && !rest.is_empty()),
            None => (v, false),
        };
        let parts = core
            .split('.')
            .map(|p| {
                let digits: String = p.chars().take_while(|c| c.is_ascii_digit()).collect();
                digits.parse().unwrap_or(0)
            })
            .collect();
        (parts, pre)
    }
    let ((pa, pre_a), (pb, pre_b)) = (split(a), split(b));
    for i in 0..pa.len().max(pb.len()) {
        let (x, y) = (pa.get(i).copied().unwrap_or(0), pb.get(i).copied().unwrap_or(0));
        match x.cmp(&y) {
            Ordering::Equal => continue,
            other => return other,
        }
    }
    match (pre_a, pre_b) {
        (true, false) => Ordering::Less,
        (false, true) => Ordering::Greater,
        _ => Ordering::Equal,
    }
}

// ---------------------------------------------------------------------------
// Minimal HTTPS GET

struct Url<'a> {
    host: &'a str,
    port: u16,
    path: &'a str,
}

fn parse_url(url: &str) -> Result<Url<'_>, String> {
    let rest = url.strip_prefix("https://").ok_or("only https URLs are supported")?;
    let (authority, path) = match rest.find('/') {
        Some(ix) => (&rest[..ix], &rest[ix..]),
        None => (rest, "/"),
    };
    let (host, port) = match authority.rsplit_once(':') {
        Some((host, port)) => (host, port.parse().map_err(|_| "invalid port")?),
        None => (authority, 443),
    };
    if host.is_empty() {
        return Err("missing host".into());
    }
    Ok(Url { host, port, path })
}

fn fetch(url: &str) -> Result<Vec<u8>, String> {
    let mut url = url.to_string();
    for _ in 0..=MAX_REDIRECTS {
        let response = get(&parse_url(&url)?)?;
        match response.status {
            200 => return Ok(response.body),
            301 | 302 | 303 | 307 | 308 => {
                url = response.location.ok_or("redirect without a location")?;
            }
            403 | 429 => return Err(format!("HTTP {} (rate limited)", response.status)),
            404 => return Err("HTTP 404 (no release published)".into()),
            status => return Err(format!("HTTP {status}")),
        }
    }
    Err("too many redirects".into())
}

struct Response {
    status: u16,
    location: Option<String>,
    body: Vec<u8>,
}

fn tls_config() -> Result<Arc<rustls::ClientConfig>, String> {
    let mut roots = rustls::RootCertStore::empty();
    for cert in rustls_native_certs::load_native_certs().certs {
        let _ = roots.add(cert);
    }
    if roots.is_empty() {
        return Err("no system root certificates".into());
    }
    Ok(Arc::new(
        rustls::ClientConfig::builder()
            .with_root_certificates(roots)
            .with_no_client_auth(),
    ))
}

fn get(url: &Url) -> Result<Response, String> {
    let addrs = (url.host, url.port).to_socket_addrs().map_err(|e| e.to_string())?;
    let mut last_error = String::from("no address");
    let mut tcp = None;
    for addr in addrs {
        match TcpStream::connect_timeout(&addr, TIMEOUT) {
            Ok(stream) => {
                tcp = Some(stream);
                break;
            }
            Err(e) => last_error = e.to_string(),
        }
    }
    let tcp = tcp.ok_or(last_error)?;
    tcp.set_read_timeout(Some(TIMEOUT)).map_err(|e| e.to_string())?;
    tcp.set_write_timeout(Some(TIMEOUT)).map_err(|e| e.to_string())?;

    let server_name = rustls::pki_types::ServerName::try_from(url.host.to_string()).map_err(|e| e.to_string())?;
    let conn = rustls::ClientConnection::new(tls_config()?, server_name).map_err(|e| e.to_string())?;
    let mut tls = rustls::StreamOwned::new(conn, tcp);
    let request = format!(
        "GET {} HTTP/1.1\r\nHost: {}\r\nUser-Agent: mini-mqtt-client/{}\r\nAccept: application/vnd.github+json\r\nConnection: close\r\n\r\n",
        url.path,
        url.host,
        current_version()
    );
    tls.write_all(request.as_bytes()).map_err(|e| e.to_string())?;

    let mut raw = Vec::new();
    let mut chunk = [0u8; 16 * 1024];
    loop {
        match tls.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => {
                raw.extend_from_slice(&chunk[..n]);
                if raw.len() > MAX_RESPONSE {
                    return Err("response too large".into());
                }
                if response_complete(&raw) {
                    break;
                }
            }
            // Servers often close without a TLS close_notify; what arrived
            // is complete if it parses.
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(e.to_string()),
        }
    }
    parse_response(&raw)
}

fn header_end(raw: &[u8]) -> Option<usize> {
    raw.windows(4).position(|w| w == b"\r\n\r\n").map(|ix| ix + 4)
}

/// Whether a full response (headers and a body of known length) has arrived,
/// so the read loop does not wait for the server to close the connection.
fn response_complete(raw: &[u8]) -> bool {
    let Some(end) = header_end(raw) else {
        return false;
    };
    let head = String::from_utf8_lossy(&raw[..end]).to_ascii_lowercase();
    let body = &raw[end..];
    if head.contains("transfer-encoding: chunked") {
        return decode_chunked(body).is_ok();
    }
    head.lines()
        .find_map(|l| l.strip_prefix("content-length:"))
        .and_then(|v| v.trim().parse::<usize>().ok())
        .is_some_and(|len| body.len() >= len)
}

fn parse_response(raw: &[u8]) -> Result<Response, String> {
    let end = header_end(raw).ok_or("incomplete HTTP response")?;
    let head = String::from_utf8_lossy(&raw[..end - 4]).into_owned();
    let mut lines = head.split("\r\n");
    let status_line = lines.next().unwrap_or_default();
    let status: u16 = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| format!("bad status line: {status_line}"))?;
    let mut chunked = false;
    let mut length = None;
    let mut location = None;
    for line in lines {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim();
        match name.trim().to_ascii_lowercase().as_str() {
            "transfer-encoding" => chunked = value.to_ascii_lowercase().contains("chunked"),
            "content-length" => length = value.parse::<usize>().ok(),
            "location" => location = Some(value.to_string()),
            _ => {}
        }
    }
    let body = &raw[end..];
    let body = if chunked {
        decode_chunked(body)?
    } else if let Some(len) = length {
        if body.len() < len {
            return Err("truncated HTTP response".into());
        }
        body[..len].to_vec()
    } else {
        body.to_vec()
    };
    Ok(Response { status, location, body })
}

fn decode_chunked(mut data: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    loop {
        let line_end = data
            .windows(2)
            .position(|w| w == b"\r\n")
            .ok_or("incomplete chunk size")?;
        let size_line = String::from_utf8_lossy(&data[..line_end]);
        let size_hex = size_line.split(';').next().unwrap_or_default().trim();
        let size = usize::from_str_radix(size_hex, 16).map_err(|_| "bad chunk size")?;
        data = &data[line_end + 2..];
        if size == 0 {
            return Ok(out);
        }
        if data.len() < size + 2 {
            return Err("incomplete chunk".into());
        }
        out.extend_from_slice(&data[..size]);
        data = &data[size + 2..];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_versions_numerically() {
        assert_eq!(compare_versions("2.0.1", "2.0.0"), Ordering::Greater);
        assert_eq!(compare_versions("2.10.0", "2.9.9"), Ordering::Greater);
        assert_eq!(compare_versions("v2.0", "2.0.0"), Ordering::Equal);
        assert_eq!(compare_versions("1.7.1", "2.0.0"), Ordering::Less);
        assert_eq!(compare_versions("2.1.0-beta.1", "2.1.0"), Ordering::Less);
        assert_eq!(compare_versions("2.1.0-beta.1", "2.0.0"), Ordering::Greater);
        assert_eq!(compare_versions("2.0.0+build.5", "2.0.0"), Ordering::Equal);
    }

    #[test]
    fn reads_the_release_tag() {
        assert_eq!(
            parse_latest_tag(br#"{"tag_name":"v2.1.0","name":"x"}"#).unwrap(),
            "2.1.0"
        );
        assert!(parse_latest_tag(br#"{"message":"Not Found"}"#).is_err());
        assert!(parse_latest_tag(b"<html>").is_err());
    }

    #[test]
    fn parses_urls() {
        let url = parse_url(LATEST_RELEASE_API).unwrap();
        assert_eq!((url.host, url.port), ("api.github.com", 443));
        assert_eq!(url.path, "/repos/dreamlonglll/mini-mqtt-client/releases/latest");
        let url = parse_url("https://example.com:8443").unwrap();
        assert_eq!((url.host, url.port, url.path), ("example.com", 8443, "/"));
        assert!(parse_url("http://example.com/").is_err());
    }

    #[test]
    fn parses_http_responses() {
        let raw = b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 5\r\n\r\nhello trailing";
        assert!(response_complete(raw));
        let r = parse_response(raw).unwrap();
        assert_eq!((r.status, r.body.as_slice()), (200, &b"hello"[..]));

        let chunked =
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n4\r\nWiki\r\n6;ext=1\r\npedia \r\n0\r\n\r\n";
        assert!(response_complete(chunked));
        assert_eq!(parse_response(chunked).unwrap().body, b"Wikipedia ");
        assert!(!response_complete(&chunked[..chunked.len() - 7]));

        let redirect = b"HTTP/1.1 301 Moved Permanently\r\nLocation: https://api.github.com/repositories/1/releases/latest\r\nContent-Length: 0\r\n\r\n";
        let r = parse_response(redirect).unwrap();
        assert_eq!(r.status, 301);
        assert_eq!(
            r.location.as_deref(),
            Some("https://api.github.com/repositories/1/releases/latest")
        );

        assert!(parse_response(b"HTTP/1.1 200 OK\r\nContent-Length: 10\r\n\r\nshort").is_err());
        assert!(parse_response(b"garbage").is_err());
    }

    /// Talks to GitHub. Run with `cargo test -- --ignored latest_release_from_github`.
    #[test]
    #[ignore]
    fn latest_release_from_github() {
        let info = check().expect("update check");
        assert!(!info.latest.is_empty());
        println!("latest release: {} (update: {})", info.latest, info.has_update);
    }
}
