use egui::{self, RichText, Rounding, Stroke, ScrollArea, TextStyle, Color32, FontFamily};
use crate::models::*;
use crate::theme::*;
use crate::ui::syntax;
use crate::proxy::listener::{self, InterceptDecision, PendingIntercept};

const BG_CARD: Color32 = Color32::from_rgb(26, 28, 36);
const BG_DARK: Color32 = Color32::from_rgb(18, 18, 20);
const ACCENT_PURPLE: Color32 = Color32::from_rgb(192, 132, 252);

pub enum InterceptUIAction {
    None,
    SendToRepeater(String, String, String, bool), // host, port, raw_req, is_tls
    SendToIntruder(String, String, String, bool), // host, port, raw_req, is_tls
}

pub fn render(
    ui: &mut egui::Ui,
    state: &mut InterceptState,
    settings: &mut AppSettings,
    ctx: &egui::Context,
) -> InterceptUIAction {
    let mut ui_action = InterceptUIAction::None;

    // Sync live pending requests held in proxy listener thread
    let pending_list = listener::get_pending_intercepts();
    state.queue_count = pending_list.len();

    // Auto-select first pending request if selection is invalid or empty
    if let Some(id) = state.selected_paused_id {
        if !pending_list.iter().any(|item| item.id == id) {
            state.selected_paused_id = pending_list.first().map(|i| i.id);
        }
    } else {
        state.selected_paused_id = pending_list.first().map(|i| i.id);
    }

    // ── Top Toolbar: Toggle Bar & Rules/Settings Dialog Trigger ──
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

        ui.add_space(8.0);

        // ⚙ Intercept Rules & Scope button
        if ui.add(
            egui::Button::new(RichText::new("⚙ Intercept Rules & Scope").size(12.0).color(ACCENT_BLUE).strong())
                .fill(BG_RAISED)
                .stroke(Stroke::new(1.0_f32, ACCENT_BLUE))
                .rounding(Rounding::same(4.0))
        ).clicked() {
            state.show_rules_modal = true;
        }

        if state.queue_count > 0 {
            ui.add_space(14.0);
            ui.label(
                RichText::new(format!("⚠ {} request(s) paused in queue", state.queue_count))
                    .size(12.0)
                    .color(ACCENT_AMBER)
                    .strong(),
            );

            ui.add_space(8.0);
            // ▶▶ Forward All Button
            if ui.add(egui::Button::new(RichText::new("▶▶ Forward All").size(11.0).color(ACCENT_GREEN))).clicked() {
                for item in &pending_list {
                    listener::resolve_pending_intercept(item.id, InterceptDecision::Forward);
                }
            }
        }
    });

    ui.add_space(6.0);

    // ── Main Workspace ──────────────────────────────────────────────────────────
    if pending_list.is_empty() {
        section_frame().show(ui, |ui| {
            ui.add_space(80.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new("Waiting for intercepted HTTP requests...").size(13.0).color(TEXT_2));
                ui.add_space(4.0);
                ui.label(RichText::new("Requests will pause and be listed here in real-time when Intercept IS ON.").size(11.0).color(TEXT_2));
            });
            ui.add_space(80.0);
        });
    } else {
        let avail_h = ui.available_height();
        let table_max_h = (avail_h * 0.35).clamp(120.0, 200.0);

        // ── Paused Requests Table ─────────────────────────────────────────────
        egui::Frame::none()
            .fill(BG_CARD)
            .rounding(Rounding::same(6.0))
            .show(ui, |ui| {
                let table = egui_extras::TableBuilder::new(ui)
                    .striped(true)
                    .resizable(true)
                    .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                    .max_scroll_height(table_max_h)
                    .column(egui_extras::Column::initial(45.0))  // ID
                    .column(egui_extras::Column::initial(65.0))  // Method
                    .column(egui_extras::Column::initial(160.0)) // Host
                    .column(egui_extras::Column::initial(220.0)) // Path / URL
                    .column(egui_extras::Column::initial(65.0))  // Status
                    .column(egui_extras::Column::initial(140.0)); // Quick Actions

                table
                    .header(20.0, |mut header| {
                        header.col(|ui| { ui.label(RichText::new("ID").size(11.0).color(TEXT_2).strong()); });
                        header.col(|ui| { ui.label(RichText::new("Method").size(11.0).color(TEXT_2).strong()); });
                        header.col(|ui| { ui.label(RichText::new("Host").size(11.0).color(TEXT_2).strong()); });
                        header.col(|ui| { ui.label(RichText::new("Path").size(11.0).color(TEXT_2).strong()); });
                        header.col(|ui| { ui.label(RichText::new("Status").size(11.0).color(TEXT_2).strong()); });
                        header.col(|ui| { ui.label(RichText::new("Quick Actions").size(11.0).color(TEXT_2).strong()); });
                    })
                    .body(|body| {
                        body.rows(20.0, pending_list.len(), |mut row| {
                            let idx = row.index();
                            let item = &pending_list[idx];
                            let is_selected = state.selected_paused_id == Some(item.id);

                            // Col 1: ID
                            row.col(|ui| {
                                if let Some(act) = render_paused_cell(ui, RichText::new(format!("#{}", item.id)).size(11.0).color(TEXT_2), is_selected, item, state) {
                                    ui_action = act;
                                }
                            });

                            // Col 2: Method
                            row.col(|ui| {
                                let badge_color = match item.method.as_str() {
                                    "GET" => ACCENT_CYAN,
                                    "POST" => ACCENT_AMBER,
                                    "PUT" | "PATCH" => ACCENT_PURPLE,
                                    "DELETE" => ACCENT_RED,
                                    _ => ACCENT_GREEN,
                                };
                                if let Some(act) = render_paused_cell(ui, RichText::new(&item.method).size(10.0).color(badge_color).strong().family(FontFamily::Monospace), is_selected, item, state) {
                                    ui_action = act;
                                }
                            });

fn truncate_intercept_str(s: &str, max_chars: usize) -> String {
    if s.chars().count() > max_chars {
        let truncated: String = s.chars().take(max_chars.saturating_sub(3)).collect();
        format!("{}...", truncated)
    } else {
        s.to_string()
    }
}

                            // Col 3: Host
                            row.col(|ui| {
                                if let Some(act) = render_paused_cell(ui, RichText::new(truncate_intercept_str(&item.host, 28)).size(11.0).color(TEXT_1), is_selected, item, state) {
                                    ui_action = act;
                                }
                            });

                            // Col 4: Path
                            row.col(|ui| {
                                if let Some(act) = render_paused_cell(ui, RichText::new(truncate_intercept_str(&item.path, 45)).size(11.0).color(TEXT_2), is_selected, item, state) {
                                    ui_action = act;
                                }
                            });

                            // Col 5: Status
                            row.col(|ui| {
                                if let Some(act) = render_paused_cell(ui, RichText::new("PAUSED").size(10.0).color(ACCENT_AMBER).strong(), is_selected, item, state) {
                                    ui_action = act;
                                }
                            });

                            // Col 6: Quick Action Buttons (Forward / Drop)
                            row.col(|ui| {
                                ui.horizontal(|ui| {
                                    if ui.add(egui::Button::new(RichText::new("▶ Forward").size(10.0).color(TEXT_0).strong()).fill(ACCENT_GREEN)).clicked() {
                                        listener::resolve_pending_intercept(item.id, InterceptDecision::Forward);
                                    }
                                    if ui.add(egui::Button::new(RichText::new("✖ Drop").size(10.0).color(TEXT_0).strong()).fill(ACCENT_RED)).clicked() {
                                        listener::resolve_pending_intercept(item.id, InterceptDecision::Drop);
                                    }
                                });
                            });
                        });
                    });
            });

        ui.add_space(6.0);

        // ── Inspector & Action Buttons for Selected Paused Request ────────────
        if let Some(selected_id) = state.selected_paused_id {
            if let Some(item) = pending_list.iter().find(|i| i.id == selected_id) {
                egui::Frame::none()
                    .fill(BG_DARK)
                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(30, 40, 55)))
                    .rounding(Rounding::same(6.0))
                    .inner_margin(egui::Margin::same(8.0))
                    .show(ui, |ui| {
                        // Title bar & Action buttons
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(format!("SELECTED REQUEST #{}: {} {}", item.id, item.method, item.url)).size(11.0).color(ACCENT_CYAN).strong().family(FontFamily::Monospace));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.add(egui::Button::new(RichText::new("🚀 Send to Repeater").size(11.0).color(ACCENT_BLUE))).clicked() {
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
                                ui.add_space(4.0);
                                if ui.add(egui::Button::new(RichText::new("🎯 Send to Intruder").size(11.0).color(ACCENT_AMBER))).clicked() {
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

                                    ui_action = InterceptUIAction::SendToIntruder(item.host.clone(), port.to_string(), raw_full, is_tls);
                                }
                                ui.add_space(4.0);
                                if ui.add(egui::Button::new(RichText::new("✖ Drop").size(11.0).color(TEXT_0).strong()).fill(ACCENT_RED)).clicked() {
                                    listener::resolve_pending_intercept(item.id, InterceptDecision::Drop);
                                }
                                ui.add_space(4.0);
                                if ui.add(egui::Button::new(RichText::new("▶ Forward").size(11.0).color(TEXT_0).strong()).fill(ACCENT_GREEN)).clicked() {
                                    listener::resolve_pending_intercept(item.id, InterceptDecision::Forward);
                                }
                            });
                        });
                        ui.separator();

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
                            .id_source("intercept_paused_inspector_scroll")
                            .max_height(ui.available_height() - 6.0)
                            .show(ui, |ui| {
                                ui.add(
                                    egui::TextEdit::multiline(&mut display_str)
                                        .font(TextStyle::Monospace)
                                        .layouter(&mut layouter)
                                        .desired_width(f32::INFINITY)
                                );
                            });
                    });
            }
        }
    }

    // ── Intercept Rules & Scope Settings Modal Window ───────────────
    if state.show_rules_modal {
        let mut is_open = state.show_rules_modal;
        egui::Window::new(RichText::new("⚙ Intercept Rules & Scope Settings").size(14.0).color(TEXT_0).strong())
            .open(&mut is_open)
            .collapsible(false)
            .resizable(true)
            .default_size([580.0, 420.0])
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.add_space(4.0);

                // Section 1: Real-time Interception Switches
                ui.label(RichText::new("Interception Rules").size(12.0).color(ACCENT_CYAN).strong());
                ui.separator();
                if ui.checkbox(&mut settings.intercept_requests, "Intercept HTTP Requests in real-time").changed() {
                    listener::set_intercept_enabled(settings.intercept_requests);
                }
                ui.checkbox(&mut settings.intercept_responses, "Intercept HTTP Responses in real-time");

                ui.add_space(10.0);

                // Section 2: SSL Passthrough Hosts
                ui.label(RichText::new("SSL Passthrough Hosts (comma separated):").size(12.0).color(ACCENT_CYAN).strong());
                ui.label(RichText::new("Bypass SSL MITM decryption for matched domains (e.g. *.google.com, banking.com)").size(10.0).color(TEXT_2));
                ui.add(egui::TextEdit::singleline(&mut settings.passthrough_hosts).desired_width(f32::INFINITY));

                ui.add_space(10.0);

                // Section 3: Notice pointing to Global Settings
                ui.separator();
                ui.add_space(4.0);
                ui.label(RichText::new("💡 Match & Replace Engine").size(12.0).color(ACCENT_CYAN).strong());
                ui.label(RichText::new("Match & Replace rules now run globally across all proxy traffic and are managed in the Settings tab under 'Global Match & Replace Engine'.").size(11.0).color(TEXT_2));

                ui.add_space(12.0);
                ui.separator();
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(RichText::new("Close").size(12.0).color(Color32::WHITE)).clicked() {
                            state.show_rules_modal = false;
                        }
                    });
                });
            });
        state.show_rules_modal = is_open;
    }

    ui_action
}

// ── Helper to render a full-width selectable paused cell with Context Menu ────
fn render_paused_cell(
    ui: &mut egui::Ui,
    rich_text: RichText,
    is_selected: bool,
    item: &PendingIntercept,
    state: &mut InterceptState,
) -> Option<InterceptUIAction> {
    let mut ui_action = None;
    let (rect, response) = ui.allocate_exact_size(ui.available_size(), egui::Sense::click());

    if is_selected {
        ui.painter().rect_filled(rect, 0.0, Color32::from_rgb(15, 65, 100));
    } else if response.hovered() {
        ui.painter().rect_filled(rect, 0.0, Color32::from_rgb(32, 36, 48));
    }

    let response = response.on_hover_text(&item.url);

    ui.allocate_ui_at_rect(rect, |ui| {
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            ui.label(rich_text);
        });
    });

    if response.clicked() {
        state.selected_paused_id = Some(item.id);
    }

    // ── Right-Click Context Menu on Table Rows ────────────────────────────────
    response.context_menu(|ui| {
        if ui.button("▶ Forward Request").clicked() {
            listener::resolve_pending_intercept(item.id, InterceptDecision::Forward);
            ui.close_menu();
        }
        if ui.button("✖ Drop Request").clicked() {
            listener::resolve_pending_intercept(item.id, InterceptDecision::Drop);
            ui.close_menu();
        }
        ui.separator();
        if ui.button("🚀 Send to Repeater").clicked() {
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

            ui_action = Some(InterceptUIAction::SendToRepeater(item.host.clone(), port.to_string(), raw_full, is_tls));
            ui.close_menu();
        }
    });

    ui_action
}
