use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;
use crate::models::{HttpEntry, WsConnection, WsFrameEntry, WsOpcode, WsDirection};
use crate::proxy::store::{next_entry_id, next_ws_conn_id, next_ws_frame_id, push_captured_entry, push_ws_connection, push_ws_frame};

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

// ── RFC 6455 WebSocket Binary Frame Structure & Parser ───────────────

#[derive(Clone, Debug)]
pub struct WsRawFrame {
    pub fin: bool,
    pub opcode_u8: u8,
    pub masked: bool,
    pub mask_key: Option<[u8; 4]>,
    pub payload: Vec<u8>,
}

impl WsRawFrame {
    pub fn to_opcode(&self) -> WsOpcode {
        match self.opcode_u8 {
            0x1 => WsOpcode::Text,
            0x2 => WsOpcode::Binary,
            0x8 => WsOpcode::Close,
            0x9 => WsOpcode::Ping,
            0xA => WsOpcode::Pong,
            0x0 => WsOpcode::Continuation,
            other => WsOpcode::Unknown(other),
        }
    }
}

/// Parses a single RFC 6455 frame from any std::io::Read stream
pub fn read_ws_frame<R: Read>(reader: &mut R) -> std::io::Result<WsRawFrame> {
    let mut header = [0u8; 2];
    reader.read_exact(&mut header)?;

    let fin = (header[0] & 0x80) != 0;
    let opcode_u8 = header[0] & 0x0F;
    let masked = (header[1] & 0x80) != 0;
    let mut payload_len = (header[1] & 0x7F) as u64;

    if payload_len == 126 {
        let mut ext = [0u8; 2];
        reader.read_exact(&mut ext)?;
        payload_len = u16::from_be_bytes(ext) as u64;
    } else if payload_len == 127 {
        let mut ext = [0u8; 8];
        reader.read_exact(&mut ext)?;
        payload_len = u64::from_be_bytes(ext);
    }

    let mask_key = if masked {
        let mut key = [0u8; 4];
        reader.read_exact(&mut key)?;
        Some(key)
    } else {
        None
    };

    let mut payload = vec![0u8; payload_len as usize];
    if payload_len > 0 {
        reader.read_exact(&mut payload)?;
    }

    // Unmask if client frame
    if let Some(key) = mask_key {
        for (i, byte) in payload.iter_mut().enumerate() {
            *byte ^= key[i % 4];
        }
    }

    Ok(WsRawFrame {
        fin,
        opcode_u8,
        masked,
        mask_key,
        payload,
    })
}

/// Serializes and writes a single RFC 6455 frame to any std::io::Write stream
pub fn write_ws_frame<W: Write>(writer: &mut W, frame: &WsRawFrame) -> std::io::Result<()> {
    let mut header = Vec::with_capacity(14);
    let mut byte0 = frame.opcode_u8 & 0x0F;
    if frame.fin {
        byte0 |= 0x80;
    }
    header.push(byte0);

    let len = frame.payload.len();
    let mask_bit = if frame.masked { 0x80 } else { 0x00 };

    if len <= 125 {
        header.push(mask_bit | (len as u8));
    } else if len <= 65535 {
        header.push(mask_bit | 126);
        header.extend_from_slice(&(len as u16).to_be_bytes());
    } else {
        header.push(mask_bit | 127);
        header.extend_from_slice(&(len as u64).to_be_bytes());
    }

    if let Some(key) = frame.mask_key {
        header.extend_from_slice(&key);
        writer.write_all(&header)?;
        let mut masked_payload = frame.payload.clone();
        for (i, byte) in masked_payload.iter_mut().enumerate() {
            *byte ^= key[i % 4];
        }
        writer.write_all(&masked_payload)?;
    } else {
        writer.write_all(&header)?;
        if !frame.payload.is_empty() {
            writer.write_all(&frame.payload)?;
        }
    }

    writer.flush()
}

// ── TLS & Plain WebSocket Proxy Tunnel Handlers ──────────────────────

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

            // Record HTTP 101 Switching Protocols in Traffic history
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

            // Client -> Server thread with RFC 6455 parsing
            let h1 = thread::spawn(move || {
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
                            let payload_text = String::from_utf8_lossy(&frame.payload).to_string();
                            push_ws_frame(WsFrameEntry {
                                id: next_ws_frame_id(),
                                connection_id: conn_id,
                                timestamp: chrono::Local::now().format("%H:%M:%S%.3f").to_string(),
                                direction: WsDirection::ClientToServer,
                                opcode: frame.to_opcode(),
                                length: frame.payload.len(),
                                payload: payload_text,
                                payload_bytes: frame.payload.clone(),
                                is_final: frame.fin,
                            });

                            if let Ok(mut s) = s_clone.lock() {
                                if write_ws_frame(&mut *s, &frame).is_err() {
                                    break;
                                }
                            } else {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
            });

            // Server -> Client thread with RFC 6455 parsing
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
                        let payload_text = String::from_utf8_lossy(&frame.payload).to_string();
                        push_ws_frame(WsFrameEntry {
                            id: next_ws_frame_id(),
                            connection_id: conn_id,
                            timestamp: chrono::Local::now().format("%H:%M:%S%.3f").to_string(),
                            direction: WsDirection::ServerToClient,
                            opcode: frame.to_opcode(),
                            length: frame.payload.len(),
                            payload: payload_text,
                            payload_bytes: frame.payload.clone(),
                            is_final: frame.fin,
                        });

                        if let Ok(mut c) = client_arc.lock() {
                            if write_ws_frame(&mut *c, &frame).is_err() {
                                break;
                            }
                        } else {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }

            let _ = h1.join();
            return true;
        }
    }
    false
}

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
            loop {
                match read_ws_frame(&mut client_clone) {
                    Ok(frame) => {
                        let payload_text = String::from_utf8_lossy(&frame.payload).to_string();
                        push_ws_frame(WsFrameEntry {
                            id: next_ws_frame_id(),
                            connection_id: conn_id,
                            timestamp: chrono::Local::now().format("%H:%M:%S%.3f").to_string(),
                            direction: WsDirection::ClientToServer,
                            opcode: frame.to_opcode(),
                            length: frame.payload.len(),
                            payload: payload_text,
                            payload_bytes: frame.payload.clone(),
                            is_final: frame.fin,
                        });
                        if write_ws_frame(&mut server_clone, &frame).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        loop {
            match read_ws_frame(&mut server_tcp) {
                Ok(frame) => {
                    let payload_text = String::from_utf8_lossy(&frame.payload).to_string();
                    push_ws_frame(WsFrameEntry {
                        id: next_ws_frame_id(),
                        connection_id: conn_id,
                        timestamp: chrono::Local::now().format("%H:%M:%S%.3f").to_string(),
                        direction: WsDirection::ServerToClient,
                        opcode: frame.to_opcode(),
                        length: frame.payload.len(),
                        payload: payload_text,
                        payload_bytes: frame.payload.clone(),
                        is_final: frame.fin,
                    });
                    if write_ws_frame(client_stream, &frame).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }

        let _ = h1.join();
        return true;
    }
    false
}
