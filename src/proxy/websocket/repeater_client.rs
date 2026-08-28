use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::mpsc::{channel, Sender};
use std::thread;
use crate::models::{WsDirection, WsFrameEntry};
use crate::proxy::websocket::protocol::{read_ws_frame, write_ws_frame, WsRawFrame};

#[allow(dead_code)]
pub struct WsRepeaterClientHandle {
    pub tx: Sender<WsRawFrame>,
}

#[allow(dead_code)]
pub fn spawn_repeater_client(
    target_url: String,
    on_frame_received: impl Fn(WsFrameEntry) + Send + 'static,
) -> Result<WsRepeaterClientHandle, String> {
    // Parse target url (e.g. ws://host:port/path or wss://host:port/path)
    let is_tls = target_url.starts_with("wss://");
    let clean_url = target_url.trim_start_matches("wss://").trim_start_matches("ws://");

    let (host_port, path) = match clean_url.split_once('/') {
        Some((hp, p)) => (hp, format!("/{}", p)),
        None => (clean_url, "/".to_string()),
    };

    let target_addr = if host_port.contains(':') {
        host_port.to_string()
    } else if is_tls {
        format!("{}:443", host_port)
    } else {
        format!("{}:80", host_port)
    };

    let mut stream = TcpStream::connect(&target_addr)
        .map_err(|e| format!("Connection failed to {}: {}", target_addr, e))?;

    // Perform standard HTTP WebSocket Upgrade handshake
    let handshake = format!(
        "GET {} HTTP/1.1\r\n\
         Host: {}\r\n\
         Upgrade: websocket\r\n\
         Connection: Upgrade\r\n\
         Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\
         Sec-WebSocket-Version: 13\r\n\r\n",
        path, host_port
    );

    stream.write_all(handshake.as_bytes()).map_err(|e| e.to_string())?;
    stream.flush().map_err(|e| e.to_string())?;

    let mut handshake_buf = [0u8; 1024];
    let n = stream.read(&mut handshake_buf).map_err(|e| e.to_string())?;
    let resp_str = String::from_utf8_lossy(&handshake_buf[..n]);

    if !resp_str.contains("101") {
        return Err(format!("Handshake rejected: {}", resp_str.lines().next().unwrap_or("")));
    }

    let (tx, rx) = channel::<WsRawFrame>();
    let mut stream_clone = stream.try_clone().map_err(|e| e.to_string())?;

    // Receiver thread
    thread::spawn(move || {
        loop {
            match read_ws_frame(&mut stream_clone) {
                Ok(frame) => {
                    let payload_text = String::from_utf8_lossy(&frame.payload).to_string();
                    let entry = WsFrameEntry {
                        id: crate::proxy::store::next_ws_frame_id(),
                        connection_id: 0,
                        timestamp: chrono::Local::now().format("%H:%M:%S%.3f").to_string(),
                        direction: WsDirection::ServerToClient,
                        opcode: frame.to_opcode(),
                        length: frame.payload.len(),
                        payload: payload_text,
                        payload_bytes: frame.payload,
                        is_final: frame.fin,
                    };
                    on_frame_received(entry);
                }
                Err(_) => break,
            }
        }
    });

    // Sender thread
    thread::spawn(move || {
        while let Ok(frame) = rx.recv() {
            if write_ws_frame(&mut stream, &frame).is_err() {
                break;
            }
        }
    });

    Ok(WsRepeaterClientHandle { tx })
}
