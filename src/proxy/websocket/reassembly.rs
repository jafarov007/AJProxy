use std::collections::HashMap;
use crate::proxy::websocket::protocol::WsRawFrame;

pub struct FrameReassembler {
    buffers: HashMap<u32, Vec<WsRawFrame>>,
}

impl FrameReassembler {
    pub fn new() -> Self {
        Self {
            buffers: HashMap::new(),
        }
    }

    /// Process incoming frame for connection. Returns `Some(WsRawFrame)` when a complete message is formed.
    pub fn process_frame(&mut self, conn_id: u32, frame: WsRawFrame) -> Option<WsRawFrame> {
        if frame.fin && frame.opcode_u8 != 0x0 {
            // Unfragmented frame: return immediately
            return Some(frame);
        }

        let entry = self.buffers.entry(conn_id).or_default();
        entry.push(frame.clone());

        if frame.fin {
            // Final continuation fragment: reassemble buffered fragments
            let mut combined_payload = Vec::new();
            let first_opcode = entry.first().map(|f| f.opcode_u8).unwrap_or(0x1);

            for f in entry.drain(..) {
                combined_payload.extend_from_slice(&f.payload);
            }

            Some(WsRawFrame {
                fin: true,
                opcode_u8: first_opcode,
                masked: frame.masked,
                mask_key: frame.mask_key,
                payload: combined_payload,
            })
        } else {
            // Intermediate fragment: wait for final fragment
            None
        }
    }

    pub fn clear_connection(&mut self, conn_id: u32) {
        self.buffers.remove(&conn_id);
    }
}
