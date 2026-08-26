#![allow(dead_code)]
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HttpEntry {
    pub id: u32,
    pub timestamp: String,
    pub method: String,
    pub host: String,
    pub path: String,
    pub url: String,
    pub status_code: u16,
    pub content_type: String,
    pub length: usize,
    pub duration_ms: u64,
    pub protocol: String,
    pub request_headers: String,
    pub request_body: String,
    pub response_headers: String,
    pub response_body: String,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tab {
    #[default]
    Dashboard,
    Traffic,
    Intercept,
    Repeater,
    BruteForce,
    Decoder,
    Comparer,
    SiteMap,
    Modules,
    Settings,
}

#[derive(Clone, Debug, Default)]
pub struct HeaderInjectionRule {
    pub enabled: bool,
    pub scope: String,
    pub header_name: String,
    pub header_value: String,
}

#[derive(Clone, Debug)]
pub struct FilterState {
    pub search_query: String,
    pub filter_method: String,
    pub filter_status: String,
    pub in_scope_only: bool,
    pub status_min: u16,
    pub status_max: u16,
    pub path_filter: String,
    pub protocol_filter: String,
}

impl Default for FilterState {
    fn default() -> Self {
        Self {
            search_query: String::new(),
            filter_method: "ALL".to_string(),
            filter_status: "ALL".to_string(),
            in_scope_only: false,
            status_min: 0,
            status_max: 999,
            path_filter: String::new(),
            protocol_filter: "ALL".to_string(),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct TrafficAction {
    pub send_to_repeater: Option<usize>,
    pub send_to_bruteforce: Option<usize>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum AttackType {
    #[default] Sniper, BatteringRam, Pitchfork, ClusterBomb,
}

#[derive(Clone, Debug, Default)]
pub struct BruteForceState {
    pub target_url: String,
    pub method: String,
    pub headers: String,
    pub request_headers: String,
    pub body_template: String,
    pub payloads: String,
    pub payload_list: String,
    pub attack_type: AttackType,
    pub running: bool,
    pub is_running: bool,
    pub results: Vec<BruteResult>,
    pub progress: f32,
}

#[derive(Clone, Debug)]
pub struct BruteResult {
    pub id: usize,
    pub payload: String,
    pub status_code: u16,
    pub length: usize,
    pub duration_ms: u64,
}

#[derive(Clone, Debug)]
pub struct InterceptState {
    pub enabled: bool,
    pub current_entry: Option<HttpEntry>,
    pub current_request: String,
    pub queue_count: usize,
    pub match_rules: Vec<InterceptRule>,
}

#[derive(Clone, Debug)]
pub struct RepeaterTab {
    pub name: String,
    pub target_host: String,
    pub target_port: String,
    pub protocol: String,
    pub is_tls: bool,
    pub request: String,
    pub request_text: String,
    pub response: String,
    pub response_text: String,
    pub response_headers: String,
    pub status: RepeaterStatus,
    pub response_time_ms: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum RepeaterStatus { #[default] Ready, Sending, Done, Error }

#[derive(Clone, Debug, Default)]
pub struct DecoderState {
    pub input: String,
    pub output: String,
    pub encoding: EncodingType,
    pub direction: TransformDirection,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum EncodingType { #[default] Base64, URL, HTML, Hex, JWT }

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum TransformDirection { #[default] Decode, Encode }

#[derive(Clone, Debug, Default)]
pub struct ComparerState {
    pub item_a: String,
    pub item_b: String,
    pub left_text: String,
    pub right_text: String,
    pub left_label: String,
    pub right_label: String,
    pub diff_mode: DiffMode,
    pub word_level: bool,
    pub sync_scroll: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum DiffMode { #[default] Words, Bytes }

#[derive(Clone, Debug)]
pub struct SiteMapNode {
    pub name: String,
    pub full_path: String,
    pub children: Vec<SiteMapNode>,
    pub request_count: usize,
    pub in_scope: bool,
    pub expanded: bool,
}

#[derive(Clone, Debug)]
pub struct InterceptRule {
    pub enabled: bool,
    pub match_type: String,
    pub pattern: String,
    pub action: String,
}

#[derive(Clone, Debug)]
pub struct ModuleInfo {
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
    pub category: ModuleCategory,
    pub enabled: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModuleCategory { Scanner, Analyzer, Custom }

#[derive(Clone, Debug)]
pub struct ProxyListenerConfig {
    pub enabled: bool,
    pub bind_address: String,
    pub bind_port: u16,
    pub protocol: String,
    pub tls_mitm: bool,
}

impl Default for ProxyListenerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            bind_address: "127.0.0.1".into(),
            bind_port: 8080,
            protocol: "Auto".into(),
            tls_mitm: true,
        }
    }
}

#[derive(Clone, Debug)]
pub struct AppSettings {
    pub listen_address: String,
    pub listen_port: u16,
    pub proxy_host: String,
    pub proxy_port: u16,
    pub tls_enabled: bool,
    pub protocol_preference: String,
    pub intercept_requests: bool,
    pub intercept_responses: bool,
    pub passthrough_hosts: String,
    pub ca_cert_path: String,
    pub listeners: Vec<ProxyListenerConfig>,
    pub cert_status_msg: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            listen_address: "127.0.0.1".into(),
            listen_port: 8080,
            proxy_host: "127.0.0.1".into(),
            proxy_port: 8080,
            tls_enabled: true,
            protocol_preference: "Auto".into(),
            intercept_requests: true,
            intercept_responses: true,
            passthrough_hosts: "*.google.com, *.gstatic.com".into(),
            ca_cert_path: "~/.ajproxy/ca.crt".into(),
            listeners: vec![
                ProxyListenerConfig {
                    enabled: true,
                    bind_address: "127.0.0.1".into(),
                    bind_port: 8080,
                    protocol: "Auto".into(),
                    tls_mitm: true,
                },
                ProxyListenerConfig {
                    enabled: false,
                    bind_address: "0.0.0.0".into(),
                    bind_port: 8088,
                    protocol: "HTTP/1.1".into(),
                    tls_mitm: true,
                },
            ],
            cert_status_msg: String::new(),
        }
    }
}

impl AppSettings {
    pub fn sync_active_listener(&mut self) {
        if let Some(active) = self.listeners.iter().find(|l| l.enabled) {
            self.listen_address = active.bind_address.clone();
            self.listen_port = active.bind_port;
        } else if let Some(first) = self.listeners.first() {
            self.listen_address = first.bind_address.clone();
            self.listen_port = first.bind_port;
        }
    }
}


