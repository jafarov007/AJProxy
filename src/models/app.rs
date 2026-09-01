#![allow(dead_code)]
use super::http::{HeaderInjectionRule, InterceptRule};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tab {
    #[default]
    Dashboard,
    Traffic,
    WebSockets,
    Intercept,
    Repeater,
    BruteForce,
    Decoder,
    Comparer,
    SiteMap,
    Modules,
    Settings,
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
    pub show_export_modal: bool,
    pub export_status_msg: String,
    pub export_path: String,
    pub hide_zero_size: bool,
    pub show_host_filter_modal: bool,
    pub host_filters: Vec<String>,
    pub new_host_filter_input: String,
    pub show_method_filter_modal: bool,
    pub method_filters: Vec<String>,
    pub new_method_filter_input: String,
    pub show_path_filter_modal: bool,
    pub path_filters: Vec<String>,
    pub new_path_filter_input: String,
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
            show_export_modal: false,
            export_status_msg: String::new(),
            export_path: String::new(),
            hide_zero_size: false,
            show_host_filter_modal: false,
            host_filters: Vec::new(),
            new_host_filter_input: String::new(),
            show_method_filter_modal: false,
            method_filters: Vec::new(),
            new_method_filter_input: String::new(),
            show_path_filter_modal: false,
            path_filters: Vec::new(),
            new_path_filter_input: String::new(),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct TrafficAction {
    pub send_to_repeater: Option<usize>,
    pub send_to_bruteforce: Option<usize>,
    pub clear_history: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum AttackType {
    #[default] Sniper, BatteringRam, Pitchfork, ClusterBomb,
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct PayloadSet {
    pub name: String,
    pub payloads_text: String,
}

#[derive(Clone, Debug)]
pub struct BruteForceState {
    pub target_url: String,
    pub method: String,
    pub headers: String,
    pub request_headers: String,
    pub body_template: String,
    pub payloads: String,
    pub payload_sets: Vec<PayloadSet>,
    pub active_set_index: usize,
    pub show_payload_modal: bool,
    pub new_set_name: String,
    pub new_set_payloads: String,
    pub attack_type: AttackType,
    pub delay_sec_input: String,
    pub concurrency_input: String,
    pub running: bool,
    pub is_running: bool,
    pub results: Vec<BruteResult>,
    pub progress: f32,
    // Status Code & Search Filters
    pub filter_status_code: String,
    pub ignore_status_code: String,
    pub search_filter: String,
    // Position to Payload Set mapping
    pub position_set_indices: Vec<usize>,
    // Real-time async / batch queue state
    pub pending_queue: Vec<(String, String)>,
    pub last_batch_time: Option<std::time::Instant>,
}

impl Default for BruteForceState {
    fn default() -> Self {
        Self {
            target_url: String::new(),
            method: "POST".to_string(),
            headers: String::new(),
            request_headers: String::new(),
            body_template: "POST /scenario/1/api/v1/login HTTP/1.1\r\nHost: localhost:8080\r\nContent-Type: application/json\r\n\r\n{\n  \"email\": \"§user.a@example.com§\",\n  \"password\": \"§password123§\"\n}".to_string(),
            payloads: String::new(),
            payload_sets: Vec::new(),
            active_set_index: 0,
            show_payload_modal: false,
            new_set_name: String::new(),
            new_set_payloads: String::new(),
            attack_type: AttackType::Sniper,
            delay_sec_input: "0.5".to_string(),
            concurrency_input: "1".to_string(),
            running: false,
            is_running: false,
            results: Vec::new(),
            progress: 0.0,
            filter_status_code: String::new(),
            ignore_status_code: String::new(),
            search_filter: String::new(),
            position_set_indices: Vec::new(),
            pending_queue: Vec::new(),
            last_batch_time: None,
        }
    }
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
    pub error_msg: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum EncodingType { #[default] Base64, URL, HTML, Hex, JWT, MD5, SHA1, SHA256, SHA512 }

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum TransformDirection { #[default] Decode, Encode }

#[derive(Clone, Debug)]
pub struct ComparerState {
    pub left_text: String,
    pub right_text: String,
    pub left_label: String,
    pub right_label: String,
    pub diff_mode: DiffMode,
    pub sync_scroll: bool,
    pub ignore_case: bool,
    pub ignore_whitespace: bool,
    pub show_diff_only: bool,
}

impl Default for ComparerState {
    fn default() -> Self {
        Self {
            left_text: String::new(),
            right_text: String::new(),
            left_label: "Item A".to_string(),
            right_label: "Item B".to_string(),
            diff_mode: DiffMode::Words,
            sync_scroll: true,
            ignore_case: false,
            ignore_whitespace: false,
            show_diff_only: false,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum DiffMode {
    #[default]
    Words,
    Lines,
    Bytes,
}

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
    pub filter_scripts_styles_fonts: bool,
    pub filter_images_media: bool,
    pub filter_noisy_domains: bool,
    pub match_rules: Vec<InterceptRule>,
    pub header_injection_rules: Vec<HeaderInjectionRule>,
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
            intercept_responses: false,
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
            ],
            cert_status_msg: String::new(),
            filter_scripts_styles_fonts: true,
            filter_images_media: true,
            filter_noisy_domains: true,
            match_rules: vec![],
            header_injection_rules: vec![],
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

    pub fn is_filtered_noise(&self, url: &str, path: &str, content_type: &str) -> bool {
        let url_lower = url.to_lowercase();
        let path_lower = path.to_lowercase();
        let ct_lower = content_type.to_lowercase();

        let clean_path = path_lower.split('?').next().unwrap_or(&path_lower);
        let clean_path = clean_path.split('#').next().unwrap_or(clean_path);

        if self.filter_scripts_styles_fonts {
            if clean_path.ends_with(".js")
                || clean_path.ends_with(".mjs")
                || clean_path.ends_with(".cjs")
                || clean_path.ends_with(".css")
                || clean_path.ends_with(".woff")
                || clean_path.ends_with(".woff2")
                || clean_path.ends_with(".ttf")
                || clean_path.ends_with(".otf")
                || clean_path.ends_with(".eot")
                || path_lower.contains(".js?")
                || path_lower.contains(".js#")
                || path_lower.contains(".css?")
                || path_lower.contains(".css#")
                || ct_lower.contains("javascript")
                || ct_lower.contains("text/css")
                || ct_lower.contains("ecmascript")
                || ct_lower.starts_with("font/")
            {
                return true;
            }
        }

        if self.filter_images_media {
            if clean_path.ends_with(".png")
                || clean_path.ends_with(".jpg")
                || clean_path.ends_with(".jpeg")
                || clean_path.ends_with(".gif")
                || clean_path.ends_with(".svg")
                || clean_path.ends_with(".ico")
                || clean_path.ends_with(".webp")
                || path_lower.contains(".png?")
                || path_lower.contains(".jpg?")
                || path_lower.contains(".jpeg?")
                || path_lower.contains(".gif?")
                || path_lower.contains(".svg?")
                || path_lower.contains(".ico?")
                || ct_lower.starts_with("image/")
            {
                return true;
            }
        }

        if self.filter_noisy_domains {
            if url_lower.contains("challenges.cloudflare.com")
                || url_lower.contains("google.")
                || url_lower.contains("googleapis.")
                || url_lower.contains("gstatic.")
                || url_lower.contains("googletagmanager.")
                || url_lower.contains("google-analytics.")
                || url_lower.contains("googlesyndication.")
                || url_lower.contains("googleadservices.")
                || url_lower.contains(".google")
                || url_lower.contains("yandex.")
                || url_lower.contains("yastatic.")
                || url_lower.contains("mc.yandex")
                || url_lower.contains(".yandex")
            {
                return true;
            }
        }

        false
    }
}
