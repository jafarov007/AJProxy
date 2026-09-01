use egui::{self, RichText, Rounding, Stroke, ScrollArea, TextStyle, FontFamily};
use crate::models::*;
use crate::theme::*;
use crate::ui::syntax;
fn update_request_first_line_protocol(raw_req: &str, new_proto: &str) -> String {
    let mut lines = raw_req.lines();
    if let Some(first_line) = lines.next() {
        let parts: Vec<&str> = first_line.split_whitespace().collect();
        if parts.len() >= 2 {
            let method = parts[0];
            let path = parts[1];
            let new_first_line = format!("{} {} {}", method, path, new_proto);
            let mut result = new_first_line;
            for line in lines {
                result.push_str("\r\n");
                result.push_str(line);
            }
            if raw_req.ends_with("\r\n") || raw_req.ends_with('\n') {
                result.push_str("\r\n");
            }
            return result;
        }
    }
    raw_req.to_string()
}

pub enum RepeaterAction {
    None,
    SendToIntruder(String, String, String, bool), // host, port, raw_req, is_tls
}

pub fn render(ui: &mut egui::Ui, tabs: &mut Vec<RepeaterTab>, active_tab: &mut usize) -> RepeaterAction {
    let mut repeater_action = RepeaterAction::None;
    if tabs.is_empty() {
        tabs.push(RepeaterTab {
            name: "Tab 1".into(),
            target_host: "jafarov007.github.io".into(),
            target_port: "443".into(),
            protocol: "HTTP/1.1".into(),
            is_tls: true,
            request: "GET / HTTP/1.1\r\nHost: jafarov007.github.io\r\nAccept: text/html\r\n".into(),
            request_text: "GET / HTTP/1.1\r\nHost: jafarov007.github.io\r\nAccept: text/html\r\n".into(),
            response: String::new(),
            response_text: String::new(),
            response_headers: String::new(),
            status: RepeaterStatus::Ready,
            response_time_ms: 0,
        });
    }

    let mut to_close: Option<usize> = None;
    let mut swap_left: Option<usize> = None;
    let mut swap_right: Option<usize> = None;
    let mut toast_msg: Option<String> = None;

    // ── Top Row: Tab Strip ──────────────────────────────────────────────────
    let request_col_width = (ui.available_width() * 0.49).max(220.0);

    ui.with_layout(egui::Layout::left_to_right(egui::Align::TOP), |ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        ui.spacing_mut().button_padding = egui::vec2(10.0, 5.0);

        ScrollArea::horizontal()
            .id_source("repeater_tab_strip_scroll")
            .max_width(request_col_width - 100.0)
            .max_height(32.0)
            .show(ui, |ui| {
                ui.with_layout(egui::Layout::left_to_right(egui::Align::TOP), |ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;

                    for i in 0..tabs.len() {
                        ui.push_id(i, |ui| {
                            let is_active = *active_tab == i;
                            let bg_color = if is_active { BG_RAISED } else { BG_SURFACE };
                            let stroke_color = if is_active { ACCENT_BLUE } else { BORDER };
                            let text_color = if is_active { TEXT_0 } else { TEXT_2 };

                            let tab_label = format!("{}   ✖", tabs[i].name);

                            let tab_btn = egui::Button::new(
                                RichText::new(&tab_label)
                                    .size(12.0)
                                    .color(text_color)
                                    .strong()
                            )
                            .fill(bg_color)
                            .stroke(Stroke::new(if is_active { 1.0_f32 } else { 0.5_f32 }, stroke_color))
                            .rounding(Rounding::same(4.0));

                            let res = ui.add(tab_btn);

                            if res.clicked() {
                                if let Some(pos) = res.interact_pointer_pos() {
                                    if pos.x >= (res.rect.max.x - 24.0) {
                                        to_close = Some(i);
                                    } else {
                                        *active_tab = i;
                                    }
                                } else {
                                    *active_tab = i;
                                }
                            }

                            res.context_menu(|ui| {
                                ui.set_min_width(180.0);
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new("✏ Rename:").size(11.0).color(TEXT_1).strong());
                                    ui.add(egui::TextEdit::singleline(&mut tabs[i].name).desired_width(110.0));
                                });
                                ui.separator();

                                if ui.button(RichText::new("📋 Copy Raw Request").size(11.5).color(TEXT_0)).clicked() {
                                    ui.output_mut(|o| o.copied_text = tabs[i].request_text.clone());
                                    toast_msg = Some("Raw Request Copied!".into());
                                    ui.close_menu();
                                }
                                if ui.button(RichText::new("💻 Copy as cURL").size(11.5).color(TEXT_0)).clicked() {
                                    let proto = if tabs[i].is_tls { "https" } else { "http" };
                                    let curl = format!(
                                        "curl -i -s -k -X GET '{}://{}:{}/' -H 'Host: {}'",
                                        proto, tabs[i].target_host, tabs[i].target_port, tabs[i].target_host
                                    );
                                    ui.output_mut(|o| o.copied_text = curl);
                                    toast_msg = Some("cURL Command Copied!".into());
                                    ui.close_menu();
                                }
                                ui.separator();

                                if i > 0 && ui.button(RichText::new("◀ Move Left").size(11.0).color(TEXT_0)).clicked() {
                                    swap_left = Some(i);
                                    ui.close_menu();
                                }
                                if i + 1 < tabs.len() && ui.button(RichText::new("▶ Move Right").size(11.0).color(TEXT_0)).clicked() {
                                    swap_right = Some(i);
                                    ui.close_menu();
                                }
                                ui.separator();
                                if ui.button(RichText::new("✖ Close Tab").size(11.0).color(ACCENT_RED)).clicked() {
                                    to_close = Some(i);
                                    ui.close_menu();
                                }
                            });
                        });
                    }
                });
            });

        let new_tab_btn = egui::Button::new(RichText::new("+ New Tab").size(11.5).color(TEXT_0).strong())
            .fill(ACCENT_BLUE)
            .rounding(Rounding::same(4.0));

        if ui.add(new_tab_btn).clicked() {
            let n = tabs.len() + 1;
            tabs.push(RepeaterTab {
                name: format!("Tab {}", n),
                target_host: "httpbin.org".into(),
                target_port: "443".into(),
                protocol: "HTTP/1.1".into(),
                is_tls: true,
                request: "GET /get HTTP/1.1\r\nHost: httpbin.org\r\n".into(),
                request_text: "GET /get HTTP/1.1\r\nHost: httpbin.org\r\n".into(),
                response: String::new(),
                response_text: String::new(),
                response_headers: String::new(),
                status: RepeaterStatus::Ready,
                response_time_ms: 0,
            });
            *active_tab = tabs.len() - 1;
        }
    });

    if let Some(i) = swap_left {
        tabs.swap(i, i - 1);
        if *active_tab == i {
            *active_tab = i - 1;
        } else if *active_tab == i - 1 {
            *active_tab = i;
        }
    }

    if let Some(i) = swap_right {
        tabs.swap(i, i + 1);
        if *active_tab == i {
            *active_tab = i + 1;
        } else if *active_tab == i + 1 {
            *active_tab = i;
        }
    }

    if let Some(i) = to_close {
        if tabs.len() > 1 {
            tabs.remove(i);
            if *active_tab >= tabs.len() {
                *active_tab = tabs.len() - 1;
            }
        } else {
            tabs[0] = RepeaterTab {
                name: "Tab 1".into(),
                target_host: "httpbin.org".into(),
                target_port: "443".into(),
                protocol: "HTTP/1.1".into(),
                is_tls: true,
                request: String::new(),
                request_text: String::new(),
                response: String::new(),
                response_text: String::new(),
                response_headers: String::new(),
                status: RepeaterStatus::Ready,
                response_time_ms: 0,
            };
            *active_tab = 0;
        }
    }

    if *active_tab >= tabs.len() { *active_tab = 0; }
    let tab = &mut tabs[*active_tab];

    ui.add_space(4.0);

    // ── Target Address Toolbar ──────────────────────────────────────────────
    ui.horizontal(|ui| {
        ui.label(RichText::new("Host:").size(12.0).color(TEXT_1).strong());
        ui.add(egui::TextEdit::singleline(&mut tab.target_host).desired_width(160.0));

        ui.label(RichText::new("Port:").size(12.0).color(TEXT_1).strong());
        ui.add(egui::TextEdit::singleline(&mut tab.target_port).desired_width(50.0));

        // HTTPS Checkbox with Auto Port Toggle
        if ui.checkbox(&mut tab.is_tls, "HTTPS").changed() {
            if tab.is_tls {
                tab.target_port = "443".to_string();
            } else {
                tab.target_port = "80".to_string();
            }
        }

        ui.add_space(6.0);

        // Protocol Selection Dropdown Menu
        ui.label(RichText::new("Protocol:").size(12.0).color(TEXT_1).strong());
        let mut proto_changed = false;
        egui::ComboBox::from_id_source(ui.id().with("repeater_proto_combo"))
            .selected_text(RichText::new(&tab.protocol).size(11.0).strong())
            .show_ui(ui, |ui| {
                if ui.selectable_value(&mut tab.protocol, "HTTP/1.1".to_string(), "HTTP/1.1").changed() { proto_changed = true; }
                if ui.selectable_value(&mut tab.protocol, "HTTP/2".to_string(), "HTTP/2").changed() { proto_changed = true; }
                if ui.selectable_value(&mut tab.protocol, "HTTP/3".to_string(), "HTTP/3").changed() { proto_changed = true; }
            });

        if proto_changed {
            tab.request_text = update_request_first_line_protocol(&tab.request_text, &tab.protocol);
            tab.request = tab.request_text.clone();
        }

        ui.add_space(6.0);

        // REAL SEND BUTTON! Delegated to proxy::repeater_engine!
        if ui.add(
            egui::Button::new(RichText::new("▶ Send").size(12.0).color(TEXT_0).strong())
                .fill(ACCENT_BLUE)
                .rounding(Rounding::same(4.0))
        ).clicked() {
            crate::proxy::repeater_engine::execute_repeater_request(tab);
        }

        if tab.response_time_ms > 0 {
            ui.label(RichText::new(format!("{}ms", tab.response_time_ms)).size(11.0).color(ACCENT_GREEN).family(FontFamily::Monospace));
        }

        if let Some(msg) = toast_msg {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(RichText::new(format!("✓ {}", msg)).size(11.0).color(ACCENT_GREEN));
            });
        }
    });

    ui.add_space(4.0);

    let available_h = (ui.available_height() - 22.0).max(100.0);

    // ── Full-Height Request / Response Split View ───────────────────────────
    ui.columns(2, |cols| {
        // Request Panel
        section_frame().show(&mut cols[0], |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Request Buffer").size(12.0).color(TEXT_0).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new("Syntax Highlighting Active").size(10.0).color(ACCENT_BLUE));
                });
            });
            ui.separator();

            let mut layouter = |ui: &egui::Ui, string: &str, wrap_width: f32| {
                syntax::http_layouter(ui, string, wrap_width)
            };

            let req_editor = ScrollArea::vertical()
                .id_source("repeater_request_scroll")
                .min_scrolled_height(available_h - 32.0)
                .show(ui, |ui| {
                    ui.add_sized(
                        [ui.available_width(), available_h - 32.0],
                        egui::TextEdit::multiline(&mut tab.request_text)
                            .font(TextStyle::Monospace)
                            .layouter(&mut layouter)
                    )
                });

            req_editor.inner.context_menu(|ui| {
                if ui.button(RichText::new("🎯 Send to Intruder").size(12.0).color(ACCENT_AMBER).strong()).clicked() {
                    repeater_action = RepeaterAction::SendToIntruder(tab.target_host.clone(), tab.target_port.clone(), tab.request_text.clone(), tab.is_tls);
                    ui.close_menu();
                }
                ui.separator();
                if ui.button(RichText::new("📋 Copy as cURL").size(12.0).color(TEXT_0)).clicked() {
                    let curl = format!("curl -X GET 'https://{}/' -H 'Host: {}'", tab.target_host, tab.target_host);
                    ui.output_mut(|o| o.copied_text = curl);
                    ui.close_menu();
                }
                if ui.button(RichText::new("📄 Copy Raw Request").size(12.0).color(TEXT_0)).clicked() {
                    ui.output_mut(|o| o.copied_text = tab.request_text.clone());
                    ui.close_menu();
                }
            });
        });

        // Response Panel
        section_frame().show(&mut cols[1], |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Response Buffer").size(12.0).color(TEXT_0).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new("Syntax Highlighting Active").size(10.0).color(ACCENT_GREEN));
                });
            });
            ui.separator();

            let mut layouter = |ui: &egui::Ui, string: &str, wrap_width: f32| {
                syntax::http_layouter(ui, string, wrap_width)
            };

            let resp_editor = ScrollArea::vertical()
                .id_source("repeater_response_scroll")
                .show(ui, |ui| {
                    ui.add_sized(
                        [ui.available_width(), available_h - 32.0],
                        egui::TextEdit::multiline(&mut tab.response_text)
                            .font(TextStyle::Monospace)
                            .layouter(&mut layouter)
                    )
                });

            resp_editor.inner.context_menu(|ui| {
                if ui.button(RichText::new("📄 Copy Response").size(12.0).color(TEXT_0)).clicked() {
                    ui.output_mut(|o| o.copied_text = tab.response_text.clone());
                    ui.close_menu();
                }
            });
        });
    });

    repeater_action
}
