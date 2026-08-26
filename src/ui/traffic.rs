use eframe::egui::{self, Color32, FontFamily, RichText, Stroke, TextStyle};
use crate::models::{HttpEntry, FilterState, HeaderInjectionRule};
use crate::ui::syntax;

// ── Color Palette ─────────────────────────────────────────────────────────────
const BG_DARK: Color32 = Color32::from_rgb(18, 18, 20);
const BG_CARD: Color32 = Color32::from_rgb(26, 28, 36);

const ACCENT_BLUE: Color32 = Color32::from_rgb(2, 114, 176);
const ACCENT_CYAN: Color32 = Color32::from_rgb(0, 210, 255);
const ACCENT_GREEN: Color32 = Color32::from_rgb(74, 222, 128);
const ACCENT_AMBER: Color32 = Color32::from_rgb(251, 146, 60);
const ACCENT_PURPLE: Color32 = Color32::from_rgb(192, 132, 252);
const ACCENT_RED: Color32 = Color32::from_rgb(248, 113, 113);

const TEXT_1: Color32 = Color32::from_rgb(240, 246, 252);
const TEXT_2: Color32 = Color32::from_rgb(148, 163, 184);

// ── Action Struct ──────────────────────────────────────────────────────────────
pub struct TrafficAction {
    pub send_to_repeater: Option<usize>,
}

// ── Main UI Function ──────────────────────────────────────────────────────────
pub fn render(
    ui: &mut egui::Ui,
    entries: &[HttpEntry],
    selected_id: &mut Option<usize>,
    _filter_state: &mut FilterState,
    active_tab: &mut usize, // 0 = Request, 1 = Response, 2 = Split View
    _header_rules: &mut Vec<HeaderInjectionRule>,
    _show_header_panel: &mut bool,
) -> TrafficAction {
    let mut action = TrafficAction { send_to_repeater: None };

    ui.vertical(|ui| {
        // ── Top Toolbar ───────────────────────────────────────────────────────
        ui.horizontal(|ui| {
            ui.label(RichText::new("HTTP HISTORY").size(14.0).color(ACCENT_CYAN).strong());
            ui.label(RichText::new(format!("({} captured)", entries.len())).size(11.0).color(TEXT_2));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if let Some(id) = *selected_id {
                    if entries.iter().any(|e| e.id as usize == id) {
                        if ui.button(RichText::new("🚀 Send to Repeater").size(11.0).color(ACCENT_GREEN)).clicked() {
                            action.send_to_repeater = Some(id);
                        }
                    }
                }
            });
        });
        ui.add_space(4.0);

        // ── Traffic Table ─────────────────────────────────────────────────────
        egui::Frame::none()
            .fill(BG_CARD)
            .rounding(egui::Rounding::same(6.0))
            .show(ui, |ui| {
                let table = egui_extras::TableBuilder::new(ui)
                    .striped(true)
                    .resizable(true)
                    .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                    .column(egui_extras::Column::initial(45.0))  // ID
                    .column(egui_extras::Column::initial(70.0))  // Method Badge
                    .column(egui_extras::Column::initial(160.0)) // Host
                    .column(egui_extras::Column::initial(220.0)) // Path / URL
                    .column(egui_extras::Column::initial(55.0))  // Status
                    .column(egui_extras::Column::initial(80.0))  // Size
                    .column(egui_extras::Column::initial(65.0)); // Time

                table
                    .header(20.0, |mut header| {
                        header.col(|ui| { ui.label(RichText::new("ID").size(11.0).color(TEXT_2).strong()); });
                        header.col(|ui| { ui.label(RichText::new("Method").size(11.0).color(TEXT_2).strong()); });
                        header.col(|ui| { ui.label(RichText::new("Host").size(11.0).color(TEXT_2).strong()); });
                        header.col(|ui| { ui.label(RichText::new("Path").size(11.0).color(TEXT_2).strong()); });
                        header.col(|ui| { ui.label(RichText::new("Status").size(11.0).color(TEXT_2).strong()); });
                        header.col(|ui| { ui.label(RichText::new("Size").size(11.0).color(TEXT_2).strong()); });
                        header.col(|ui| { ui.label(RichText::new("Time").size(11.0).color(TEXT_2).strong()); });
                    })
                    .body(|body| {
                        body.rows(20.0, entries.len(), |mut row| {
                            let index = row.index();
                            let entry = &entries[entries.len() - 1 - index]; // latest first
                            let entry_id = entry.id as usize;
                            let is_selected = *selected_id == Some(entry_id);

                            // Col 1: ID
                            row.col(|ui| {
                                render_full_cell(ui, RichText::new(format!("#{}", entry.id)).size(11.0).color(TEXT_2), is_selected, entry, &mut action, selected_id);
                            });

                            // Col 2: HTTP Method Badge
                            row.col(|ui| {
                                let (badge_color, _bg_color) = match entry.method.as_str() {
                                    "GET" => (ACCENT_CYAN, Color32::from_rgb(6, 40, 60)),
                                    "POST" => (ACCENT_AMBER, Color32::from_rgb(50, 30, 10)),
                                    "PUT" | "PATCH" => (ACCENT_PURPLE, Color32::from_rgb(40, 20, 50)),
                                    "DELETE" => (ACCENT_RED, Color32::from_rgb(50, 10, 10)),
                                    "CONNECT" => (ACCENT_BLUE, Color32::from_rgb(10, 30, 50)),
                                    _ => (ACCENT_GREEN, Color32::from_rgb(10, 40, 20)),
                                };
                                render_full_cell(ui, RichText::new(&entry.method).size(10.0).color(badge_color).strong().family(FontFamily::Monospace), is_selected, entry, &mut action, selected_id);
                            });

                            // Col 3: Host
                            row.col(|ui| {
                                render_full_cell(ui, RichText::new(&entry.host).size(11.0).color(TEXT_1), is_selected, entry, &mut action, selected_id);
                            });

                            // Col 4: Path
                            row.col(|ui| {
                                render_full_cell(ui, RichText::new(&entry.path).size(11.0).color(TEXT_2), is_selected, entry, &mut action, selected_id);
                            });

                            // Col 5: Status
                            row.col(|ui| {
                                let color = match entry.status_code {
                                    200..=299 => ACCENT_GREEN,
                                    300..=399 => ACCENT_CYAN,
                                    400..=499 => ACCENT_AMBER,
                                    _ => ACCENT_RED,
                                };
                                render_full_cell(ui, RichText::new(entry.status_code.to_string()).size(11.0).color(color).strong(), is_selected, entry, &mut action, selected_id);
                            });

                            // Col 6: Size
                            row.col(|ui| {
                                render_full_cell(ui, RichText::new(format!("{} B", entry.length)).size(10.0).color(TEXT_2), is_selected, entry, &mut action, selected_id);
                            });

                            // Col 7: Duration
                            row.col(|ui| {
                                render_full_cell(ui, RichText::new(format!("{} ms", entry.duration_ms)).size(10.0).color(TEXT_2), is_selected, entry, &mut action, selected_id);
                            });
                        });
                    });
            });

        ui.add_space(6.0);

        // ── Bottom Inspector Panel ───────────────────────────────────────────
        if let Some(id) = *selected_id {
            if let Some(entry) = entries.iter().find(|e| e.id as usize == id) {
                egui::Frame::none()
                    .fill(BG_DARK)
                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(30, 40, 55)))
                    .rounding(egui::Rounding::same(6.0))
                    .inner_margin(egui::Margin::same(8.0))
                    .show(ui, |ui| {
                        // Tabs Header
                        ui.horizontal(|ui| {
                            ui.selectable_value(active_tab, 0, RichText::new("REQUEST").size(11.0).strong());
                            ui.selectable_value(active_tab, 1, RichText::new("RESPONSE").size(11.0).strong());
                            ui.selectable_value(active_tab, 2, RichText::new("SPLIT VIEW").size(11.0).strong());
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label(RichText::new(format!("URL: {}", entry.url)).size(10.0).color(ACCENT_CYAN));
                            });
                        });
                        ui.separator();

                        match *active_tab {
                            0 => render_inspector_section(ui, entry, true),
                            1 => render_inspector_section(ui, entry, false),
                            _ => {
                                // Split View: Left = Request, Right = Response
                                ui.columns(2, |cols| {
                                    render_inspector_section(&mut cols[0], entry, true);
                                    render_inspector_section(&mut cols[1], entry, false);
                                });
                            }
                        }
                    });
            }
        }
    });

    action
}

fn render_full_cell(
    ui: &mut egui::Ui,
    text: RichText,
    is_selected: bool,
    entry: &HttpEntry,
    action: &mut TrafficAction,
    selected_id: &mut Option<usize>,
) {
    let size = ui.available_size();
    let resp = ui.add_sized(size, egui::SelectableLabel::new(is_selected, text));
    if resp.clicked() {
        *selected_id = Some(entry.id as usize);
    }
    add_row_context_menu(&resp, entry, action, selected_id);
}

fn add_row_context_menu(
    response: &egui::Response,
    entry: &HttpEntry,
    action: &mut TrafficAction,
    selected_id: &mut Option<usize>,
) {
    let entry_id = entry.id as usize;
    response.context_menu(|ui| {
        *selected_id = Some(entry_id);
        if ui.button("🚀 Send to Repeater").clicked() {
            action.send_to_repeater = Some(entry_id);
            ui.close_menu();
        }
        if ui.button("📋 Copy URL").clicked() {
            ui.output_mut(|o| o.copied_text = entry.url.clone());
            ui.close_menu();
        }
        if ui.button("📋 Copy Request").clicked() {
            let req = format!("{} {} HTTP/1.1\r\n{}\r\n\r\n{}", entry.method, entry.path, entry.request_headers, entry.request_body);
            ui.output_mut(|o| o.copied_text = req);
            ui.close_menu();
        }
    });
}

// ── Raw Burp Suite Style Inspector Renderer (With Syntax Highlighting) ─────────
pub fn render_inspector_section(ui: &mut egui::Ui, entry: &HttpEntry, is_request: bool) {
    ui.vertical(|ui| {
        let raw_text = if is_request {
            let mut text = String::new();
            if !entry.request_headers.starts_with(&entry.method) {
                text.push_str(&format!("{} {} HTTP/1.1\r\n", entry.method, entry.path));
            }
            text.push_str(&entry.request_headers);
            if !text.ends_with("\r\n\r\n") && !text.ends_with("\n\n") {
                text.push_str("\r\n\r\n");
            }
            text.push_str(&entry.request_body);
            text
        } else {
            let mut text = String::new();
            text.push_str(&entry.response_headers);
            if !text.ends_with("\r\n\r\n") && !text.ends_with("\n\n") {
                text.push_str("\r\n\r\n");
            }
            text.push_str(&entry.response_body);
            text
        };

        let mut display_str = raw_text.as_str();
        let mut layouter = |ui: &egui::Ui, string: &str, wrap_width: f32| {
            syntax::http_layouter(ui, string, wrap_width)
        };

        ui.add(
            egui::TextEdit::multiline(&mut display_str)
                .font(TextStyle::Monospace)
                .layouter(&mut layouter)
                .desired_width(f32::INFINITY)
                .desired_rows(18)
        );
    });
}
