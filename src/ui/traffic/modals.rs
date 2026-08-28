use eframe::egui::{self, RichText};
use crate::models::{HttpEntry, FilterState};
use std::fs::File;
use std::io::Write;

const TEXT_0: eframe::egui::Color32 = eframe::egui::Color32::from_rgb(255, 255, 255);
const TEXT_2: eframe::egui::Color32 = eframe::egui::Color32::from_rgb(148, 163, 184);
const ACCENT_CYAN: eframe::egui::Color32 = eframe::egui::Color32::from_rgb(0, 210, 255);
const ACCENT_GREEN: eframe::egui::Color32 = eframe::egui::Color32::from_rgb(74, 222, 128);
const ACCENT_AMBER: eframe::egui::Color32 = eframe::egui::Color32::from_rgb(251, 146, 60);
const ACCENT_RED: eframe::egui::Color32 = eframe::egui::Color32::from_rgb(248, 113, 113);

pub fn render_export_modal(ctx: &egui::Context, filter_state: &mut FilterState, entries: &[HttpEntry]) {
    if !filter_state.show_export_modal {
        return;
    }

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

pub fn render_host_filter_modal(ctx: &egui::Context, filter_state: &mut FilterState) {
    if !filter_state.show_host_filter_modal {
        return;
    }

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
