#![allow(dead_code)]
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WsSubTab {
    #[default]
    History,
    Intercept,
    Repeater,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WsOpcode {
    Text,
    Binary,
    Ping,
    Pong,
    Close,
    Continuation,
    Unknown(u8),
}

impl Default for WsOpcode {
    fn default() -> Self {
        WsOpcode::Text
    }
}

impl WsOpcode {
    pub fn label(&self) -> &'static str {
        match self {
            WsOpcode::Text => "TEXT",
            WsOpcode::Binary => "BINARY",
            WsOpcode::Ping => "PING",
            WsOpcode::Pong => "PONG",
            WsOpcode::Close => "CLOSE",
            WsOpcode::Continuation => "CONT",
            WsOpcode::Unknown(_) => "UNKNOWN",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WsDirection {
    ClientToServer, // ⬆️
    ServerToClient, // ⬇️
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WsFrameEntry {
    pub id: u64,
    pub connection_id: u32,
    pub timestamp: String,
    pub direction: WsDirection,
    pub opcode: WsOpcode,
    pub length: usize,
    pub payload: String,
    pub payload_bytes: Vec<u8>,
    pub is_final: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WsConnection {
    pub id: u32,
    pub url: String,
    pub host: String,
    pub path: String,
    pub client_addr: String,
    pub connected_at: String,
    pub status: String, // "Active", "Closed"
    pub message_count: usize,
}

#[derive(Clone, Debug, Default)]
pub struct WsHistoryState {
    pub selected_connection_id: Option<u32>,
    pub search_query: String,
    pub filter_opcode: Option<WsOpcode>,
    pub selected_frame_id: Option<u64>,
    pub inspector_mode: usize, // 0: Raw Text, 1: Hex, 2: JSON
    pub show_export_modal: bool,
    pub export_status_msg: String,
}

#[derive(Clone, Default)]
pub struct WsInterceptState {
    pub enabled: bool,
    pub selected_frame_id: Option<u64>,
    pub edited_payload: String,
    pub edited_opcode: WsOpcode,
}

#[derive(Clone, Debug)]
pub struct WsRepeaterTab {
    pub name: String,
    pub target_url: String,
    pub is_connected: bool,
    pub send_opcode: WsOpcode,
    pub payload_input: String,
    pub log_messages: Vec<WsFrameEntry>,
}

impl Default for WsRepeaterTab {
    fn default() -> Self {
        Self {
            name: "WS Tab 1".into(),
            target_url: "wss://echo.websocket.events".into(),
            is_connected: false,
            send_opcode: WsOpcode::Text,
            payload_input: "{\"event\":\"ping\",\"data\":\"hello_ajproxy\"}".into(),
            log_messages: vec![],
        }
    }
}
