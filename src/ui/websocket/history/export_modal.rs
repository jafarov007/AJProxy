use std::fs::File;
use std::io::Write;
use egui::{self, RichText};
use serde::Serialize;
use crate::models::*;
use crate::theme::*;

pub fn render_export_modal(
    ctx: &egui::Context,
    state: &mut WsHistoryState,
    connections: &[WsConnection],
    frames: &[WsFrameEntry],
) {
    if !state.show_export_modal {
        return;
    }

    let mut open = true;
    let mut export_saved_msg: Option<String> = None;
    let mut close_clicked = false;

    egui::Window::new("📤 Export WebSocket History Data")
        .open(&mut open)
        .resizable(false)
        .collapsible(false)
        .default_width(450.0)
        .show(ctx, |ui| {
            ui.add_space(6.0);
            ui.label(RichText::new("Choose an export format to save your captured WebSocket session data:").size(11.0).color(TEXT_1));
            ui.add_space(10.0);

            // Option 1: JSON Export
            ui.horizontal(|ui| {
                ui.label(RichText::new("1. Full JSON Dump (.json)").size(12.0).color(ACCENT_CYAN).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(RichText::new("📂 Save JSON").size(11.0).color(ACCENT_GREEN)).clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .set_title("Export AJProxy WebSocket History (JSON)")
                            .set_file_name("ajproxy_ws_history.json")
                            .add_filter("JSON File", &["json"])
                            .save_file()
                        {
                            #[derive(Serialize)]
                            struct WsExportData<'a> {
                                connections: &'a [WsConnection],
                                frames: &'a [WsFrameEntry],
                            }
                            let export_obj = WsExportData { connections, frames };
                            let data = serde_json::to_string_pretty(&export_obj).unwrap_or_default();
                            if let Ok(mut f) = File::create(&path) {
                                let _ = f.write_all(data.as_bytes());
                                export_saved_msg = Some(format!("✔ Saved: {}", path.to_string_lossy()));
                            }
                        }
                    }
                });
            });
            ui.label(RichText::new("Structured JSON data including connection metadata and frame payloads.").size(10.0).color(TEXT_2));

            ui.add_space(10.0);

            // Option 2: CSV Export
            ui.horizontal(|ui| {
                ui.label(RichText::new("2. CSV Spreadsheet (.csv)").size(12.0).color(ACCENT_CYAN).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(RichText::new("📂 Save CSV").size(11.0).color(ACCENT_GREEN)).clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .set_title("Export AJProxy WebSocket History (CSV)")
                            .set_file_name("ajproxy_ws_history.csv")
                            .add_filter("CSV File", &["csv"])
                            .save_file()
                        {
                            let mut csv_buf = String::from("FrameID,Timestamp,ConnID,Direction,Opcode,Length,Payload\n");
                            for f_entry in frames {
                                let dir_str = match f_entry.direction {
                                    WsDirection::ClientToServer => "Client->Server",
                                    WsDirection::ServerToClient => "Server->Client",
                                };
                                let esc_payload = f_entry.payload.replace('"', "\"\"");
                                csv_buf.push_str(&format!(
                                    "\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",\"{}\"\n",
                                    f_entry.id, f_entry.timestamp, f_entry.connection_id, dir_str, f_entry.opcode.label(), f_entry.length, esc_payload
                                ));
                            }
                            if let Ok(mut f) = File::create(&path) {
                                let _ = f.write_all(csv_buf.as_bytes());
                                export_saved_msg = Some(format!("✔ Saved: {}", path.to_string_lossy()));
                            }
                        }
                    }
                });
            });
            ui.label(RichText::new("Spreadsheet table format compatible with Excel and security analysis scripts.").size(10.0).color(TEXT_2));

            ui.add_space(10.0);

            // Option 3: Plain Text Log Export
            ui.horizontal(|ui| {
                ui.label(RichText::new("3. Raw Text Log (.log)").size(12.0).color(ACCENT_CYAN).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(RichText::new("📂 Save LOG").size(11.0).color(ACCENT_GREEN)).clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .set_title("Export AJProxy WebSocket History (LOG)")
                            .set_file_name("ajproxy_ws_history.log")
                            .add_filter("Log File", &["log", "txt"])
                            .save_file()
                        {
                            let mut log_buf = String::new();
                            for conn in connections {
                                log_buf.push_str("========================================================================\n");
                                log_buf.push_str(&format!("WEBSOCKET TUNNEL #{}: {} ({})\n", conn.id, conn.url, conn.status));
                                log_buf.push_str(&format!("Connected At: {}\n\n", conn.connected_at));
                            }
                            log_buf.push_str(&format!("--- WEBSOCKET FRAMES LOG ({}) ---\n", frames.len()));
                            for f_entry in frames {
                                let dir_str = match f_entry.direction {
                                    WsDirection::ClientToServer => "⬆️ Client -> Server",
                                    WsDirection::ServerToClient => "⬇️ Server -> Client",
                                };
                                log_buf.push_str(&format!(
                                    "[{}] Conn #{} {} | Opcode: {} | Length: {} B\n{}\n\n",
                                    f_entry.timestamp, f_entry.connection_id, dir_str, f_entry.opcode.label(), f_entry.length, f_entry.payload
                                ));
                            }
                            if let Ok(mut f) = File::create(&path) {
                                let _ = f.write_all(log_buf.as_bytes());
                                export_saved_msg = Some(format!("✔ Saved: {}", path.to_string_lossy()));
                            }
                        }
                    }
                });
            });
            ui.label(RichText::new("Formatted text log suitable for penetration testing reports.").size(10.0).color(TEXT_2));

            if !state.export_status_msg.is_empty() {
                ui.add_space(10.0);
                ui.label(RichText::new(&state.export_status_msg).size(11.0).color(ACCENT_GREEN).strong());
            }

            ui.add_space(12.0);
            ui.separator();
            ui.add_space(6.0);

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(RichText::new("Close").size(11.0)).clicked() {
                    close_clicked = true;
                }
            });
        });

    if let Some(msg) = export_saved_msg {
        state.export_status_msg = msg;
    }
    if !open || close_clicked {
        state.show_export_modal = false;
    }
}
