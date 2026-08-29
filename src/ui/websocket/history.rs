use egui::{self, RichText, Color32, ScrollArea, Rounding, FontFamily};
use crate::models::*;
use crate::theme::*;
use std::fs::File;
use std::io::Write;

pub fn render(
    ui: &mut egui::Ui,
    state: &mut WsHistoryState,
    connections: &[WsConnection],
    frames: &[WsFrameEntry],
    repeater_tabs: &mut Vec<WsRepeaterTab>,
    active_repeater_tab: &mut usize,
    ws_sub_tab: &mut WsSubTab,
) {
    // ── Render WS Export Modal if requested ──
    render_ws_export_modal(ui.ctx(), state, connections, frames);

    egui::SidePanel::left("ws_history_connections_panel")
        .default_width(260.0)
        .width_range(200.0..=360.0)
        .show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("WS Connections").size(12.0).color(TEXT_0).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if state.selected_connection_id.is_some() {
                        if ui.add(egui::Button::new(RichText::new("Show All").size(10.0).color(ACCENT_BLUE))).clicked() {
                            state.selected_connection_id = None;
                        }
                    }
                });
            });

            ui.add_space(4.0);
            ui.separator();
            ui.add_space(4.0);

            ScrollArea::vertical()
                .id_source("ws_conn_scroll")
                .show(ui, |ui| {
                    if connections.is_empty() {
                        ui.add_space(20.0);
                        ui.label(RichText::new("No WebSocket connections captured yet.").size(11.0).color(TEXT_2));
                    } else {
                        for conn in connections {
                            let is_selected = state.selected_connection_id == Some(conn.id);
                            let bg = if is_selected { BG_RAISED } else { Color32::TRANSPARENT };

                            egui::Frame::none()
                                .fill(bg)
                                .rounding(Rounding::same(4.0))
                                .inner_margin(egui::Margin::same(6.0))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        let status_color = if conn.status.starts_with("Active") { ACCENT_GREEN } else { TEXT_2 };
                                        ui.label(RichText::new("●").size(10.0).color(status_color));

                                        if ui.add(
                                            egui::SelectableLabel::new(
                                                is_selected,
                                                RichText::new(format!("WS #{} - {}", conn.id, conn.host))
                                                    .size(11.0)
                                                    .color(TEXT_0)
                                                    .strong()
                                            )
                                        ).clicked() {
                                            state.selected_connection_id = Some(conn.id);
                                        }
                                    });

                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new(&conn.path).size(10.0).color(TEXT_2).family(FontFamily::Monospace));
                                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                            ui.label(RichText::new(format!("{} msgs", conn.message_count)).size(9.0).color(ACCENT_CYAN));
                                        });
                                    });
                                });
                            ui.add_space(2.0);
                        }
                    }
                });
        });

    // ── Main Frame Stream & Inspector Panel ──────────────────────────
    egui::CentralPanel::default().show_inside(ui, |ui| {
        // Toolbar: Search, Opcode Filter, Clear & Export Buttons
        ui.horizontal(|ui| {
            ui.label(RichText::new("🔍 Search:").size(11.0).color(TEXT_1).strong());
            ui.add(
                egui::TextEdit::singleline(&mut state.search_query)
                    .hint_text("Filter payload text...")
                    .desired_width(140.0)
            );

            ui.add_space(6.0);
            ui.label(RichText::new("Opcode:").size(11.0).color(TEXT_1).strong());

            let filters = [
                (None, "All"),
                (Some(WsOpcode::Text), "Text"),
                (Some(WsOpcode::Binary), "Binary"),
                (Some(WsOpcode::Ping), "Ping/Pong"),
                (Some(WsOpcode::Close), "Close"),
            ];

            for (op, label) in filters {
                let active = state.filter_opcode == op;
                if ui.add(
                    egui::SelectableLabel::new(active, RichText::new(label).size(10.0))
                ).clicked() {
                    state.filter_opcode = op;
                }
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Feature 1: Clear WS History Button
                if ui.add(
                    egui::Button::new(RichText::new("🗑 Clear History").size(10.0).color(ACCENT_RED).strong())
                        .fill(Color32::from_rgb(50, 15, 20))
                        .rounding(Rounding::same(4.0))
                ).clicked() {
                    crate::proxy::store::clear_ws_history();
                    state.selected_connection_id = None;
                    state.selected_frame_id = None;
                }

                // Feature 2: Export WS History Button
                if ui.add(
                    egui::Button::new(RichText::new("📥 Export Logs").size(10.0).color(ACCENT_GREEN).strong())
                        .fill(Color32::from_rgb(15, 45, 25))
                        .rounding(Rounding::same(4.0))
                ).clicked() {
                    state.show_export_modal = true;
                    state.export_status_msg.clear();
                }
            });
        });

        ui.add_space(4.0);

        // Compute filtered frames list
        let filtered_frames: Vec<&WsFrameEntry> = frames
            .iter()
            .filter(|f| {
                if let Some(conn_id) = state.selected_connection_id {
                    if f.connection_id != conn_id {
                        return false;
                    }
                }
                if let Some(ref filter_op) = state.filter_opcode {
                    match filter_op {
                        WsOpcode::Ping => {
                            if f.opcode != WsOpcode::Ping && f.opcode != WsOpcode::Pong {
                                return false;
                            }
                        }
                        _ => {
                            if &f.opcode != filter_op {
                                return false;
                            }
                        }
                    }
                }
                if !state.search_query.is_empty() {
                    let q = state.search_query.to_lowercase();
                    if !f.payload.to_lowercase().contains(&q) {
                        return false;
                    }
                }
                true
            })
            .collect();

        // ── Frame Table Split View ───────────────────────────────────
        egui::TopBottomPanel::top("ws_frames_table_panel")
            .resizable(true)
            .default_height(260.0)
            .height_range(140.0..=500.0)
            .show_inside(ui, |ui| {
                section_frame().show(ui, |ui| {
                    ScrollArea::vertical()
                        .id_source("ws_frames_scroll")
                        .show(ui, |ui| {
                            egui::Grid::new("ws_frames_grid")
                                .striped(true)
                                .num_columns(6)
                                .spacing([12.0, 4.0])
                                .show(ui, |ui| {
                                    ui.label(RichText::new("#").size(11.0).color(TEXT_1).strong());
                                    ui.label(RichText::new("Time").size(11.0).color(TEXT_1).strong());
                                    ui.label(RichText::new("Dir").size(11.0).color(TEXT_1).strong());
                                    ui.label(RichText::new("Opcode").size(11.0).color(TEXT_1).strong());
                                    ui.label(RichText::new("Length").size(11.0).color(TEXT_1).strong());
                                    ui.label(RichText::new("Payload Preview").size(11.0).color(TEXT_1).strong());
                                    ui.end_row();

                                    if filtered_frames.is_empty() {
                                        ui.label("");
                                        ui.label("");
                                        ui.label(RichText::new("No WebSocket frames captured matching current filter.").size(11.0).color(TEXT_2));
                                        ui.label("");
                                        ui.label("");
                                        ui.label("");
                                        ui.end_row();
                                    }

                                    for frame in &filtered_frames {
                                        let is_selected = state.selected_frame_id == Some(frame.id);

                                        let (dir_icon, dir_color) = match frame.direction {
                                            WsDirection::ClientToServer => ("⬆️ Client", ACCENT_BLUE),
                                            WsDirection::ServerToClient => ("⬇️ Server", ACCENT_GREEN),
                                        };

                                        let op_color = match frame.opcode {
                                            WsOpcode::Text => TEXT_0,
                                            WsOpcode::Binary => ACCENT_CYAN,
                                            WsOpcode::Close => ACCENT_RED,
                                            WsOpcode::Ping | WsOpcode::Pong => ACCENT_AMBER,
                                            _ => TEXT_2,
                                        };

                                        if ui.add(
                                            egui::SelectableLabel::new(is_selected, RichText::new(format!("{}", frame.id)).size(10.0).family(FontFamily::Monospace))
                                        ).clicked() {
                                            state.selected_frame_id = Some(frame.id);
                                        }

                                        ui.label(RichText::new(&frame.timestamp).size(10.0).color(TEXT_2).family(FontFamily::Monospace));
                                        ui.label(RichText::new(dir_icon).size(10.0).color(dir_color).strong());
                                        ui.label(RichText::new(frame.opcode.label()).size(10.0).color(op_color).strong());
                                        ui.label(RichText::new(format!("{} B", frame.length)).size(10.0).color(TEXT_2));

                                        let payload_short = if frame.payload.len() > 80 {
                                            format!("{}...", &frame.payload[..80])
                                        } else {
                                            frame.payload.clone()
                                        };

                                        ui.horizontal(|ui| {
                                            ui.label(RichText::new(&payload_short).size(10.0).color(TEXT_0).family(FontFamily::Monospace));

                                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                if ui.add(
                                                    egui::Button::new(RichText::new("➡️ Repeater").size(9.0).color(ACCENT_CYAN))
                                                        .fill(BG_RAISED)
                                                ).clicked() {
                                                    // Send to Repeater tab
                                                    let conn_url = connections
                                                        .iter()
                                                        .find(|c| c.id == frame.connection_id)
                                                        .map(|c| c.url.clone())
                                                        .unwrap_or_else(|| format!("WS #{}", frame.connection_id));

                                                    let new_tab = WsRepeaterTab {
                                                        name: format!("Repeater #{}", repeater_tabs.len() + 1),
                                                        target_url: conn_url,
                                                        is_connected: true,
                                                        send_opcode: frame.opcode.clone(),
                                                        payload_input: frame.payload.clone(),
                                                        log_messages: vec![(*frame).clone()],
                                                    };
                                                    repeater_tabs.push(new_tab);
                                                    *active_repeater_tab = repeater_tabs.len() - 1;
                                                    *ws_sub_tab = WsSubTab::Repeater;
                                                }
                                            });
                                        });

                                        ui.end_row();
                                    }
                                });
                        });
                });
            });

        // ── Frame Inspector Panel (Bottom) ───────────────────────────
        egui::CentralPanel::default().show_inside(ui, |ui| {
            section_frame().show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Frame Inspector").size(12.0).color(TEXT_0).strong());

                    ui.add_space(10.0);
                    let modes = ["Raw Text", "Hex Dump", "JSON Pretty"];
                    for (idx, mode_name) in modes.iter().enumerate() {
                        let active = state.inspector_mode == idx;
                        if ui.add(egui::SelectableLabel::new(active, RichText::new(*mode_name).size(10.0))).clicked() {
                            state.inspector_mode = idx;
                        }
                    }
                });

                ui.separator();
                ui.add_space(4.0);

                let selected_frame = frames.iter().find(|f| Some(f.id) == state.selected_frame_id);

                if let Some(frame) = selected_frame {
                    ScrollArea::vertical()
                        .id_source("ws_inspector_scroll")
                        .show(ui, |ui| {
                            let content = match state.inspector_mode {
                                0 => frame.payload.clone(),
                                1 => format_hex_dump(&frame.payload_bytes),
                                2 => {
                                    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&frame.payload) {
                                        serde_json::to_string_pretty(&parsed).unwrap_or_else(|_| frame.payload.clone())
                                    } else {
                                        "[Non-JSON Payload]\n".to_string() + &frame.payload
                                    }
                                }
                                _ => frame.payload.clone(),
                            };

                            ui.add(
                                egui::TextEdit::multiline(&mut content.as_str())
                                    .font(egui::TextStyle::Monospace)
                                    .desired_width(f32::INFINITY)
                                    .desired_rows(8)
                            );
                        });
                } else {
                    ui.label(RichText::new("Select a frame from the stream table above to view detailed contents.").size(11.0).color(TEXT_2));
                }
            });
        });
    });
}

fn format_hex_dump(bytes: &[u8]) -> String {
    let mut out = String::new();
    for (idx, chunk) in bytes.chunks(16).enumerate() {
        out.push_str(&format!("{:04X}  ", idx * 16));
        for b in chunk {
            out.push_str(&format!("{:02X} ", b));
        }
        for _ in chunk.len()..16 {
            out.push_str("   ");
        }
        out.push_str(" |");
        for b in chunk {
            if b.is_ascii_graphic() || *b == b' ' {
                out.push(*b as char);
            } else {
                out.push('.');
            }
        }
        out.push_str("|\n");
    }
    out
}

fn render_ws_export_modal(
    ctx: &egui::Context,
    state: &mut WsHistoryState,
    connections: &[WsConnection],
    frames: &[WsFrameEntry],
) {
    if !state.show_export_modal {
        return;
    }

    let mut is_open = state.show_export_modal;
    egui::Window::new(RichText::new("📥 Export WebSocket Traffic History").size(14.0).color(TEXT_0).strong())
        .open(&mut is_open)
        .collapsible(false)
        .resizable(false)
        .default_size([520.0, 340.0])
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| {
            ui.add_space(4.0);
            ui.label(RichText::new(format!("Export {} WebSocket connections and {} captured frames:", connections.len(), frames.len())).size(11.0).color(TEXT_2));
            ui.separator();
            ui.add_space(8.0);

            // Option 1: JSON Export
            ui.horizontal(|ui| {
                ui.label(RichText::new("1. JSON Format (.json)").size(12.0).color(ACCENT_CYAN).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(RichText::new("📂 Save JSON").size(11.0).color(ACCENT_GREEN)).clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .set_title("Export AJProxy WebSocket History (JSON)")
                            .set_file_name("ajproxy_ws_history.json")
                            .add_filter("JSON File", &["json"])
                            .save_file()
                        {
                            #[derive(serde::Serialize)]
                            struct WsExportData<'a> {
                                connections: &'a [WsConnection],
                                frames: &'a [WsFrameEntry],
                            }
                            let export_obj = WsExportData { connections, frames };
                            let data = serde_json::to_string_pretty(&export_obj).unwrap_or_default();
                            if let Ok(mut f) = File::create(&path) {
                                let _ = f.write_all(data.as_bytes());
                                state.export_status_msg = format!("✔ Saved: {}", path.to_string_lossy());
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
                                state.export_status_msg = format!("✔ Saved: {}", path.to_string_lossy());
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
                                log_buf.push_str(&format!("========================================================================\n"));
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
                                state.export_status_msg = format!("✔ Saved: {}", path.to_string_lossy());
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
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(RichText::new("Close").size(11.0).color(TEXT_0)).clicked() {
                        state.show_export_modal = false;
                    }
                });
            });
        });
    state.show_export_modal = is_open;
}
