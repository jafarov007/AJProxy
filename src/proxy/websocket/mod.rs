pub mod protocol;
pub mod intercept;
pub mod repeater_client;
pub mod reassembly;

use std::io::Write;
use std::net::TcpStream;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;

use crate::models::{HttpEntry, WsConnection, WsDirection, WsFrameEntry};
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

// ── TLS WebSocket Proxy Tunnel Handler ───────────────────────────────

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
                response_headers: "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n".to_string(),
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

                    match frame_res {
                        Ok(frame) => {
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

                            let payload_text = String::from_utf8_lossy(&frame_to_send.payload).to_string();

                            if let Some(reassembled) = reassembler.process_frame(conn_id, frame_to_send.clone()) {
                                push_ws_frame(WsFrameEntry {
                                    id: next_ws_frame_id(),
                                    connection_id: conn_id,
                                    timestamp: chrono::Local::now().format("%H:%M:%S%.3f").to_string(),
                                    direction: WsDirection::ClientToServer,
                                    opcode: reassembled.to_opcode(),
                                    length: reassembled.payload.len(),
                                    payload: payload_text,
                                    payload_bytes: reassembled.payload,
                                    is_final: reassembled.fin,
                                });
                            }

                            if let Ok(mut s) = s_clone.lock() {
                                if write_ws_frame(&mut *s, &frame_to_send).is_err() {
                                    break;
                                }
                            } else {
                                break;
                            }
                        }
                        Err(_) => {
                            update_ws_conn_status(conn_id, "Closed (EOF)");
                            break;
                        }
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

                match frame_res {
                    Ok(frame) => {
                        // Auto Ping -> Pong response
                        if frame.opcode_u8 == 0x9 {
                            let pong = build_pong_frame(&frame);
                            if let Ok(mut s) = server_arc.lock() {
                                let _ = write_ws_frame(&mut *s, &pong);
                            }
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

                        let payload_text = String::from_utf8_lossy(&frame_to_send.payload).to_string();

                        if let Some(reassembled) = reassembler.process_frame(conn_id, frame_to_send.clone()) {
                            push_ws_frame(WsFrameEntry {
                                id: next_ws_frame_id(),
                                connection_id: conn_id,
                                timestamp: chrono::Local::now().format("%H:%M:%S%.3f").to_string(),
                                direction: WsDirection::ServerToClient,
                                opcode: reassembled.to_opcode(),
                                length: reassembled.payload.len(),
                                payload: payload_text,
                                payload_bytes: reassembled.payload,
                                is_final: reassembled.fin,
                            });
                        }

                        if let Ok(mut c) = client_arc.lock() {
                            if write_ws_frame(&mut *c, &frame_to_send).is_err() {
                                break;
                            }
                        } else {
                            break;
                        }
                    }
                    Err(_) => {
                        update_ws_conn_status(conn_id, "Closed (EOF)");
                        break;
                    }
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
        let _ = server_tcp.write_all(req_headers.as_bytes());
        let _ = server_tcp.write_all(b"\r\n\r\n");
        if !req_body.is_empty() {
            let _ = server_tcp.write_all(req_body.as_bytes());
        }
        let _ = server_tcp.flush();

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
            response_headers: "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n".to_string(),
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
                match read_ws_frame(&mut client_clone) {
                    Ok(frame) => {
                        if frame.opcode_u8 == 0x8 {
                            let (code, reason) = parse_close_code(&frame.payload);
                            update_ws_conn_status(conn_id, &format!("Closed ({})", code));
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
                            let _ = write_ws_frame(&mut server_clone, &frame);
                            break;
                        }

                        let frame_to_send = match check_and_intercept_frame(conn_id, frame, WsDirection::ClientToServer) {
                            Some(f) => f,
                            None => continue,
                        };

                        let payload_text = String::from_utf8_lossy(&frame_to_send.payload).to_string();

                        if let Some(reassembled) = reassembler.process_frame(conn_id, frame_to_send.clone()) {
                            push_ws_frame(WsFrameEntry {
                                id: next_ws_frame_id(),
                                connection_id: conn_id,
                                timestamp: chrono::Local::now().format("%H:%M:%S%.3f").to_string(),
                                direction: WsDirection::ClientToServer,
                                opcode: reassembled.to_opcode(),
                                length: reassembled.payload.len(),
                                payload: payload_text,
                                payload_bytes: reassembled.payload,
                                is_final: reassembled.fin,
                            });
                        }

                        if write_ws_frame(&mut server_clone, &frame_to_send).is_err() {
                            break;
                        }
                    }
                    Err(_) => {
                        update_ws_conn_status(conn_id, "Closed (EOF)");
                        break;
                    }
                }
            }
        });

        let mut reassembler = FrameReassembler::new();
        loop {
            match read_ws_frame(&mut server_tcp) {
                Ok(frame) => {
                    if frame.opcode_u8 == 0x9 {
                        let pong = build_pong_frame(&frame);
                        let _ = write_ws_frame(&mut server_tcp, &pong);
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
                            length: frame.payload.len(),
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

                    let payload_text = String::from_utf8_lossy(&frame_to_send.payload).to_string();

                    if let Some(reassembled) = reassembler.process_frame(conn_id, frame_to_send.clone()) {
                        push_ws_frame(WsFrameEntry {
                            id: next_ws_frame_id(),
                            connection_id: conn_id,
                            timestamp: chrono::Local::now().format("%H:%M:%S%.3f").to_string(),
                            direction: WsDirection::ServerToClient,
                            opcode: reassembled.to_opcode(),
                            length: reassembled.payload.len(),
                            payload: payload_text,
                            payload_bytes: reassembled.payload,
                            is_final: reassembled.fin,
                        });
                    }

                    if write_ws_frame(client_stream, &frame_to_send).is_err() {
                        break;
                    }
                }
                Err(_) => {
                    update_ws_conn_status(conn_id, "Closed (EOF)");
                    break;
                }
            }
        }

        let _ = h1.join();
        return true;
    }
    false
}
