use eframe::App;
use egui::{self, TopBottomPanel, CentralPanel};
use std::time::Instant;
use std::collections::HashMap;

use crate::models::*;
use crate::theme::*;
use crate::ui::{top_bar, status_bar, dashboard, traffic, intercept, repeater, bruteforce, decoder, comparer, sitemap, modules, settings};

pub struct AJProxyApp {
    #[allow(dead_code)]
    pub splash_start: Instant,
    pub active_tab: Tab,
    pub proxy_running: bool,
    pub http_entries: Vec<HttpEntry>,
    pub selected_entry: Option<usize>,
    pub filter_state: FilterState,
    pub detail_tab: usize,
    pub header_rules: Vec<HeaderInjectionRule>,
    pub show_header_panel: bool,
    pub intercept_state: InterceptState,
    pub repeater_tabs: Vec<RepeaterTab>,
    pub active_repeater_tab: usize,
    pub bruteforce_state: BruteForceState,
    pub decoder_state: DecoderState,
    pub comparer_state: ComparerState,
    pub sitemap_nodes: Vec<SiteMapNode>,
    pub modules_list: Vec<ModuleInfo>,
    pub settings: AppSettings,
}

impl AJProxyApp {
    pub fn new() -> Self {
        Self {
            splash_start: Instant::now(),
            active_tab: Tab::Dashboard,
            proxy_running: true,
            http_entries: vec![],
            selected_entry: None,
            filter_state: FilterState::default(),
            detail_tab: 0,
            header_rules: vec![],
            show_header_panel: false,
            intercept_state: InterceptState {
                enabled: false,
                current_entry: None,
                current_request: String::new(),
                match_rules: vec![],
                queue_count: 0,
                show_rules_modal: false,
                selected_paused_id: None,
            },
            repeater_tabs: vec![
                RepeaterTab {
                    name: "Tab 1".into(),
                    target_host: "".into(),
                    target_port: "80".into(),
                    protocol: "HTTP/1.1".into(),
                    is_tls: false,
                    request: "".into(),
                    request_text: "".into(),
                    response: "".into(),
                    response_text: "".into(),
                    response_headers: "".into(),
                    status: RepeaterStatus::Ready,
                    response_time_ms: 0,
                }
            ],
            active_repeater_tab: 0,
            bruteforce_state: BruteForceState {
                target_url: "".into(),
                method: "POST".into(),
                headers: "".into(),
                request_headers: "".into(),
                body_template: "".into(),
                payloads: "".into(),
                payload_list: "".into(),
                attack_type: AttackType::Sniper,
                running: false,
                is_running: false,
                results: vec![],
                progress: 0.0,
            },
            decoder_state: DecoderState::default(),
            comparer_state: ComparerState {
                item_a: "".into(),
                item_b: "".into(),
                left_text: "".into(),
                right_text: "".into(),
                left_label: "Request A".into(),
                right_label: "Request B".into(),
                diff_mode: DiffMode::Words,
                word_level: true,
                sync_scroll: true,
            },
            sitemap_nodes: vec![],
            modules_list: vec![],
            settings: AppSettings::default(),
        }
    }

    pub fn sync_live_traffic(&mut self, ctx: &egui::Context) {
        crate::proxy::listener::update_noise_filter_settings(
            crate::proxy::listener::NoiseFilterFlags {
                filter_scripts_styles_fonts: self.settings.filter_scripts_styles_fonts,
                filter_images_media: self.settings.filter_images_media,
                filter_noisy_domains: self.settings.filter_noisy_domains,
            }
        );
        crate::proxy::listener::update_passthrough_hosts(&self.settings.passthrough_hosts);

        let live_entries = crate::proxy::listener::get_captured_entries();
        if live_entries.len() != self.http_entries.len() {
            self.http_entries = live_entries;
            if self.active_tab == Tab::SiteMap {
                self.rebuild_sitemap();
            }
            ctx.request_repaint();
        }
    }

    pub fn rebuild_sitemap(&mut self) {
        let mut host_map: HashMap<String, Vec<String>> = HashMap::new();

        for entry in &self.http_entries {
            host_map.entry(entry.host.clone()).or_default().push(entry.path.clone());
        }

        self.sitemap_nodes = host_map
            .into_iter()
            .map(|(host, paths)| {
                let unique_paths: Vec<String> = paths.into_iter().collect();
                SiteMapNode {
                    name: host.clone(),
                    full_path: format!("http://{}", host),
                    request_count: unique_paths.len(),
                    in_scope: true,
                    expanded: true,
                    children: unique_paths
                        .into_iter()
                        .take(15)
                        .map(|p| SiteMapNode {
                            name: p.clone(),
                            full_path: format!("http://{}{}", host, p),
                            request_count: 1,
                            in_scope: true,
                            expanded: false,
                            children: vec![],
                        })
                        .collect(),
                }
            })
            .collect();
    }
}

impl Default for AJProxyApp {
    fn default() -> Self {
        Self::new()
    }
}

impl App for AJProxyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        apply_theme(ctx);
        paint_background_gradient(ctx);

        // Schedule continuous smooth UI repaint for live streaming traffic (100ms)
        ctx.request_repaint_after(std::time::Duration::from_millis(100));

        // Sync live traffic from TCP proxy listener
        self.sync_live_traffic(ctx);

        // Top Navigation Bar
        TopBottomPanel::top("top_bar")
            .exact_height(42.0)
            .show(ctx, |ui| {
                top_bar::render(
                    ui,
                    &mut self.active_tab,
                    &mut self.proxy_running,
                    &self.settings.listen_address,
                    self.settings.listen_port,
                );
            });

        // Window drag from top bar — raw pointer check, zero interaction overlay
        let top_bar_rect = egui::Rect::from_min_max(
            egui::pos2(0.0, 0.0),
            egui::pos2(ctx.screen_rect().width(), 42.0),
        );
        let should_drag = ctx.input(|i| {
            if let Some(origin) = i.pointer.press_origin() {
                top_bar_rect.contains(origin)
                    && i.pointer.is_decidedly_dragging()
            } else {
                false
            }
        });
        if should_drag && !ctx.is_using_pointer() {
            ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }

        // Compute noise-filtered entry list for UI rendering
        let filtered_entries: Vec<HttpEntry> = self
            .http_entries
            .iter()
            .filter(|e| !self.settings.is_filtered_noise(&e.url, &e.path, &e.content_type))
            .cloned()
            .collect();

        // Bottom Status Bar
        TopBottomPanel::bottom("status_bar")
            .exact_height(28.0)
            .show(ctx, |ui| {
                status_bar::render(
                    ui,
                    self.proxy_running,
                    filtered_entries.len(),
                    0,
                );
            });

        // Main Tab Content Area
        CentralPanel::default().show(ctx, |ui| {
            match self.active_tab {
                Tab::Dashboard => {
                    dashboard::render(
                        ui,
                        &filtered_entries,
                        self.proxy_running,
                    );
                }
                Tab::Traffic => {
                    let action = traffic::render(
                        ui,
                        &filtered_entries,
                        &mut self.selected_entry,
                        &mut self.filter_state,
                        &mut self.detail_tab,
                        &mut self.header_rules,
                        &mut self.show_header_panel,
                        ctx,
                    );
                    if action.clear_history {
                        self.http_entries.clear();
                        self.selected_entry = None;
                        crate::proxy::listener::clear_captured_entries();
                    }
                    if let Some(id) = action.send_to_repeater {
                        if let Some(entry) = self.http_entries.iter().find(|e| e.id as usize == id) {
                            let is_tls = entry.url.starts_with("https");
                            let default_port = if is_tls { "443" } else { "80" };

                            let mut req_full = String::new();
                            if !entry.request_headers.starts_with(&entry.method) {
                                req_full.push_str(&format!("{} {} HTTP/1.1\r\n", entry.method, entry.path));
                            }
                            req_full.push_str(&entry.request_headers);
                            if !req_full.ends_with("\r\n\r\n") && !req_full.ends_with("\n\n") {
                                req_full.push_str("\r\n\r\n");
                            }
                            req_full.push_str(&entry.request_body);

                            let req_full = crate::proxy::filters::apply_header_injection_rules(&entry.host, req_full);

                            self.repeater_tabs.push(RepeaterTab {
                                name: format!("Tab {}", self.repeater_tabs.len() + 1),
                                target_host: entry.host.clone(),
                                target_port: default_port.into(),
                                protocol: if entry.protocol.is_empty() { "HTTP/1.1".into() } else { entry.protocol.clone() },
                                is_tls,
                                request: req_full.clone(),
                                request_text: req_full,
                                response: entry.response_body.clone(),
                                response_text: entry.response_body.clone(),
                                response_headers: entry.response_headers.clone(),
                                status: RepeaterStatus::Ready,
                                response_time_ms: entry.duration_ms,
                            });
                            self.active_repeater_tab = self.repeater_tabs.len() - 1;
                            self.active_tab = Tab::Repeater;
                        }
                    }
                }
                Tab::Intercept => {
                    if let intercept::InterceptUIAction::SendToRepeater(host, port, req_raw, is_tls) = intercept::render(ui, &mut self.intercept_state, &mut self.settings, ctx) {
                        let req_raw = crate::proxy::filters::apply_header_injection_rules(&host, req_raw);
                        self.repeater_tabs.push(RepeaterTab {
                            name: format!("Tab {}", self.repeater_tabs.len() + 1),
                            target_host: host,
                            target_port: port,
                            protocol: "HTTP/1.1".into(),
                            is_tls,
                            request: req_raw.clone(),
                            request_text: req_raw,
                            response: String::new(),
                            response_text: String::new(),
                            response_headers: String::new(),
                            status: RepeaterStatus::Ready,
                            response_time_ms: 0,
                        });
                        self.active_repeater_tab = self.repeater_tabs.len() - 1;
                        self.active_tab = Tab::Repeater;
                    }
                }
                Tab::Repeater => repeater::render(
                    ui,
                    &mut self.repeater_tabs,
                    &mut self.active_repeater_tab,
                ),
                Tab::BruteForce => bruteforce::render(ui, &mut self.bruteforce_state),
                Tab::Decoder => decoder::render(ui, &mut self.decoder_state),
                Tab::Comparer => comparer::render(ui, &mut self.comparer_state),
                Tab::SiteMap => sitemap::render(
                    ui,
                    &mut self.sitemap_nodes,
                ),
                Tab::Modules => modules::render(ui, &mut self.modules_list),
                Tab::Settings => settings::render(ui, &mut self.settings),
            }
        });
    }
}
