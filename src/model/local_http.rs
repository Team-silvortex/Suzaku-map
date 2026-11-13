//! Bounded loopback-only HTTP transport, independent of provider configuration.
use super::{MAX_RESPONSE_BYTES, check_cancellation};
use crate::prediction::{PredictionCancellation, PredictionError};
use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ParsedHttpEndpoint {
    pub(super) host: String,
    pub(super) port: u16,
    pub(super) path: String,
}

fn io_error(error: std::io::Error) -> PredictionError {
    match error.kind() {
        std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock => PredictionError::Timeout,
        _ => PredictionError::Unavailable,
    }
}

pub(super) fn http_request(
    endpoint: &str,
    method: &str,
    body: Option<&str>,
    timeout: Duration,
) -> Result<String, PredictionError> {
    http_request_with_cancel(endpoint, method, body, timeout, None)
}

pub(super) fn http_request_with_cancel(
    endpoint: &str,
    method: &str,
    body: Option<&str>,
    timeout: Duration,
    cancellation: Option<&PredictionCancellation>,
) -> Result<String, PredictionError> {
    let endpoint = parse_http_endpoint(endpoint).ok_or(PredictionError::InvalidEndpoint)?;
    let address = loopback_address(&endpoint).ok_or(PredictionError::InvalidEndpoint)?;
    let deadline = Instant::now() + timeout;
    let remaining = || {
        check_cancellation(cancellation)?;
        deadline
            .checked_duration_since(Instant::now())
            .filter(|duration| !duration.is_zero())
            .ok_or(PredictionError::Timeout)
    };
    // Poll only on this worker's existing socket, preserving one absolute
    // deadline. No extra thread, reconnect, replay or new HTTP request.
    let io_budget = || {
        remaining().map(|left| {
            if cancellation.is_some() {
                left.min(Duration::from_millis(50))
            } else {
                left
            }
        })
    };
    let mut stream = TcpStream::connect_timeout(&address, remaining()?).map_err(io_error)?;
    let body = body.unwrap_or("");
    let wire = format!(
        "{method} {} HTTP/1.1\r\nHost: {}:{}\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{body}",
        endpoint.path,
        endpoint.host,
        endpoint.port,
        body.len()
    );
    let mut written = 0;
    while written < wire.len() {
        stream
            .set_write_timeout(Some(io_budget()?))
            .map_err(io_error)?;
        match stream.write(&wire.as_bytes()[written..]) {
            Ok(0) => return Err(PredictionError::Unavailable),
            Ok(count) => written += count,
            Err(error) if pollable_io(&error, cancellation.is_some()) => continue,
            Err(error) => return Err(io_error(error)),
        }
    }
    let mut response = Vec::new();
    let mut buffer = [0u8; 4096];
    loop {
        stream
            .set_read_timeout(Some(io_budget()?))
            .map_err(io_error)?;
        let count = match stream.read(&mut buffer) {
            Ok(count) => count,
            Err(error) if pollable_io(&error, cancellation.is_some()) => continue,
            Err(error) => return Err(io_error(error)),
        };
        check_cancellation(cancellation)?;
        if response.len() + count > MAX_RESPONSE_BYTES {
            return Err(PredictionError::ResponseTooLarge);
        }
        response.extend_from_slice(&buffer[..count]);
        if let Some(line_end) = response.windows(2).position(|part| part == b"\r\n") {
            let status_line = std::str::from_utf8(&response[..line_end])
                .map_err(|_| PredictionError::InvalidResponse)?;
            let mut parts = status_line.split_whitespace();
            if !matches!(parts.next(), Some("HTTP/1.0" | "HTTP/1.1")) {
                return Err(PredictionError::InvalidResponse);
            }
            let status = parts
                .next()
                .and_then(|value| value.parse::<u16>().ok())
                .ok_or(PredictionError::InvalidResponse)?;
            if status != 200 {
                return Err(PredictionError::HttpStatus(status));
            }
        }
        if let Some(body) = decode_http_response(&response, count == 0) {
            return String::from_utf8(body).map_err(|_| PredictionError::InvalidResponse);
        }
        if count == 0 {
            return Err(PredictionError::InvalidResponse);
        }
        if response.len() > 16 * 1024 && !response.windows(4).any(|part| part == b"\r\n\r\n") {
            return Err(PredictionError::ResponseTooLarge);
        }
    }
}

fn pollable_io(error: &std::io::Error, cancellable: bool) -> bool {
    error.kind() == std::io::ErrorKind::Interrupted
        || (cancellable
            && matches!(
                error.kind(),
                std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
            ))
}

/// Plain HTTP is deliberately local-only. No redirects, DNS lookup or remote credentials.
pub fn is_local_llm_endpoint(endpoint: &str) -> bool {
    parse_http_endpoint(endpoint)
        .as_ref()
        .and_then(loopback_address)
        .is_some()
}
pub(super) fn parse_http_endpoint(endpoint: &str) -> Option<ParsedHttpEndpoint> {
    if endpoint.len() > 2048
        || endpoint
            .chars()
            .any(|ch| ch.is_whitespace() || ch.is_control())
        || endpoint.contains(['@', '#', '?', '\\'])
    {
        return None;
    }
    let tail = endpoint.strip_prefix("http://")?;
    let (authority, path) = tail
        .split_once('/')
        .map(|(host, path)| (host, format!("/{path}")))
        .unwrap_or((tail, "/v1/chat/completions".into()));
    let (host, port) = if let Some(bracketed) = authority.strip_prefix('[') {
        let (host, suffix) = bracketed.split_once(']')?;
        let port = if suffix.is_empty() {
            80
        } else {
            suffix.strip_prefix(':')?.parse().ok()?
        };
        (format!("[{host}]"), port)
    } else if let Some((host, port)) = authority.split_once(':') {
        (host.to_string(), port.parse().ok()?)
    } else {
        (authority.to_string(), 80)
    };
    if host.is_empty() || port == 0 || !path.starts_with('/') {
        return None;
    }
    Some(ParsedHttpEndpoint { host, port, path })
}
fn loopback_address(endpoint: &ParsedHttpEndpoint) -> Option<SocketAddr> {
    let host = endpoint.host.trim_matches(['[', ']']);
    let ip = if host.eq_ignore_ascii_case("localhost") {
        IpAddr::V4(Ipv4Addr::LOCALHOST)
    } else {
        host.parse::<IpAddr>().ok()?
    };
    ip.is_loopback()
        .then_some(SocketAddr::new(ip, endpoint.port))
}

pub(super) fn decode_http_response(response: &[u8], eof: bool) -> Option<Vec<u8>> {
    let split = response.windows(4).position(|bytes| bytes == b"\r\n\r\n")?;
    if split > 16 * 1024 {
        return None;
    }
    let headers = std::str::from_utf8(&response[..split]).ok()?;
    let mut lines = headers.lines();
    let status = lines.next()?.split_whitespace().collect::<Vec<_>>();
    if status.len() < 2 || !matches!(status[0], "HTTP/1.0" | "HTTP/1.1") || status[1] != "200" {
        return None;
    }
    let mut length = None;
    let mut chunked = false;
    for line in lines {
        let (key, value) = line.split_once(':')?;
        if key.eq_ignore_ascii_case("content-length") {
            if length.is_some() {
                return None;
            }
            length = Some(value.trim().parse::<usize>().ok()?);
        } else if key.eq_ignore_ascii_case("transfer-encoding") {
            if !value.trim().eq_ignore_ascii_case("chunked") {
                return None;
            }
            chunked = true;
        }
    }
    let body = &response[split + 4..];
    if chunked {
        if length.is_some() {
            return None;
        }
        let mut decoded = Vec::new();
        let mut rest = body;
        loop {
            let line_end = rest.windows(2).position(|part| part == b"\r\n")?;
            let size_line = std::str::from_utf8(&rest[..line_end]).ok()?;
            let count = usize::from_str_radix(size_line.split(';').next()?.trim(), 16).ok()?;
            rest = &rest[line_end + 2..];
            if count == 0 {
                return rest.starts_with(b"\r\n").then_some(decoded);
            }
            if count > MAX_RESPONSE_BYTES - decoded.len()
                || rest.len() < count + 2
                || &rest[count..count + 2] != b"\r\n"
            {
                return None;
            }
            decoded.extend_from_slice(&rest[..count]);
            rest = &rest[count + 2..];
        }
    }
    if let Some(length) = length {
        if length > MAX_RESPONSE_BYTES || body.len() < length {
            return None;
        }
        return Some(body[..length].to_vec());
    }
    eof.then(|| body.to_vec())
}
