use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use crate::models::{HttpEntry, InterceptRule, HeaderInjectionRule};

static NEXT_ID: AtomicU32 = AtomicU32::new(1);
static INTERCEPT_ENABLED: AtomicBool = AtomicBool::new(false); // DEFAULT OFF!

pub fn next_entry_id() -> u32 {
    NEXT_ID.fetch_add(1, Ordering::SeqCst)
}

pub fn set_intercept_enabled(enabled: bool) {
    INTERCEPT_ENABLED.store(enabled, Ordering::SeqCst);
}

pub fn is_intercept_enabled() -> bool {
    INTERCEPT_ENABLED.load(Ordering::SeqCst)
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
    pub static ref UPSTREAM_AGENT: ureq::Agent = ureq::AgentBuilder::new()
        .redirects(0)
        .timeout(std::time::Duration::from_secs(30))
        .max_idle_connections(200)
        .max_idle_connections_per_host(20)
        .build();
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
