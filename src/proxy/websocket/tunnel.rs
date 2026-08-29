use std::net::TcpStream;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::models::{WsDirection, WsFrameEntry, WsOpcode};
use crate::proxy::store::{
    next_ws_frame_id, push_ws_frame, update_ws_conn_status,
};
use super::protocol::{build_pong_frame, parse_close_code, read_ws_frame, write_ws_frame};
use super::intercept::check_and_intercept_frame;
use super::reassembly::FrameReassembler;

pub fn format_ws_payload_preview(opcode_u8: u8, payload: &[u8]) -> String {
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

pub fn is_timeout_err(err: &std::io::Error) -> bool {
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

pub fn run_tls_websocket_tunnel_loop(
    conn_id: u32,
    tls_stream: openssl::ssl::SslStream<TcpStream>,
    server_tls: openssl::ssl::SslStream<TcpStream>,
) {
    let client_arc = Arc::new(Mutex::new(tls_stream));
    let server_arc = Arc::new(Mutex::new(server_tls));

    let c_clone = Arc::clone(&client_arc);
    let s_clone = Arc::clone(&server_arc);

    let h1 = thread::spawn(move || {
        let mut reassembler = FrameReassembler::new();
        loop {
            if crate::proxy::store::is_ws_conn_force_closed(conn_id) {
                break;
            }

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
                    if crate::proxy::store::is_ws_conn_force_closed(conn_id) {
                        break;
                    }
                    thread::sleep(Duration::from_millis(5));
                    continue;
                }
                Err(_) => {
                    if !crate::proxy::store::is_ws_conn_force_closed(conn_id) {
                        update_ws_conn_status(conn_id, "Closed (EOF)");
                    }
                    break;
                }
            };

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

                if let Ok(mut s) = s_clone.lock() {
                    let _ = write_ws_frame(&mut *s, &frame);
                }
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

            if let Ok(mut s) = s_clone.lock() {
                if write_ws_frame(&mut *s, &frame_to_send).is_err() {
                    break;
                }
            } else {
                break;
            }
        }
    });

    let mut reassembler = FrameReassembler::new();
    loop {
        if crate::proxy::store::is_ws_conn_force_closed(conn_id) {
            break;
        }

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
                if crate::proxy::store::is_ws_conn_force_closed(conn_id) {
                    break;
                }
                thread::sleep(Duration::from_millis(5));
                continue;
            }
            Err(_) => {
                if !crate::proxy::store::is_ws_conn_force_closed(conn_id) {
                    update_ws_conn_status(conn_id, "Closed (EOF)");
                }
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

        if let Ok(mut c) = client_arc.lock() {
            if write_ws_frame(&mut *c, &frame_to_send).is_err() {
                break;
            }
        } else {
            break;
        }
    }

    let _ = h1.join();
}

pub fn run_plain_websocket_tunnel_loop(
    conn_id: u32,
    client_stream: &mut TcpStream,
    mut server_tcp: TcpStream,
) {
    let mut client_clone = match client_stream.try_clone() {
        Ok(c) => c,
        Err(_) => return,
    };
    let mut server_clone = match server_tcp.try_clone() {
        Ok(s) => s,
        Err(_) => return,
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
}
