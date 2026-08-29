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

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct HeaderInjectionRule {
    pub enabled: bool,
    pub scope: String,
    pub header_name: String,
    pub header_value: String,
}

#[derive(Clone, Debug)]
pub struct InterceptRule {
    pub enabled: bool,
    pub match_type: String,
    pub pattern: String,
    pub action: String,
}

#[derive(Clone, Debug)]
pub struct InterceptState {
    pub enabled: bool,
    pub current_entry: Option<HttpEntry>,
    pub current_request: String,
    pub queue_count: usize,
    pub match_rules: Vec<InterceptRule>,
    pub show_rules_modal: bool,
    pub selected_paused_id: Option<u32>,
}

impl Default for InterceptState {
    fn default() -> Self {
        Self {
            enabled: false,
            current_entry: None,
            current_request: String::new(),
            queue_count: 0,
            match_rules: vec![],
            show_rules_modal: false,
            selected_paused_id: None,
        }
    }
}
