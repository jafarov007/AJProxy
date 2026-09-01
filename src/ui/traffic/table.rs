use eframe::egui::{self, Color32, FontFamily, RichText};
use crate::models::{HttpEntry, TrafficAction};

const BG_CARD: Color32 = Color32::from_rgb(26, 28, 36);

const ACCENT_BLUE: Color32 = Color32::from_rgb(2, 114, 176);
const ACCENT_CYAN: Color32 = Color32::from_rgb(0, 210, 255);
const ACCENT_GREEN: Color32 = Color32::from_rgb(74, 222, 128);
const ACCENT_AMBER: Color32 = Color32::from_rgb(251, 146, 60);
const ACCENT_PURPLE: Color32 = Color32::from_rgb(192, 132, 252);
const ACCENT_RED: Color32 = Color32::from_rgb(248, 113, 113);

const TEXT_1: Color32 = Color32::from_rgb(240, 246, 252);
const TEXT_2: Color32 = Color32::from_rgb(148, 163, 184);

pub fn truncate_str(s: &str, max_chars: usize) -> String {
    if s.chars().count() > max_chars {
        let truncated: String = s.chars().take(max_chars.saturating_sub(3)).collect();
        format!("{}...", truncated)
    } else {
        s.to_string()
    }
}

pub fn render_full_cell(
    ui: &mut egui::Ui,
    rich_text: RichText,
    is_selected: bool,
    entry: &HttpEntry,
    action: &mut TrafficAction,
    selected_id: &mut Option<usize>,
) {
    let (rect, response) = ui.allocate_exact_size(ui.available_size(), egui::Sense::click());

    if is_selected {
        ui.painter().rect_filled(rect, 0.0, Color32::from_rgb(15, 65, 100));
    } else if response.hovered() {
        ui.painter().rect_filled(rect, 0.0, Color32::from_rgb(32, 36, 48));
    }

    let response = response.on_hover_text(&entry.url);

    ui.allocate_ui_at_rect(rect, |ui| {
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            ui.add(egui::Label::new(rich_text).truncate(true));
        });
    });

    if response.clicked() {
        *selected_id = Some(entry.id as usize);
    }

    response.context_menu(|ui| {
        if ui.button("🚀 Send to Repeater").clicked() {
            action.send_to_repeater = Some(entry.id as usize);
            ui.close_menu();
        }
        if ui.button("🎯 Send to Intruder").clicked() {
            action.send_to_bruteforce = Some(entry.id as usize);
            ui.close_menu();
        }
        ui.separator();
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

pub fn render_traffic_table(
    ui: &mut egui::Ui,
    filtered_entries: &[&HttpEntry],
    selected_id: &mut Option<usize>,
    table_max_h: f32,
    action: &mut TrafficAction,
) {
    egui::Frame::none()
        .fill(BG_CARD)
        .rounding(egui::Rounding::same(6.0))
        .show(ui, |ui| {
            let table = egui_extras::TableBuilder::new(ui)
                .striped(true)
                .resizable(true)
                .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                .max_scroll_height(table_max_h)
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
                    body.rows(20.0, filtered_entries.len(), |mut row| {
                        let index = row.index();
                        let entry = filtered_entries[filtered_entries.len() - 1 - index]; // latest first
                        let entry_id = entry.id as usize;
                        let is_selected = *selected_id == Some(entry_id);

                        // Col 1: ID
                        row.col(|ui| {
                            render_full_cell(ui, RichText::new(format!("#{}", entry.id)).size(11.0).color(TEXT_2), is_selected, entry, action, selected_id);
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
                            render_full_cell(ui, RichText::new(&entry.method).size(10.0).color(badge_color).strong().family(FontFamily::Monospace), is_selected, entry, action, selected_id);
                        });

                        // Col 3: Host
                        row.col(|ui| {
                            render_full_cell(ui, RichText::new(truncate_str(&entry.host, 28)).size(11.0).color(TEXT_1), is_selected, entry, action, selected_id);
                        });

                        // Col 4: Path
                        row.col(|ui| {
                            render_full_cell(ui, RichText::new(truncate_str(&entry.path, 45)).size(11.0).color(TEXT_2), is_selected, entry, action, selected_id);
                        });

                        // Col 5: Status Code
                        row.col(|ui| {
                            let (color, text) = match entry.status_code {
                                100..=199 => (ACCENT_BLUE, entry.status_code.to_string()),
                                200..=299 => (ACCENT_GREEN, entry.status_code.to_string()),
                                300..=399 => (ACCENT_CYAN, entry.status_code.to_string()),
                                400..=499 => (ACCENT_AMBER, entry.status_code.to_string()),
                                500..=599 => (ACCENT_RED, entry.status_code.to_string()),
                                _ => (TEXT_2, if entry.status_code == 0 { "---".into() } else { entry.status_code.to_string() }),
                            };
                            render_full_cell(ui, RichText::new(text).size(11.0).color(color).strong().family(FontFamily::Monospace), is_selected, entry, action, selected_id);
                        });

                        // Col 6: Size
                        row.col(|ui| {
                            render_full_cell(ui, RichText::new(format!("{} B", entry.length)).size(10.0).color(TEXT_2), is_selected, entry, action, selected_id);
                        });

                        // Col 7: Duration
                        row.col(|ui| {
                            render_full_cell(ui, RichText::new(format!("{} ms", entry.duration_ms)).size(10.0).color(TEXT_2), is_selected, entry, action, selected_id);
                        });
                    });
                });
        });
}
