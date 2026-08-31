use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};
use crate::models::WsDirection;
use crate::proxy::store::{is_ws_intercept_enabled, PENDING_WS_FRAMES, PendingWsFrame};
use crate::proxy::websocket::protocol::WsRawFrame;

pub fn check_and_intercept_frame(
    conn_id: u32,
    frame: WsRawFrame,
    direction: WsDirection,
) -> Option<WsRawFrame> {
    if !is_ws_intercept_enabled() || !crate::proxy::store::is_ws_conn_in_scope(conn_id) {
        return Some(frame);
    }

    // Do not block heartbeat or closure control frames (Close 0x8, Ping 0x9, Pong 0xA)
    if frame.opcode_u8 == 0x8 || frame.opcode_u8 == 0x9 || frame.opcode_u8 == 0xA {
        return Some(frame);
    }

    let (tx, rx) = channel::<Option<WsRawFrame>>();

    let pending = PendingWsFrame {
        id: crate::proxy::store::next_ws_frame_id(),
        connection_id: conn_id,
        direction,
        opcode: frame.to_opcode(),
        raw_opcode_u8: frame.opcode_u8,
        payload: String::from_utf8_lossy(&frame.payload).to_string(),
        payload_bytes: frame.payload.clone(),
        responder: Arc::new(Mutex::new(Some(tx))),
    };

    if let Ok(mut lock) = PENDING_WS_FRAMES.lock() {
        lock.push(pending);
    } else {
        return Some(frame);
    }

    // Block current tunnel thread until UI resolves Forward or Drop
    match rx.recv() {
        Ok(resolved_frame) => resolved_frame,
        Err(_) => Some(frame),
    }
}
