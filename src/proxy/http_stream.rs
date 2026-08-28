use std::io::{Read, Write};
use std::time::Instant;
use crate::models::HttpEntry;
use crate::proxy::store::{next_entry_id, push_captured_entry};

/// Reads an entire HTTP request from a stream (Headers + Content-Length or Chunked Body)
pub fn read_full_http_request<R: Read>(reader: &mut R) -> Result<(String, Vec<u8>), std::io::Error> {
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 4096];
    let mut header_end_pos = None;

    // 1. Read until we find "\r\n\r\n"
    loop {
        let n = reader.read(&mut chunk)?;
        if n == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..n]);
        if let Some(pos) = buffer.windows(4).position(|w| w == b"\r\n\r\n") {
            header_end_pos = Some(pos);
            break;
        }
    }

    let pos = match header_end_pos {
        Some(p) => p,
        None => {
            let headers = String::from_utf8_lossy(&buffer).to_string();
            return Ok((headers, Vec::new()));
        }
    };

    let headers_str = String::from_utf8_lossy(&buffer[..pos]).to_string();
    let mut body_bytes = buffer[pos + 4..].to_vec();

    // 2. Parse Content-Length and Transfer-Encoding
    let mut content_length = 0;
    let mut is_chunked = false;
    for line in headers_str.lines() {
        if let Some((k, v)) = line.split_once(':') {
            let k = k.trim();
            if k.eq_ignore_ascii_case("content-length") {
                if let Ok(len) = v.trim().parse::<usize>() {
                    content_length = len;
                }
            } else if k.eq_ignore_ascii_case("transfer-encoding") && v.trim().to_lowercase().contains("chunked") {
                is_chunked = true;
            }
        }
    }

    // 3. Read the rest of the body if needed
    if is_chunked {
        while !body_bytes.windows(5).any(|w| w == b"0\r\n\r\n") {
            let mut b = [0u8; 1024];
            let n = reader.read(&mut b)?;
            if n == 0 {
                break;
            }
            body_bytes.extend_from_slice(&b[..n]);
        }
    } else if body_bytes.len() < content_length {
        let mut remaining = content_length - body_bytes.len();
        let mut body_chunk = vec![0u8; remaining.min(4096)];
        while remaining > 0 {
            let n = reader.read(&mut body_chunk)?;
            if n == 0 {
                break;
            }
            body_bytes.extend_from_slice(&body_chunk[..n]);
            remaining -= n;
            if remaining > 0 && body_chunk.len() > remaining {
                body_chunk.resize(remaining, 0);
            }
        }
    }

    Ok((headers_str, body_bytes))
}

/// Helper function to process response and forward to client with SSE & Streaming support
pub fn process_and_send_response<W: Write>(
    stream: &mut W,
    resp: ureq::Response,
    method: &str,
    target_host: &str,
    raw_path: &str,
    full_url: &str,
    req_headers: &str,
    req_body: &str,
    start_time: Instant,
) {
    let status = resp.status();
    let content_type = resp.header("Content-Type").unwrap_or("text/html").to_string();

    let status_str = match status {
        200 => "OK",
        201 => "Created",
        202 => "Accepted",
        204 => "No Content",
        206 => "Partial Content",
        301 => "Moved Permanently",
        302 => "Found",
        303 => "See Other",
        304 => "Not Modified",
        307 => "Temporary Redirect",
        308 => "Permanent Redirect",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        409 => "Conflict",
        500 => "Internal Server Error",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        504 => "Gateway Timeout",
        _ => "OK",
    };

    let is_sse = content_type.to_lowercase().contains("text/event-stream")
        || content_type.to_lowercase().contains("application/stream+json")
        || content_type.to_lowercase().contains("application/x-ndjson");

    let is_chunked_upstream = resp.header("transfer-encoding").map(|v| v.to_lowercase().contains("chunked")).unwrap_or(false);
    let is_large_content = resp.header("content-length").and_then(|v| v.parse::<usize>().ok()).unwrap_or(0) > 5 * 1024 * 1024;

    // Collect all response headers for logging
    let mut resp_headers_str = format!("HTTP/1.1 {} {}\r\n", status, status_str);
    let mut forwarded_headers = String::new();
    for h_name in resp.headers_names() {
        if let Some(h_val) = resp.header(&h_name) {
            resp_headers_str.push_str(&format!("{}: {}\r\n", h_name, h_val));
            if !h_name.eq_ignore_ascii_case("content-length")
                && !h_name.eq_ignore_ascii_case("transfer-encoding")
                && !h_name.eq_ignore_ascii_case("content-encoding")
            {
                forwarded_headers.push_str(&format!("{}: {}\r\n", h_name, h_val));
            }
        }
    }

    // ── REAL-TIME STREAMING MODE (SSE / AI LLM Chat / Large Media Downloads) ──
    if is_sse || (is_chunked_upstream && is_large_content) {
        let mut http_resp = format!("HTTP/1.1 {} {}\r\n", status, status_str);
        http_resp.push_str(&forwarded_headers);
        if is_sse {
            http_resp.push_str("Cache-Control: no-cache\r\nConnection: keep-alive\r\n");
        } else if is_chunked_upstream {
            http_resp.push_str("Transfer-Encoding: chunked\r\n");
        }
        http_resp.push_str("\r\n");

        let _ = stream.write_all(http_resp.as_bytes());
        let _ = stream.flush();

        let mut reader = resp.into_reader();
        let mut buf = [0u8; 8192];
        let mut captured_bytes = Vec::new();

        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    if is_chunked_upstream && !is_sse {
                        let chunk_header = format!("{:x}\r\n", n);
                        if stream.write_all(chunk_header.as_bytes()).is_err() { break; }
                        if stream.write_all(&buf[..n]).is_err() { break; }
                        if stream.write_all(b"\r\n").is_err() { break; }
                    } else {
                        if stream.write_all(&buf[..n]).is_err() { break; }
                    }
                    let _ = stream.flush();

                    if captured_bytes.len() < 16384 {
                        let take = n.min(16384 - captured_bytes.len());
                        captured_bytes.extend_from_slice(&buf[..take]);
                    }
                }
                Err(_) => break,
            }
        }

        if is_chunked_upstream && !is_sse {
            let _ = stream.write_all(b"0\r\n\r\n");
            let _ = stream.flush();
        }

        let parsed_url = url::Url::parse(full_url).ok();
        let host = parsed_url.as_ref().and_then(|u| u.host_str()).unwrap_or(target_host).to_string();
        let path = parsed_url.as_ref().map(|u| u.path()).unwrap_or(raw_path).to_string();

        push_captured_entry(HttpEntry {
            id: next_entry_id(),
            timestamp: chrono::Local::now().format("%H:%M:%S%.3f").to_string(),
            method: method.to_string(),
            host,
            path,
            url: full_url.to_string(),
            status_code: status,
            content_type: content_type.clone(),
            length: captured_bytes.len(),
            duration_ms: start_time.elapsed().as_millis() as u64,
            protocol: "HTTP/1.1".to_string(),
            request_headers: req_headers.to_string(),
            request_body: req_body.to_string(),
            response_headers: resp_headers_str,
            response_body: String::from_utf8_lossy(&captured_bytes).to_string(),
        });
        return;
    }

    // Standard Response Mode
    let mut body_bytes = Vec::new();
    let _ = resp.into_reader().read_to_end(&mut body_bytes);

    let mut http_resp = format!("HTTP/1.1 {} {}\r\n", status, status_str);
    http_resp.push_str(&forwarded_headers);
    http_resp.push_str(&format!("Content-Length: {}\r\n", body_bytes.len()));
    http_resp.push_str("\r\n");

    let _ = stream.write_all(http_resp.as_bytes());
    let _ = stream.write_all(&body_bytes);
    let _ = stream.flush();

    let parsed_url = url::Url::parse(full_url).ok();
    let host = parsed_url.as_ref().and_then(|u| u.host_str()).unwrap_or(target_host).to_string();
    let path = parsed_url.as_ref().map(|u| u.path()).unwrap_or(raw_path).to_string();

    push_captured_entry(HttpEntry {
        id: next_entry_id(),
        timestamp: chrono::Local::now().format("%H:%M:%S%.3f").to_string(),
        method: method.to_string(),
        host,
        path,
        url: full_url.to_string(),
        status_code: status,
        content_type: content_type.clone(),
        length: body_bytes.len(),
        duration_ms: start_time.elapsed().as_millis() as u64,
        protocol: "HTTP/1.1".to_string(),
        request_headers: req_headers.to_string(),
        request_body: req_body.to_string(),
        response_headers: resp_headers_str,
        response_body: String::from_utf8_lossy(&body_bytes[..body_bytes.len().min(16384)]).to_string(),
    });
}
