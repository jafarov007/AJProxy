use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

use crate::models::{HttpEntry, WsConnection};
use crate::proxy::store::{
    next_entry_id, next_ws_conn_id, push_captured_entry, push_ws_connection,
};
use super::tunnel::{run_plain_websocket_tunnel_loop, run_tls_websocket_tunnel_loop};

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

// ── TLS WebSocket Handshake Handler ──────────────────────────────────

pub fn handle_tls_websocket_tunnel(
    mut tls_stream: openssl::ssl::SslStream<TcpStream>,
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
        let _ = server_tcp.set_read_timeout(Some(Duration::from_secs(10)));
        let _ = server_tcp.set_write_timeout(Some(Duration::from_secs(10)));

        if let Ok(mut server_tls) = connector.connect(target_host, server_tcp) {
            let _ = server_tls.write_all(req_headers.as_bytes());
            let _ = server_tls.write_all(b"\r\n\r\n");
            if !req_body_bytes.is_empty() {
                let _ = server_tls.write_all(req_body_bytes);
            }
            let _ = server_tls.flush();

            let mut handshake_buf = [0u8; 4096];
            let mut resp_headers = String::new();
            if let Ok(n) = server_tls.read(&mut handshake_buf) {
                if n > 0 {
                    let _ = tls_stream.write_all(&handshake_buf[..n]);
                    let _ = tls_stream.flush();
                    resp_headers = String::from_utf8_lossy(&handshake_buf[..n]).to_string();
                }
            }

            let _ = server_tls.get_ref().set_read_timeout(Some(Duration::from_millis(200)));
            let _ = server_tls.get_ref().set_write_timeout(Some(Duration::from_millis(200)));
            let _ = tls_stream.get_ref().set_read_timeout(Some(Duration::from_millis(200)));
            let _ = tls_stream.get_ref().set_write_timeout(Some(Duration::from_millis(200)));

            let conn_id = next_ws_conn_id();

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
                response_headers: if resp_headers.is_empty() {
                    "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\n".to_string()
                } else {
                    resp_headers.clone()
                },
                response_body: format!("[WebSocket #{} Active Tunnel: {}]", conn_id, full_url),
            });

            push_ws_connection(WsConnection {
                id: conn_id,
                url: full_url.to_string(),
                host: target_host.to_string(),
                path: raw_path.to_string(),
                client_addr: "127.0.0.1".to_string(),
                connected_at: chrono::Local::now().format("%H:%M:%S").to_string(),
                status: "Active".to_string(),
                message_count: 0,
            });

            run_tls_websocket_tunnel_loop(conn_id, tls_stream, server_tls);
            return true;
        }
    }
    false
}

// ── Plain TCP WebSocket Handshake Handler ───────────────────────────

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
        let _ = server_tcp.set_read_timeout(Some(Duration::from_secs(10)));
        let _ = server_tcp.set_write_timeout(Some(Duration::from_secs(10)));

        let _ = server_tcp.write_all(req_headers.as_bytes());
        let _ = server_tcp.write_all(b"\r\n\r\n");
        if !req_body.is_empty() {
            let _ = server_tcp.write_all(req_body.as_bytes());
        }
        let _ = server_tcp.flush();

        let mut handshake_buf = [0u8; 4096];
        let mut resp_headers = String::new();
        if let Ok(n) = server_tcp.read(&mut handshake_buf) {
            if n > 0 {
                let _ = client_stream.write_all(&handshake_buf[..n]);
                let _ = client_stream.flush();
                resp_headers = String::from_utf8_lossy(&handshake_buf[..n]).to_string();
            }
        }

        let _ = server_tcp.set_read_timeout(Some(Duration::from_millis(200)));
        let _ = server_tcp.set_write_timeout(Some(Duration::from_millis(200)));
        let _ = client_stream.set_read_timeout(Some(Duration::from_millis(200)));
        let _ = client_stream.set_write_timeout(Some(Duration::from_millis(200)));

        let conn_id = next_ws_conn_id();

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
            response_headers: if resp_headers.is_empty() {
                "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\n".to_string()
            } else {
                resp_headers.clone()
            },
            response_body: format!("[WebSocket #{} Active Tunnel: {}]", conn_id, full_url),
        });

        push_ws_connection(WsConnection {
            id: conn_id,
            url: full_url.to_string(),
            host: host.to_string(),
            path: path.to_string(),
            client_addr: "127.0.0.1".to_string(),
            connected_at: chrono::Local::now().format("%H:%M:%S").to_string(),
            status: "Active".to_string(),
            message_count: 0,
        });

        run_plain_websocket_tunnel_loop(conn_id, client_stream, server_tcp);
        return true;
    }
    false
}
