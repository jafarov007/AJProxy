pub mod protocol;
pub mod intercept;
pub mod repeater_client;
pub mod reassembly;

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crate::models::{HttpEntry, WsConnection, WsDirection, WsFrameEntry, WsOpcode};
use crate::proxy::store::{
    next_entry_id, next_ws_conn_id, next_ws_frame_id, push_captured_entry,
    push_ws_connection, push_ws_frame, update_ws_conn_status,
};
use protocol::{build_pong_frame, parse_close_code, read_ws_frame, write_ws_frame};
use intercept::check_and_intercept_frame;
use reassembly::FrameReassembler;

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

fn format_ws_payload_preview(opcode_u8: u8, payload: &[u8]) -> String {
    if payload.is_empty() {
        return match opcode_u8 {
            0x9 => "[Ping]".to_string(),
            0xA => "[Pong]".to_string(),
            _ => String::new(),
        };
    }
    match std::str::from_utf8(payload) {
        Ok(s) => s.to_string(),
        Err(_) => {
            let hex_preview: Vec<String> = payload.iter().take(16).map(|b| format!("{:02X}", b)).collect();
            format!("[Binary {}B] {}", payload.len(), hex_preview.join(" "))
        }
    }
}

fn is_timeout_err(err: &std::io::Error) -> bool {
    if matches!(
        err.kind(),
        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
    ) {
        return true;
    }
    let msg = err.to_string().to_lowercase();
    msg.contains("would block")
        || msg.contains("wouldblock")
        || msg.contains("want read")
        || msg.contains("timed out")
        || msg.contains("timeout")
        || msg.contains("resource temporarily unavailable")
}

// ── TLS WebSocket Proxy Tunnel Handler ───────────────────────────────

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
        // Set normal 10s timeout during HTTP 101 Handshake so server response is never prematurely cut off
        let _ = server_tcp.set_read_timeout(Some(Duration::from_secs(10)));
        let _ = server_tcp.set_write_timeout(Some(Duration::from_secs(10)));

        if let Ok(mut server_tls) = connector.connect(target_host, server_tcp) {
            let _ = server_tls.write_all(req_headers.as_bytes());
            let _ = server_tls.write_all(b"\r\n\r\n");
            if !req_body_bytes.is_empty() {
                let _ = server_tls.write_all(req_body_bytes);
            }
            let _ = server_tls.flush();

            // Read target server HTTP 101 response and forward to browser client
            let mut handshake_buf = [0u8; 4096];
            let mut resp_headers = String::new();
            if let Ok(n) = server_tls.read(&mut handshake_buf) {
                if n > 0 {
                    let _ = tls_stream.write_all(&handshake_buf[..n]);
                    let _ = tls_stream.flush();
                    resp_headers = String::from_utf8_lossy(&handshake_buf[..n]).to_string();
                }
            }

            // Handshake complete! Set 200ms socket timeouts for full-duplex non-blocking frame proxying
            let _ = server_tls.get_ref().set_read_timeout(Some(Duration::from_millis(200)));
            let _ = server_tls.get_ref().set_write_timeout(Some(Duration::from_millis(200)));
            let _ = tls_stream.get_ref().set_read_timeout(Some(Duration::from_millis(200)));
            let _ = tls_stream.get_ref().set_write_timeout(Some(Duration::from_millis(200)));

            let conn_id = next_ws_conn_id();

            // Record HTTP 101 Switching Protocols
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

            // Register WS Connection
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

            let client_arc = Arc::new(Mutex::new(tls_stream));
            let server_arc = Arc::new(Mutex::new(server_tls));

            let c_clone = Arc::clone(&client_arc);
            let s_clone = Arc::clone(&server_arc);

            // Client -> Server thread
            let h1 = thread::spawn(move || {
                let mut reassembler = FrameReassembler::new();
                loop {
                    let frame_res = {
                        if let Ok(mut c) = c_clone.lock() {
                            read_ws_frame(&mut *c)
                        } else {
                            break;
                        }
                    };

                    let frame = match frame_res {
                        Ok(f) => f,
                        Err(ref e) if is_timeout_err(e) => {
                            thread::sleep(Duration::from_millis(5));
                            continue;
                        }
                        Err(_) => {
                            update_ws_conn_status(conn_id, "Closed (EOF)");
                            break;
                        }
                    };

                    // Ping frame handling
                    if frame.opcode_u8 == 0x9 {
                        push_ws_frame(WsFrameEntry {
                            id: next_ws_frame_id(),
                            connection_id: conn_id,
                            timestamp: chrono::Local::now().format("%H:%M:%S%.3f").to_string(),
                            direction: WsDirection::ClientToServer,
                            opcode: WsOpcode::Ping,
                            length: frame.payload.len(),
                            payload: format_ws_payload_preview(0x9, &frame.payload),
                            payload_bytes: frame.payload.clone(),
                            is_final: true,
                        });
                    }

                    // Pong frame handling
                    if frame.opcode_u8 == 0xA {
                        push_ws_frame(WsFrameEntry {
                            id: next_ws_frame_id(),
                            connection_id: conn_id,
                            timestamp: chrono::Local::now().format("%H:%M:%S%.3f").to_string(),
                            direction: WsDirection::ClientToServer,
                            opcode: WsOpcode::Pong,
                            length: frame.payload.len(),
                            payload: format_ws_payload_preview(0xA, &frame.payload),
                            payload_bytes: frame.payload.clone(),
                            is_final: true,
                        });
                    }

                    // Close frame check
                    if frame.opcode_u8 == 0x8 {
                        let (code, reason) = parse_close_code(&frame.payload);
                        update_ws_conn_status(conn_id, &format!("Closed ({})", code));
                        reassembler.clear_connection(conn_id);

                        push_ws_frame(WsFrameEntry {
                            id: next_ws_frame_id(),
                            connection_id: conn_id,
                            timestamp: chrono::Local::now().format("%H:%M:%S%.3f").to_string(),
                            direction: WsDirection::ClientToServer,
                            opcode: frame.to_opcode(),
                            length: frame.payload.len(),
                            payload: reason,
                            payload_bytes: frame.payload.clone(),
                            is_final: true,
                        });

                        if let Ok(mut s) = s_clone.lock() {
                            let _ = write_ws_frame(&mut *s, &frame);
                        }
                        break;
                    }

                    // Intercept check
                    let frame_to_send = match check_and_intercept_frame(conn_id, frame, WsDirection::ClientToServer) {
                        Some(f) => f,
                        None => continue, // Dropped
                    };

                    let payload_preview = format_ws_payload_preview(frame_to_send.opcode_u8, &frame_to_send.payload);

                    if let Some(reassembled) = reassembler.process_frame(conn_id, frame_to_send.clone()) {
                        if reassembled.opcode_u8 != 0x9 && reassembled.opcode_u8 != 0xA && reassembled.opcode_u8 != 0x8 {
                            push_ws_frame(WsFrameEntry {
                                id: next_ws_frame_id(),
                                connection_id: conn_id,
                                timestamp: chrono::Local::now().format("%H:%M:%S%.3f").to_string(),
                                direction: WsDirection::ClientToServer,
                                opcode: reassembled.to_opcode(),
                                length: reassembled.payload.len(),
                                payload: payload_preview,
                                payload_bytes: reassembled.payload,
                                is_final: reassembled.fin,
                            });
                        }
                    }

                    if let Ok(mut s) = s_clone.lock() {
                        if write_ws_frame(&mut *s, &frame_to_send).is_err() {
                            break;
                        }
                    } else {
                        break;
                    }
                }
            });

            // Server -> Client thread
            let mut reassembler = FrameReassembler::new();
            loop {
                let frame_res = {
                    if let Ok(mut s) = server_arc.lock() {
                        read_ws_frame(&mut *s)
                    } else {
                        break;
                    }
                };

                let frame = match frame_res {
                    Ok(f) => f,
                    Err(ref e) if is_timeout_err(e) => {
                        thread::sleep(Duration::from_millis(5));
                        continue;
                    }
                    Err(_) => {
                        update_ws_conn_status(conn_id, "Closed (EOF)");
                        break;
                    }
                };

                // Auto Ping -> Pong response
                if frame.opcode_u8 == 0x9 {
                    push_ws_frame(WsFrameEntry {
                        id: next_ws_frame_id(),
                        connection_id: conn_id,
                        timestamp: chrono::Local::now().format("%H:%M:%S%.3f").to_string(),
                        direction: WsDirection::ServerToClient,
                        opcode: WsOpcode::Ping,
                        length: frame.payload.len(),
                        payload: format_ws_payload_preview(0x9, &frame.payload),
                        payload_bytes: frame.payload.clone(),
                        is_final: true,
                    });

                    let pong = build_pong_frame(&frame);
                    if let Ok(mut s) = server_arc.lock() {
                        let _ = write_ws_frame(&mut *s, &pong);
                    }

                    push_ws_frame(WsFrameEntry {
                        id: next_ws_frame_id(),
                        connection_id: conn_id,
                        timestamp: chrono::Local::now().format("%H:%M:%S%.3f").to_string(),
                        direction: WsDirection::ClientToServer,
                        opcode: WsOpcode::Pong,
                        length: pong.payload.len(),
                        payload: format_ws_payload_preview(0xA, &pong.payload),
                        payload_bytes: pong.payload.clone(),
                        is_final: true,
                    });
                }

                // Pong frame check
                if frame.opcode_u8 == 0xA {
                    push_ws_frame(WsFrameEntry {
                        id: next_ws_frame_id(),
                        connection_id: conn_id,
                        timestamp: chrono::Local::now().format("%H:%M:%S%.3f").to_string(),
                        direction: WsDirection::ServerToClient,
                        opcode: WsOpcode::Pong,
                        length: frame.payload.len(),
                        payload: format_ws_payload_preview(0xA, &frame.payload),
                        payload_bytes: frame.payload.clone(),
                        is_final: true,
                    });
                }

                // Close frame check
                if frame.opcode_u8 == 0x8 {
                    let (code, reason) = parse_close_code(&frame.payload);
                    update_ws_conn_status(conn_id, &format!("Closed ({})", code));

                    push_ws_frame(WsFrameEntry {
                        id: next_ws_frame_id(),
                        connection_id: conn_id,
                        timestamp: chrono::Local::now().format("%H:%M:%S%.3f").to_string(),
                        direction: WsDirection::ServerToClient,
                        opcode: frame.to_opcode(),
                        length: frame.payload.len(),
                        payload: reason,
                        payload_bytes: frame.payload.clone(),
                        is_final: true,
                    });

                    if let Ok(mut c) = client_arc.lock() {
                        let _ = write_ws_frame(&mut *c, &frame);
                    }
                    break;
                }

                // Intercept check
                let frame_to_send = match check_and_intercept_frame(conn_id, frame, WsDirection::ServerToClient) {
                    Some(f) => f,
                    None => continue, // Dropped
                };

                let payload_preview = format_ws_payload_preview(frame_to_send.opcode_u8, &frame_to_send.payload);

                if let Some(reassembled) = reassembler.process_frame(conn_id, frame_to_send.clone()) {
                    if reassembled.opcode_u8 != 0x9 && reassembled.opcode_u8 != 0xA && reassembled.opcode_u8 != 0x8 {
                        push_ws_frame(WsFrameEntry {
                            id: next_ws_frame_id(),
                            connection_id: conn_id,
                            timestamp: chrono::Local::now().format("%H:%M:%S%.3f").to_string(),
                            direction: WsDirection::ServerToClient,
                            opcode: reassembled.to_opcode(),
                            length: reassembled.payload.len(),
                            payload: payload_preview,
                            payload_bytes: reassembled.payload,
                            is_final: reassembled.fin,
                        });
                    }
                }

                if let Ok(mut c) = client_arc.lock() {
                    if write_ws_frame(&mut *c, &frame_to_send).is_err() {
                        break;
                    }
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

// ── Plain TCP WebSocket Proxy Tunnel Handler ─────────────────────────

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

        let mut client_clone = match client_stream.try_clone() {
            Ok(c) => c,
            Err(_) => return false,
        };
        let mut server_clone = match server_tcp.try_clone() {
            Ok(s) => s,
            Err(_) => return false,
        };

        let h1 = thread::spawn(move || {
            let mut reassembler = FrameReassembler::new();
            loop {
                let frame_res = read_ws_frame(&mut client_clone);
                let frame = match frame_res {
                    Ok(f) => f,
                    Err(ref e) if is_timeout_err(e) => {
                        thread::sleep(Duration::from_millis(5));
                        continue;
                    }
                    Err(_) => {
                        update_ws_conn_status(conn_id, "Closed (EOF)");
                        break;
                    }
                };

                // Ping / Pong frame check
                if frame.opcode_u8 == 0x9 || frame.opcode_u8 == 0xA {
                    push_ws_frame(WsFrameEntry {
                        id: next_ws_frame_id(),
                        connection_id: conn_id,
                        timestamp: chrono::Local::now().format("%H:%M:%S%.3f").to_string(),
                        direction: WsDirection::ClientToServer,
                        opcode: frame.to_opcode(),
                        length: frame.payload.len(),
                        payload: format_ws_payload_preview(frame.opcode_u8, &frame.payload),
                        payload_bytes: frame.payload.clone(),
                        is_final: true,
                    });
                }

                if frame.opcode_u8 == 0x8 {
                    let (code, reason) = parse_close_code(&frame.payload);
                    update_ws_conn_status(conn_id, &format!("Closed ({})", code));
                    push_ws_frame(WsFrameEntry {
                        id: next_ws_frame_id(),
                        connection_id: conn_id,
                        timestamp: chrono::Local::now().format("%H:%M:%S%.3f").to_string(),
                        direction: WsDirection::ClientToServer,
                        opcode: frame.to_opcode(),
                        length: reason.len(),
                        payload: reason,
                        payload_bytes: frame.payload.clone(),
                        is_final: true,
                    });
                    let _ = write_ws_frame(&mut server_clone, &frame);
                    break;
                }

                let frame_to_send = match check_and_intercept_frame(conn_id, frame, WsDirection::ClientToServer) {
                    Some(f) => f,
                    None => continue,
                };

                let payload_preview = format_ws_payload_preview(frame_to_send.opcode_u8, &frame_to_send.payload);

                if let Some(reassembled) = reassembler.process_frame(conn_id, frame_to_send.clone()) {
                    if reassembled.opcode_u8 != 0x9 && reassembled.opcode_u8 != 0xA && reassembled.opcode_u8 != 0x8 {
                        push_ws_frame(WsFrameEntry {
                            id: next_ws_frame_id(),
                            connection_id: conn_id,
                            timestamp: chrono::Local::now().format("%H:%M:%S%.3f").to_string(),
                            direction: WsDirection::ClientToServer,
                            opcode: reassembled.to_opcode(),
                            length: reassembled.payload.len(),
                            payload: payload_preview,
                            payload_bytes: reassembled.payload,
                            is_final: reassembled.fin,
                        });
                    }
                }

                if write_ws_frame(&mut server_clone, &frame_to_send).is_err() {
                    break;
                }
            }
        });

        let mut reassembler = FrameReassembler::new();
        loop {
            let frame_res = read_ws_frame(&mut server_tcp);
            let frame = match frame_res {
                Ok(f) => f,
                Err(ref e) if is_timeout_err(e) => {
                    thread::sleep(Duration::from_millis(5));
                    continue;
                }
                Err(_) => {
                    update_ws_conn_status(conn_id, "Closed (EOF)");
                    break;
                }
            };

            if frame.opcode_u8 == 0x9 {
                push_ws_frame(WsFrameEntry {
                    id: next_ws_frame_id(),
                    connection_id: conn_id,
                    timestamp: chrono::Local::now().format("%H:%M:%S%.3f").to_string(),
                    direction: WsDirection::ServerToClient,
                    opcode: WsOpcode::Ping,
                    length: frame.payload.len(),
                    payload: format_ws_payload_preview(0x9, &frame.payload),
                    payload_bytes: frame.payload.clone(),
                    is_final: true,
                });

                let pong = build_pong_frame(&frame);
                let _ = write_ws_frame(client_stream, &pong);

                push_ws_frame(WsFrameEntry {
                    id: next_ws_frame_id(),
                    connection_id: conn_id,
                    timestamp: chrono::Local::now().format("%H:%M:%S%.3f").to_string(),
                    direction: WsDirection::ClientToServer,
                    opcode: WsOpcode::Pong,
                    length: pong.payload.len(),
                    payload: format_ws_payload_preview(0xA, &pong.payload),
                    payload_bytes: pong.payload.clone(),
                    is_final: true,
                });
            }

            if frame.opcode_u8 == 0x8 {
                let (code, reason) = parse_close_code(&frame.payload);
                update_ws_conn_status(conn_id, &format!("Closed ({})", code));
                push_ws_frame(WsFrameEntry {
                    id: next_ws_frame_id(),
                    connection_id: conn_id,
                    timestamp: chrono::Local::now().format("%H:%M:%S%.3f").to_string(),
                    direction: WsDirection::ServerToClient,
                    opcode: frame.to_opcode(),
                    length: reason.len(),
                    payload: reason,
                    payload_bytes: frame.payload.clone(),
                    is_final: true,
                });
                let _ = write_ws_frame(client_stream, &frame);
                break;
            }

            let frame_to_send = match check_and_intercept_frame(conn_id, frame, WsDirection::ServerToClient) {
                Some(f) => f,
                None => continue,
            };

            let payload_preview = format_ws_payload_preview(frame_to_send.opcode_u8, &frame_to_send.payload);

            if let Some(reassembled) = reassembler.process_frame(conn_id, frame_to_send.clone()) {
                if reassembled.opcode_u8 != 0x9 && reassembled.opcode_u8 != 0xA && reassembled.opcode_u8 != 0x8 {
                    push_ws_frame(WsFrameEntry {
                        id: next_ws_frame_id(),
                        connection_id: conn_id,
                        timestamp: chrono::Local::now().format("%H:%M:%S%.3f").to_string(),
                        direction: WsDirection::ServerToClient,
                        opcode: reassembled.to_opcode(),
                        length: reassembled.payload.len(),
                        payload: payload_preview,
                        payload_bytes: reassembled.payload,
                        is_final: reassembled.fin,
                    });
                }
            }

            if write_ws_frame(client_stream, &frame_to_send).is_err() {
                break;
            }
        }

        let _ = h1.join();
        return true;
    }
    false
}
