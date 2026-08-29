use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use crate::models::{HttpEntry, InterceptRule, HeaderInjectionRule, WsConnection, WsFrameEntry};

static NEXT_ID: AtomicU32 = AtomicU32::new(1);
static NEXT_WS_CONN_ID: AtomicU32 = AtomicU32::new(1);
static NEXT_WS_FRAME_ID: AtomicU32 = AtomicU32::new(1);
static INTERCEPT_ENABLED: AtomicBool = AtomicBool::new(false); // DEFAULT OFF!
static WS_INTERCEPT_ENABLED: AtomicBool = AtomicBool::new(false); // DEFAULT OFF!

pub fn next_entry_id() -> u32 {
    NEXT_ID.fetch_add(1, Ordering::SeqCst)
}

pub fn next_ws_conn_id() -> u32 {
    NEXT_WS_CONN_ID.fetch_add(1, Ordering::SeqCst)
}

pub fn next_ws_frame_id() -> u64 {
    NEXT_WS_FRAME_ID.fetch_add(1, Ordering::SeqCst) as u64
}

pub fn set_intercept_enabled(enabled: bool) {
    INTERCEPT_ENABLED.store(enabled, Ordering::SeqCst);
}

pub fn is_intercept_enabled() -> bool {
    INTERCEPT_ENABLED.load(Ordering::SeqCst)
}

pub fn set_ws_intercept_enabled(enabled: bool) {
    WS_INTERCEPT_ENABLED.store(enabled, Ordering::SeqCst);
}

pub fn is_ws_intercept_enabled() -> bool {
    WS_INTERCEPT_ENABLED.load(Ordering::SeqCst)
}

pub enum InterceptDecision {
    Forward,
    Drop,
}

#[derive(Clone)]
pub struct PendingIntercept {
    pub id: u32,
    pub method: String,
    pub host: String,
    pub path: String,
    pub url: String,
    pub headers: String,
    pub body: String,
    pub responder: Arc<Mutex<Option<Sender<InterceptDecision>>>>,
}

#[derive(Clone)]
pub struct PendingWsFrame {
    #[allow(dead_code)]
    pub id: u64,
    pub connection_id: u32,
    pub direction: crate::models::WsDirection,
    pub opcode: crate::models::WsOpcode,
    pub payload: String,
    pub payload_bytes: Vec<u8>,
    pub responder: Arc<Mutex<Option<Sender<Option<crate::proxy::websocket::protocol::WsRawFrame>>>>>,
}

#[derive(Clone, Debug)]
pub struct NoiseFilterFlags {
    pub filter_scripts_styles_fonts: bool,
    pub filter_images_media: bool,
    pub filter_noisy_domains: bool,
}

impl Default for NoiseFilterFlags {
    fn default() -> Self {
        Self {
            filter_scripts_styles_fonts: true,
            filter_images_media: true,
            filter_noisy_domains: true,
        }
    }
}

lazy_static::lazy_static! {
    pub static ref TRAFFIC_STORE: Arc<Mutex<Vec<HttpEntry>>> = Arc::new(Mutex::new(Vec::new()));
    pub static ref PENDING_INTERCEPTS: Arc<Mutex<Vec<PendingIntercept>>> = Arc::new(Mutex::new(Vec::new()));
    pub static ref MATCH_REPLACE_RULES: Arc<Mutex<Vec<InterceptRule>>> = Arc::new(Mutex::new(Vec::new()));
    pub static ref HEADER_INJECTION_RULES: Arc<Mutex<Vec<HeaderInjectionRule>>> = Arc::new(Mutex::new(Vec::new()));
    pub static ref NOISE_FILTER_SETTINGS: Arc<Mutex<NoiseFilterFlags>> = Arc::new(Mutex::new(NoiseFilterFlags::default()));
    pub static ref PASSTHROUGH_HOSTS: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    pub static ref WS_CONNECTIONS: Arc<Mutex<Vec<WsConnection>>> = Arc::new(Mutex::new(Vec::new()));
    pub static ref WS_FRAMES: Arc<Mutex<Vec<WsFrameEntry>>> = Arc::new(Mutex::new(Vec::new()));
    pub static ref PENDING_WS_FRAMES: Arc<Mutex<Vec<PendingWsFrame>>> = Arc::new(Mutex::new(Vec::new()));
    pub static ref UPSTREAM_AGENT: ureq::Agent = ureq::AgentBuilder::new()
        .redirects(0)
        .timeout(std::time::Duration::from_secs(30))
        .max_idle_connections(200)
        .max_idle_connections_per_host(20)
        .build();
}

pub fn update_ws_conn_status(conn_id: u32, new_status: &str) {
    if let Ok(mut lock) = WS_CONNECTIONS.lock() {
        if let Some(conn) = lock.iter_mut().find(|c| c.id == conn_id) {
            conn.status = new_status.to_string();
        }
    }
}

pub fn push_ws_connection(conn: WsConnection) {
    if let Ok(mut lock) = WS_CONNECTIONS.lock() {
        lock.push(conn);
    }
}

const MAX_STORE_ENTRIES: usize = 5000;

pub fn push_ws_frame(frame: WsFrameEntry) {
    if let Ok(mut lock) = WS_FRAMES.lock() {
        if let Ok(mut conns) = WS_CONNECTIONS.lock() {
            if let Some(conn) = conns.iter_mut().find(|c| c.id == frame.connection_id) {
                conn.message_count += 1;
            }
        }
        if lock.len() >= MAX_STORE_ENTRIES {
            lock.drain(0..1000);
        }
        lock.push(frame);
    }
}

pub fn get_ws_connections() -> Vec<WsConnection> {
    if let Ok(lock) = WS_CONNECTIONS.lock() {
        lock.clone()
    } else {
        Vec::new()
    }
}

pub fn get_ws_frames() -> Vec<WsFrameEntry> {
    if let Ok(lock) = WS_FRAMES.lock() {
        lock.clone()
    } else {
        Vec::new()
    }
}

pub fn clear_ws_history() {
    if let Ok(mut lock) = WS_CONNECTIONS.lock() {
        lock.clear();
    }
    if let Ok(mut lock) = WS_FRAMES.lock() {
        lock.clear();
    }
}

pub fn update_passthrough_hosts(hosts_csv: &str) {
    let hosts: Vec<String> = hosts_csv
        .split(',')
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
        .collect();
    if let Ok(mut lock) = PASSTHROUGH_HOSTS.lock() {
        *lock = hosts;
    }
}

pub fn update_noise_filter_settings(flags: NoiseFilterFlags) {
    if let Ok(mut lock) = NOISE_FILTER_SETTINGS.lock() {
        *lock = flags;
    }
}

pub fn update_match_rules(rules: Vec<InterceptRule>) {
    if let Ok(mut lock) = MATCH_REPLACE_RULES.lock() {
        *lock = rules;
    }
}

pub fn update_header_injection_rules(rules: Vec<HeaderInjectionRule>) {
    if let Ok(mut lock) = HEADER_INJECTION_RULES.lock() {
        *lock = rules;
    }
}

pub fn push_captured_entry(entry: HttpEntry) {
    if let Ok(mut store) = TRAFFIC_STORE.lock() {
        if store.len() >= MAX_STORE_ENTRIES {
            store.drain(0..1000);
        }
        store.push(entry);
    }
}

pub fn get_captured_entries() -> Vec<HttpEntry> {
    if let Ok(store) = TRAFFIC_STORE.lock() {
        store.clone()
    } else {
        Vec::new()
    }
}

pub fn clear_captured_entries() {
    if let Ok(mut store) = TRAFFIC_STORE.lock() {
        store.clear();
    }
}

#[allow(dead_code)]
pub fn clear_traffic_store() {
    clear_captured_entries();
}

pub fn get_pending_intercepts() -> Vec<PendingIntercept> {
    if let Ok(lock) = PENDING_INTERCEPTS.lock() {
        lock.clone()
    } else {
        Vec::new()
    }
}

pub fn resolve_pending_intercept(id: u32, decision: InterceptDecision) {
    let mut sender_opt = None;
    if let Ok(mut lock) = PENDING_INTERCEPTS.lock() {
        if let Some(pos) = lock.iter().position(|p| p.id == id) {
            let item = lock.remove(pos);
            let responder = item.responder.clone();
            if let Ok(mut s_lock) = responder.lock() {
                sender_opt = s_lock.take();
            };
        }
    }
    if let Some(sender) = sender_opt {
        let _ = sender.send(decision);
    }
}
