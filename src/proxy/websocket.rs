use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;
use crate::models::HttpEntry;
use crate::proxy::store::{next_entry_id, push_captured_entry};

/// Helper to detect if HTTP request headers contain WebSocket Upgrade request
pub fn is_websocket_upgrade(req_headers: &str) -> bool {
    req_headers.lines().any(|l| {
        if let Some((k, v)) = l.split_once(':') {
            let k = k.trim().to_lowercase();
            let v = v.trim().to_lowercase();
            (k == "upgrade" && v.contains("websocket")) || (k == "connection" && v.contains("upgrade"))
        } else {
            false
        }
    })
}

/// Handles HTTPS WebSocket Upgrade by establishing a direct TLS stream and copying data bidirectionally
pub fn handle_tls_websocket_tunnel(
    tls_stream: openssl::ssl::SslStream<TcpStream>,
    target_host: &str,
    raw_path: &str,
    full_url: &str,
    method: &str,
    req_headers: &str,
    req_body: &str,
    req_body_bytes: &[u8],
    request_start: Instant,
) -> bool {
    let mut connector_builder = match openssl::ssl::SslConnector::builder(openssl::ssl::SslMethod::tls()) {
        Ok(b) => b,
        Err(_) => return false,
    };
    connector_builder.set_verify(openssl::ssl::SslVerifyMode::NONE);
    let connector = connector_builder.build();

    let target_addr = if target_host.contains(':') {
        target_host.to_string()
    } else {
        format!("{}:443", target_host)
    };

    if let Ok(server_tcp) = TcpStream::connect(&target_addr) {
        if let Ok(mut server_tls) = connector.connect(target_host, server_tcp) {
            let _ = server_tls.write_all(req_headers.as_bytes());
            let _ = server_tls.write_all(b"\r\n\r\n");
            if !req_body_bytes.is_empty() {
                let _ = server_tls.write_all(req_body_bytes);
            }
            let _ = server_tls.flush();

            push_captured_entry(HttpEntry {
                id: next_entry_id(),
                timestamp: chrono::Local::now().format("%H:%M:%S%.3f").to_string(),
                method: method.to_string(),
                host: target_host.to_string(),
                path: raw_path.to_string(),
                url: full_url.to_string(),
                status_code: 101,
                content_type: "websocket".to_string(),
                length: 0,
                duration_ms: request_start.elapsed().as_millis() as u64,
                protocol: "HTTP/1.1".to_string(),
                request_headers: req_headers.to_string(),
                request_body: req_body.to_string(),
                response_headers: "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n".to_string(),
                response_body: "[WebSocket Bidirectional Tunnel Active]".to_string(),
            });

            let client_arc = Arc::new(Mutex::new(tls_stream));
            let server_arc = Arc::new(Mutex::new(server_tls));

            let c_clone = Arc::clone(&client_arc);
            let s_clone = Arc::clone(&server_arc);

            let h1 = thread::spawn(move || {
                let mut buf = [0u8; 8192];
                loop {
                    let n = match c_clone.lock() {
                        Ok(mut c) => match c.read(&mut buf) {
                            Ok(0) => break,
                            Ok(n) => n,
                            Err(_) => break,
                        },
                        Err(_) => break,
                    };
                    if let Ok(mut s) = s_clone.lock() {
                        if s.write_all(&buf[..n]).is_err() { break; }
                        let _ = s.flush();
                    } else {
                        break;
                    }
                }
            });

            let mut buf = [0u8; 8192];
            loop {
                let n = match server_arc.lock() {
                    Ok(mut s) => match s.read(&mut buf) {
                        Ok(0) => break,
                        Ok(n) => n,
                        Err(_) => break,
                    },
                    Err(_) => break,
                };
                if let Ok(mut c) = client_arc.lock() {
                    if c.write_all(&buf[..n]).is_err() { break; }
                    let _ = c.flush();
                } else {
                    break;
                }
            }

            let _ = h1.join();
            return true;
        }
    }
    false
}

/// Handles Plain HTTP WebSocket Upgrade by establishing a direct TCP stream and copying data bidirectionally
pub fn handle_plain_websocket_tunnel(
    client_stream: &mut TcpStream,
    host: &str,
    path: &str,
    full_url: &str,
    method: &str,
    req_headers: &str,
    req_body: &str,
    start_time: Instant,
) -> bool {
    let target_addr = if host.contains(':') { host.to_string() } else { format!("{}:80", host) };
    if let Ok(mut server_tcp) = TcpStream::connect(&target_addr) {
        let _ = server_tcp.write_all(req_headers.as_bytes());
        let _ = server_tcp.write_all(b"\r\n\r\n");
        if !req_body.is_empty() {
            let _ = server_tcp.write_all(req_body.as_bytes());
        }
        let _ = server_tcp.flush();

        push_captured_entry(HttpEntry {
            id: next_entry_id(),
            timestamp: chrono::Local::now().format("%H:%M:%S%.3f").to_string(),
            method: method.to_string(),
            host: host.to_string(),
            path: path.to_string(),
            url: full_url.to_string(),
            status_code: 101,
            content_type: "websocket".to_string(),
            length: 0,
            duration_ms: start_time.elapsed().as_millis() as u64,
            protocol: "HTTP/1.1".to_string(),
            request_headers: req_headers.to_string(),
            request_body: req_body.to_string(),
            response_headers: "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n".to_string(),
            response_body: "[WebSocket Bidirectional Tunnel Active]".to_string(),
        });

        let mut client_clone = match client_stream.try_clone() {
            Ok(c) => c,
            Err(_) => return false,
        };
        let mut server_clone = match server_tcp.try_clone() {
            Ok(s) => s,
            Err(_) => return false,
        };

        thread::spawn(move || {
            let _ = std::io::copy(&mut client_clone, &mut server_clone);
        });
        let _ = std::io::copy(&mut server_tcp, client_stream);
        return true;
    }
    false
}
