pub mod export_modal;

use egui::{self, RichText, Color32, ScrollArea, Rounding, FontFamily};
use crate::models::*;
use crate::theme::*;
use export_modal::render_export_modal;

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
    render_export_modal(ui.ctx(), state, connections, frames);

    egui::SidePanel::left("ws_history_connections_panel")
        .default_width(220.0)
        .width_range(170.0..=280.0)
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
                                    // Row 1: Status dot and Host title
                                    ui.horizontal(|ui| {
                                        let status_color = if conn.status.starts_with("Active") { ACCENT_GREEN } else { TEXT_2 };
                                        ui.label(RichText::new("●").size(10.0).color(status_color));

                                        let conn_title = format!("WS #{} - {}", conn.id, conn.host);
                                        let btn = ui.add(
                                            egui::SelectableLabel::new(
                                                is_selected,
                                                RichText::new(&conn_title)
                                                    .size(11.0)
                                                    .color(TEXT_0)
                                                    .strong()
                                            )
                                        );
                                        if btn.clicked() {
                                            state.selected_connection_id = Some(conn.id);
                                        }
                                    });

                                    // Row 2: Message count and Path
                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new(format!("{} msgs", conn.message_count)).size(9.0).color(ACCENT_CYAN));
                                        ui.add_space(4.0);
                                        ui.add(egui::Label::new(RichText::new(&conn.path).size(10.0).color(TEXT_2).family(FontFamily::Monospace)).truncate(true));
                                    });

                                    // Row 3: Close Tunnel Button (Dedicated row if active)
                                    if conn.status.starts_with("Active") {
                                        ui.add_space(2.0);
                                        if ui.add(
                                            egui::Button::new(RichText::new("🔌 Close Active Tunnel").size(9.5).color(ACCENT_RED).strong())
                                                .fill(Color32::from_rgb(50, 15, 20))
                                                .stroke(egui::Stroke::new(0.5_f32, ACCENT_RED))
                                                .min_size(egui::vec2(ui.available_width(), 18.0))
                                                .rounding(Rounding::same(3.0))
                                        ).on_hover_text("Disconnect active WebSocket tunnel").clicked() {
                                            crate::proxy::store::close_ws_connection(conn.id);
                                        }
                                    }
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
                    // Deep search: payload + connection URL/host
                    let matches_payload = f.payload.to_lowercase().contains(&q);
                    let matches_conn = connections
                        .iter()
                        .find(|c| c.id == f.connection_id)
                        .map(|c| c.url.to_lowercase().contains(&q) || c.host.to_lowercase().contains(&q))
                        .unwrap_or(false);
                    if !matches_payload && !matches_conn {
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
                                            ui.add(egui::Label::new(RichText::new(&payload_short).size(10.0).color(TEXT_0).family(FontFamily::Monospace)).truncate(true));

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
                    ScrollArea::both()
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
