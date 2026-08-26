use egui::{self, RichText, Rounding, Stroke, ScrollArea, TextStyle};
use crate::models::*;
use crate::theme::*;
use crate::ui::syntax;
use crate::proxy::listener::{self, InterceptDecision};

pub enum InterceptUIAction {
    None,
    SendToRepeater(String, String, String, bool), // host, port, raw_req, is_tls
}

pub fn render(ui: &mut egui::Ui, state: &mut InterceptState) -> InterceptUIAction {
    let mut ui_action = InterceptUIAction::None;

    // Sync live pending requests held in proxy listener thread
    let pending_list = listener::get_pending_intercepts();
    state.queue_count = pending_list.len();

    // ── Toggle bar ────────────────────────────────────────────────
    ui.horizontal(|ui| {
        let (label, color) = if state.enabled {
            ("● Intercept IS ON", ACCENT_GREEN)
        } else {
            ("○ Intercept IS OFF", TEXT_2)
        };

        if ui.add(
            egui::Button::new(RichText::new(label).size(12.0).color(color).strong())
                .fill(BG_RAISED)
                .stroke(Stroke::new(1.0_f32, color))
                .rounding(Rounding::same(4.0))
        ).clicked() {
            state.enabled = !state.enabled;
            listener::set_intercept_enabled(state.enabled);
        }

        if state.queue_count > 0 {
            ui.add_space(14.0);
            ui.label(
                RichText::new(format!("⚠ {} request(s) paused & waiting in queue", state.queue_count))
                    .size(12.0)
                    .color(ACCENT_AMBER)
                    .strong(),
            );
        }
    });

    ui.add_space(6.0);

    // ── Intercepted message workspace ─────────────────────────────
    section_frame().show(ui, |ui| {
        if let Some(item) = pending_list.first() {
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("PAUSED REQUEST #{}: {} {}", item.id, item.method, item.url)).size(12.0).color(ACCENT_CYAN).strong().family(egui::FontFamily::Monospace));
            });
            ui.separator();

            // Action buttons: Forward, Drop, Send to Repeater
            ui.horizontal(|ui| {
                if ui.add(egui::Button::new(RichText::new("▶ Forward").size(12.0).color(TEXT_0).strong()).fill(ACCENT_GREEN)).clicked() {
                    listener::resolve_pending_intercept(item.id, InterceptDecision::Forward);
                }
                if ui.add(egui::Button::new(RichText::new("✖ Drop").size(12.0).color(TEXT_0).strong()).fill(ACCENT_RED)).clicked() {
                    listener::resolve_pending_intercept(item.id, InterceptDecision::Drop);
                }
                ui.add_space(10.0);
                if ui.add(egui::Button::new(RichText::new("🚀 Send to Repeater").size(12.0).color(ACCENT_BLUE).strong())).clicked() {
                    let is_tls = item.url.starts_with("https");
                    let port = if is_tls { "443" } else { "80" };
                    let mut raw_full = String::new();
                    if !item.headers.starts_with(&item.method) {
                        raw_full.push_str(&format!("{} {} HTTP/1.1\r\n", item.method, item.path));
                    }
                    raw_full.push_str(&item.headers);
                    if !raw_full.ends_with("\r\n\r\n") && !raw_full.ends_with("\n\n") {
                        raw_full.push_str("\r\n\r\n");
                    }
                    raw_full.push_str(&item.body);

                    ui_action = InterceptUIAction::SendToRepeater(item.host.clone(), port.to_string(), raw_full, is_tls);
                }
            });

            ui.add_space(6.0);

            let mut layouter = |ui: &egui::Ui, string: &str, wrap_width: f32| {
                syntax::http_layouter(ui, string, wrap_width)
            };

            let mut raw_full = String::new();
            if !item.headers.starts_with(&item.method) {
                raw_full.push_str(&format!("{} {} HTTP/1.1\r\n", item.method, item.path));
            }
            raw_full.push_str(&item.headers);
            if !raw_full.ends_with("\r\n\r\n") && !raw_full.ends_with("\n\n") {
                raw_full.push_str("\r\n\r\n");
            }
            raw_full.push_str(&item.body);

            let mut display_str = raw_full.as_str();

            ScrollArea::vertical()
                .id_source("intercept_request_scroll")
                .show(ui, |ui| {
                    ui.add(
                        egui::TextEdit::multiline(&mut display_str)
                            .font(TextStyle::Monospace)
                            .layouter(&mut layouter)
                            .desired_width(f32::INFINITY)
                    );
                });
        } else {
            ui.add_space(60.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new("Waiting for intercepted HTTP request...").size(13.0).color(TEXT_2));
                ui.add_space(4.0);
                ui.label(RichText::new("Requests will pause here in real-time when Intercept IS ON.").size(11.0).color(TEXT_2));
            });
            ui.add_space(60.0);
        }
    });

    ui.add_space(6.0);

    // ── Match & Replace Rules ─────────────────────────────────────
    section_frame().show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Match & Replace Rules").size(12.0).color(TEXT_0).strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(RichText::new("+ Add Rule").size(11.0).color(ACCENT_BLUE)).clicked() {
                    state.match_rules.push(InterceptRule {
                        enabled: true,
                        match_type: "Header".into(),
                        pattern: "".into(),
                        action: "".into(),
                    });
                }
            });
        });

        if !state.match_rules.is_empty() {
            ui.separator();
            let mut to_delete = None;
            for (idx, rule) in state.match_rules.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.checkbox(&mut rule.enabled, "");
                    ui.label(RichText::new("Match:").size(11.0).color(TEXT_2));
                    ui.add(egui::TextEdit::singleline(&mut rule.pattern).desired_width(200.0));
                    ui.label(RichText::new("Replace:").size(11.0).color(TEXT_2));
                    ui.add(egui::TextEdit::singleline(&mut rule.action).desired_width(200.0));

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(RichText::new("✖").size(11.0).color(ACCENT_RED)).clicked() {
                            to_delete = Some(idx);
                        }
                    });
                });
            }
            if let Some(idx) = to_delete {
                state.match_rules.remove(idx);
            }
        }
        listener::update_match_rules(state.match_rules.clone());
    });

    ui_action
}
