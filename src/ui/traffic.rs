use eframe::egui::{self, Color32, FontFamily, RichText, Stroke, TextStyle};
use crate::models::{HttpEntry, FilterState, HeaderInjectionRule, TrafficAction};
use crate::ui::syntax;
use std::fs::File;
use std::io::Write;

// ── Color Palette ─────────────────────────────────────────────────────────────
const BG_DARK: Color32 = Color32::from_rgb(18, 18, 20);
const BG_CARD: Color32 = Color32::from_rgb(26, 28, 36);

const ACCENT_BLUE: Color32 = Color32::from_rgb(2, 114, 176);
const ACCENT_CYAN: Color32 = Color32::from_rgb(0, 210, 255);
const ACCENT_GREEN: Color32 = Color32::from_rgb(74, 222, 128);
const ACCENT_AMBER: Color32 = Color32::from_rgb(251, 146, 60);
const ACCENT_PURPLE: Color32 = Color32::from_rgb(192, 132, 252);
const ACCENT_RED: Color32 = Color32::from_rgb(248, 113, 113);

const TEXT_0: Color32 = Color32::from_rgb(255, 255, 255);
const TEXT_1: Color32 = Color32::from_rgb(240, 246, 252);
const TEXT_2: Color32 = Color32::from_rgb(148, 163, 184);

// ── Main UI Function ──────────────────────────────────────────────────────────
pub fn render(
    ui: &mut egui::Ui,
    entries: &[HttpEntry],
    selected_id: &mut Option<usize>,
    filter_state: &mut FilterState,
    active_tab: &mut usize, // 0 = Request, 1 = Response, 2 = Split View
    _header_rules: &mut Vec<HeaderInjectionRule>,
    _show_header_panel: &mut bool,
    ctx: &egui::Context,
) -> TrafficAction {
    let mut action = TrafficAction::default();

    // ── Filter Captured Entries ───────────────────────────────────────────────
    let filtered_entries: Vec<&HttpEntry> = entries
        .iter()
        .filter(|e| {
            // 1. Hide zero-size response packets if checkbox enabled
            if filter_state.hide_zero_size && e.length == 0 {
                return false;
            }

            // 2. Host Filters (wildcard/substring search e.g. "target" matches *target*)
            if !filter_state.host_filters.is_empty() {
                let host_lower = e.host.to_lowercase();
                let matches_host = filter_state.host_filters.iter().any(|filter| {
                    host_lower.contains(&filter.to_lowercase())
                });
                if !matches_host {
                    return false;
                }
            }

            // 3. Search query filter
            if !filter_state.search_query.is_empty() {
                let q = filter_state.search_query.to_lowercase();
                let matches_q = e.host.to_lowercase().contains(&q)
                    || e.url.to_lowercase().contains(&q)
                    || e.method.to_lowercase().contains(&q)
                    || e.request_headers.to_lowercase().contains(&q)
                    || e.response_headers.to_lowercase().contains(&q);
                if !matches_q {
                    return false;
                }
            }

            true
        })
        .collect();

    ui.vertical(|ui| {
        // ── Top Toolbar ───────────────────────────────────────────────────────
        ui.horizontal(|ui| {
            ui.label(RichText::new("HTTP HISTORY").size(14.0).color(ACCENT_CYAN).strong());
            ui.label(RichText::new(format!("({}/{} shown)", filtered_entries.len(), entries.len())).size(11.0).color(TEXT_2));
            
            ui.add_space(10.0);

            // 🗑 Clear History Button
            if ui.add(egui::Button::new(RichText::new("🗑 Clear History").size(11.0).color(ACCENT_RED))).clicked() {
                action.clear_history = true;
                *selected_id = None;
            }

            // 📥 Export History Button
            if ui.add(egui::Button::new(RichText::new("📥 Export History").size(11.0).color(ACCENT_BLUE))).clicked() {
                filter_state.show_export_modal = true;
                filter_state.export_status_msg = String::new();
            }

            // Top Right: Send to Repeater Button (Always visible!)
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let has_sel = selected_id.is_some() && entries.iter().any(|e| selected_id.map(|id| e.id as usize == id).unwrap_or(false));
                let btn_color = if has_sel { ACCENT_GREEN } else { TEXT_2 };
                if ui.add_enabled(has_sel, egui::Button::new(RichText::new("🚀 Send to Repeater").size(11.0).color(btn_color))).clicked() {
                    if let Some(id) = *selected_id {
                        action.send_to_repeater = Some(id);
                    }
                }
            });
        });
        ui.add_space(4.0);

        // ── Action Bar / Filter Controls (Directly under Send to Repeater) ────
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Right-most: Add Host Filter Button (Directly under Send to Repeater)
                let filter_btn_label = if filter_state.host_filters.is_empty() {
                    "➕ Add Host Filter".to_string()
                } else {
                    format!("🎯 Host Filters ({} Active)", filter_state.host_filters.len())
                };

                if ui.button(RichText::new(filter_btn_label).size(11.0).color(if filter_state.host_filters.is_empty() { ACCENT_BLUE } else { ACCENT_AMBER })).clicked() {
                    filter_state.show_host_filter_modal = true;
                }

                ui.add_space(12.0);

                // Sol tarafında: Checkbox (Hide 0-byte responses)
                ui.checkbox(&mut filter_state.hide_zero_size, RichText::new("🚫 Hide 0-byte responses (Size = 0)").size(11.0).color(TEXT_1));
            });
        });
        ui.add_space(4.0);

        let has_selection = selected_id.is_some() && entries.iter().any(|e| selected_id.map(|id| e.id as usize == id).unwrap_or(false));
        let avail_h = ui.available_height();
        let table_max_h = if has_selection { (avail_h * 0.38).clamp(140.0, 240.0) } else { avail_h - 10.0 };

        // ── Traffic Table ─────────────────────────────────────────────────────
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

fn truncate_str(s: &str, max_chars: usize) -> String {
    if s.chars().count() > max_chars {
        let truncated: String = s.chars().take(max_chars.saturating_sub(3)).collect();
        format!("{}...", truncated)
    } else {
        s.to_string()
    }
}

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
                                render_full_cell(ui, RichText::new(truncate_str(&entry.host, 28)).size(11.0).color(TEXT_1), is_selected, entry, &mut action, selected_id);
                            });

                            // Col 4: Path
                            row.col(|ui| {
                                render_full_cell(ui, RichText::new(truncate_str(&entry.path, 45)).size(11.0).color(TEXT_2), is_selected, entry, &mut action, selected_id);
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
                                render_full_cell(ui, RichText::new(text).size(11.0).color(color).strong().family(FontFamily::Monospace), is_selected, entry, &mut action, selected_id);
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

                        egui::ScrollArea::vertical()
                            .id_source("traffic_inspector_scroll")
                            .max_height(ui.available_height() - 8.0)
                            .show(ui, |ui| {
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
                    });
            }
        }
    });

    // ── Export History Modal Dialog Window ─────────────────────────────────────
    if filter_state.show_export_modal {
        let mut is_open = filter_state.show_export_modal;
        egui::Window::new(RichText::new("📥 Export Traffic History Logs").size(14.0).color(TEXT_0).strong())
            .open(&mut is_open)
            .collapsible(false)
            .resizable(false)
            .default_size([500.0, 320.0])
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.add_space(4.0);
                ui.label(RichText::new(format!("Select export format and destination folder for {} captured requests:", entries.len())).size(11.0).color(TEXT_2));
                ui.separator();
                ui.add_space(8.0);

                // Option 1: JSON Export
                ui.horizontal(|ui| {
                    ui.label(RichText::new("1. JSON Format (.json)").size(12.0).color(ACCENT_CYAN).strong());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(RichText::new("📂 Choose Folder & Save JSON").size(11.0).color(ACCENT_GREEN)).clicked() {
                            if let Some(path) = rfd::FileDialog::new()
                                .set_title("Export AJProxy Traffic History (JSON)")
                                .set_file_name("ajproxy_history.json")
                                .add_filter("JSON File", &["json"])
                                .save_file()
                            {
                                let data = serde_json::to_string_pretty(entries).unwrap_or_default();
                                if let Ok(mut f) = File::create(&path) {
                                    let _ = f.write_all(data.as_bytes());
                                    filter_state.export_path = path.to_string_lossy().to_string();
                                    filter_state.export_status_msg = format!("✔ Saved: {}", filter_state.export_path);
                                }
                            }
                        }
                    });
                });
                ui.label(RichText::new("Full structured data including headers, bodies, timestamps, and status codes.").size(10.0).color(TEXT_2));

                ui.add_space(10.0);

                // Option 2: CSV Export
                ui.horizontal(|ui| {
                    ui.label(RichText::new("2. CSV Spreadsheet (.csv)").size(12.0).color(ACCENT_CYAN).strong());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(RichText::new("📂 Choose Folder & Save CSV").size(11.0).color(ACCENT_GREEN)).clicked() {
                            if let Some(path) = rfd::FileDialog::new()
                                .set_title("Export AJProxy Traffic History (CSV)")
                                .set_file_name("ajproxy_history.csv")
                                .add_filter("CSV File", &["csv"])
                                .save_file()
                            {
                                let mut csv_buf = String::from("ID,Timestamp,Method,Host,Path,Status,Length,DurationMs,URL\n");
                                for e in entries {
                                    csv_buf.push_str(&format!(
                                        "\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",\"{}\"\n",
                                        e.id, e.timestamp, e.method, e.host, e.path, e.status_code, e.length, e.duration_ms, e.url
                                    ));
                                }
                                if let Ok(mut f) = File::create(&path) {
                                    let _ = f.write_all(csv_buf.as_bytes());
                                    filter_state.export_path = path.to_string_lossy().to_string();
                                    filter_state.export_status_msg = format!("✔ Saved: {}", filter_state.export_path);
                                }
                            }
                        }
                    });
                });
                ui.label(RichText::new("Spreadsheet table format compatible with Excel, LibreOffice Calc, and Python.").size(10.0).color(TEXT_2));

                ui.add_space(10.0);

                // Option 3: Plain Text / Burp Log Export
                ui.horizontal(|ui| {
                    ui.label(RichText::new("3. Burp Raw Text Log (.log)").size(12.0).color(ACCENT_CYAN).strong());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(RichText::new("📂 Choose Folder & Save LOG").size(11.0).color(ACCENT_GREEN)).clicked() {
                            if let Some(path) = rfd::FileDialog::new()
                                .set_title("Export AJProxy Traffic History (LOG)")
                                .set_file_name("ajproxy_history.log")
                                .add_filter("Log File", &["log", "txt"])
                                .save_file()
                            {
                                let mut log_buf = String::new();
                                for e in entries {
                                    log_buf.push_str(&format!("========================================================================\n"));
                                    log_buf.push_str(&format!("HTTP ENTRY #{}: {} {} -> Status {}\n", e.id, e.method, e.url, e.status_code));
                                    log_buf.push_str(&format!("--- REQUEST ---\n{}\n\n{}", e.request_headers, e.request_body));
                                    log_buf.push_str(&format!("\n--- RESPONSE ---\n{}\n\n{}\n\n", e.response_headers, e.response_body));
                                }
                                if let Ok(mut f) = File::create(&path) {
                                    let _ = f.write_all(log_buf.as_bytes());
                                    filter_state.export_path = path.to_string_lossy().to_string();
                                    filter_state.export_status_msg = format!("✔ Saved: {}", filter_state.export_path);
                                }
                            }
                        }
                    });
                });
                ui.label(RichText::new("Raw HTTP request/response log format compatible with security tools.").size(10.0).color(TEXT_2));

                if !filter_state.export_status_msg.is_empty() {
                    ui.add_space(10.0);
                    ui.label(RichText::new(&filter_state.export_status_msg).size(11.0).color(ACCENT_GREEN).strong());
                }

                ui.add_space(12.0);
                ui.separator();
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(RichText::new("Close").size(11.0).color(TEXT_0)).clicked() {
                            filter_state.show_export_modal = false;
                        }
                    });
                });
            });
        filter_state.show_export_modal = is_open;
    }

    // ── Host Filter Configuration Modal Dialog Window ─────────────────────────
    if filter_state.show_host_filter_modal {
        let mut is_open = filter_state.show_host_filter_modal;
        egui::Window::new(RichText::new("🎯 Host Filter Rules").size(14.0).color(TEXT_0).strong())
            .open(&mut is_open)
            .collapsible(false)
            .resizable(false)
            .default_size([480.0, 300.0])
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.add_space(4.0);
                ui.label(RichText::new("Only show traffic matching specified domain keywords (e.g. 'target' matches *target*, 'api.example.com'):").size(11.0).color(TEXT_2));
                ui.add_space(8.0);

                ui.horizontal(|ui| {
                    ui.add(egui::TextEdit::singleline(&mut filter_state.new_host_filter_input).hint_text("Type keyword (e.g. target)...").desired_width(280.0));
                    if ui.button(RichText::new("➕ Add Host").size(11.0).color(ACCENT_GREEN)).clicked() {
                        let val = filter_state.new_host_filter_input.trim().to_string();
                        if !val.is_empty() && !filter_state.host_filters.contains(&val) {
                            filter_state.host_filters.push(val);
                            filter_state.new_host_filter_input.clear();
                        }
                    }
                });

                ui.add_space(10.0);
                ui.separator();
                ui.label(RichText::new(format!("Active Host Filters ({})", filter_state.host_filters.len())).size(12.0).color(ACCENT_CYAN).strong());
                ui.add_space(4.0);

                egui::ScrollArea::vertical()
                    .id_source("host_filters_scroll")
                    .max_height(130.0)
                    .show(ui, |ui| {
                        if filter_state.host_filters.is_empty() {
                            ui.label(RichText::new("No host filters active. Traffic from all hosts is visible.").size(11.0).color(TEXT_2));
                        } else {
                            let mut to_remove = None;
                            for (idx, filter) in filter_state.host_filters.iter().enumerate() {
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new(format!("• *{}*", filter)).size(12.0).color(ACCENT_GREEN).strong());
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        if ui.button(RichText::new("✖").size(10.0).color(ACCENT_RED)).clicked() {
                                            to_remove = Some(idx);
                                        }
                                    });
                                });
                            }
                            if let Some(idx) = to_remove {
                                filter_state.host_filters.remove(idx);
                            }
                        }
                    });

                ui.add_space(12.0);
                ui.separator();
                ui.horizontal(|ui| {
                    if !filter_state.host_filters.is_empty() {
                        if ui.button(RichText::new("🗑 Clear All Filters").size(11.0).color(ACCENT_AMBER)).clicked() {
                            filter_state.host_filters.clear();
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(RichText::new("Close").size(11.0).color(TEXT_0)).clicked() {
                            filter_state.show_host_filter_modal = false;
                        }
                    });
                });
            });
        filter_state.show_host_filter_modal = is_open;
    }

    action
}

// ── Helper to render a full-width selectable cell ────────────────────────────
fn render_full_cell(
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
            ui.label(rich_text);
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
