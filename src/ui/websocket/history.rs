use egui::{self, RichText, Color32, ScrollArea, Rounding, FontFamily};
use crate::models::*;
use crate::theme::*;

pub fn render(
    ui: &mut egui::Ui,
    state: &mut WsHistoryState,
    connections: &[WsConnection],
    frames: &[WsFrameEntry],
    repeater_tabs: &mut Vec<WsRepeaterTab>,
    active_repeater_tab: &mut usize,
    ws_sub_tab: &mut WsSubTab,
) {
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
        // Toolbar: Search & Opcode Filter
        ui.horizontal(|ui| {
            ui.label(RichText::new("🔍 Search:").size(11.0).color(TEXT_1).strong());
            ui.add(
                egui::TextEdit::singleline(&mut state.search_query)
                    .hint_text("Filter payload text...")
                    .desired_width(180.0)
            );

            ui.add_space(10.0);
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
                                        ui.label(RichText::new("No WebSocket frames captured yet.").size(11.0).color(TEXT_2));
                                        ui.label("");
                                        ui.label("");
                                        ui.label("");
                                        ui.end_row();
                                    }

                                    for frame in &filtered_frames {
                                        let is_sel = state.selected_frame_id == Some(frame.id);

                                        // Entire row click detection
                                        let row_id_str = format!("{}", frame.id);

                                        // ID Column
                                        if ui.add(egui::SelectableLabel::new(is_sel, RichText::new(&row_id_str).size(10.0).family(FontFamily::Monospace))).clicked() {
                                            state.selected_frame_id = Some(frame.id);
                                        }

                                        // Time Column
                                        if ui.add(egui::SelectableLabel::new(is_sel, RichText::new(&frame.timestamp).size(10.0).color(TEXT_2).family(FontFamily::Monospace))).clicked() {
                                            state.selected_frame_id = Some(frame.id);
                                        }

                                        // Direction Column ⬆️ Client / ⬇️ Server
                                        let (dir_str, dir_color) = match frame.direction {
                                            WsDirection::ClientToServer => ("⬆️ Client", ACCENT_GREEN),
                                            WsDirection::ServerToClient => ("⬇️ Server", ACCENT_BLUE),
                                        };
                                        if ui.add(egui::SelectableLabel::new(is_sel, RichText::new(dir_str).size(10.0).color(dir_color).strong())).clicked() {
                                            state.selected_frame_id = Some(frame.id);
                                        }

                                        // Opcode Badge Column
                                        let (op_bg, op_fg) = match frame.opcode {
                                            WsOpcode::Text => (Color32::from_rgb(15, 40, 70), ACCENT_BLUE),
                                            WsOpcode::Binary => (Color32::from_rgb(45, 20, 65), Color32::from_rgb(192, 132, 252)),
                                            WsOpcode::Ping | WsOpcode::Pong => (Color32::from_rgb(60, 45, 10), ACCENT_AMBER),
                                            WsOpcode::Close => (Color32::from_rgb(65, 15, 20), ACCENT_RED),
                                            _ => (BG_RAISED, TEXT_2),
                                        };

                                        let op_btn = egui::Frame::none()
                                            .fill(op_bg)
                                            .rounding(Rounding::same(3.0))
                                            .inner_margin(egui::Margin::symmetric(5.0, 2.0))
                                            .show(ui, |ui| {
                                                ui.label(RichText::new(frame.opcode.label()).size(9.0).color(op_fg).strong().family(FontFamily::Monospace));
                                            }).response;

                                        if op_btn.clicked() {
                                            state.selected_frame_id = Some(frame.id);
                                        }

                                        // Length Column
                                        if ui.add(egui::SelectableLabel::new(is_sel, RichText::new(format!("{} B", frame.length)).size(10.0).color(TEXT_2).family(FontFamily::Monospace))).clicked() {
                                            state.selected_frame_id = Some(frame.id);
                                        }

                                        // Payload Preview Column
                                        let preview = if frame.payload.len() > 70 {
                                            format!("{}...", &frame.payload[..70])
                                        } else {
                                            frame.payload.clone()
                                        };
                                        if ui.add(egui::SelectableLabel::new(is_sel, RichText::new(preview).size(10.0).color(TEXT_0).family(FontFamily::Monospace))).clicked() {
                                            state.selected_frame_id = Some(frame.id);
                                        }

                                        ui.end_row();
                                    }
                                });
                        });
                });
            });

        ui.add_space(4.0);

        // ── Bottom Panel: Frame Payload Inspector ──────────────────────
        section_frame().show(ui, |ui| {
            let selected_frame = frames.iter().find(|f| Some(f.id) == state.selected_frame_id);

            ui.horizontal(|ui| {
                ui.label(RichText::new("Frame Payload Inspector").size(12.0).color(TEXT_0).strong());
                ui.add_space(12.0);

                let modes = ["Raw Text", "Hex Dump", "Formatted JSON"];
                for (idx, mode_name) in modes.iter().enumerate() {
                    if ui.selectable_label(state.inspector_mode == idx, RichText::new(*mode_name).size(10.0)).clicked() {
                        state.inspector_mode = idx;
                    }
                }

                if let Some(frame) = selected_frame {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add(
                            egui::Button::new(RichText::new("🚀 Send to WS Repeater").size(11.0).color(TEXT_0).strong())
                                .fill(ACCENT_BLUE)
                                .rounding(Rounding::same(4.0))
                        ).clicked() {
                            let target_url = connections
                                .iter()
                                .find(|c| c.id == frame.connection_id)
                                .map(|c| c.url.clone())
                                .unwrap_or_else(|| "wss://echo.websocket.events".into());

                            repeater_tabs.push(WsRepeaterTab {
                                name: format!("WS Tab {}", repeater_tabs.len() + 1),
                                target_url,
                                is_connected: false,
                                send_opcode: frame.opcode.clone(),
                                payload_input: frame.payload.clone(),
                                log_messages: vec![],
                            });
                            *active_repeater_tab = repeater_tabs.len() - 1;
                            *ws_sub_tab = WsSubTab::Repeater;
                        }
                    });
                }
            });

            ui.separator();
            ui.add_space(4.0);

            if let Some(frame) = selected_frame {
                ScrollArea::vertical()
                    .id_source("ws_inspector_scroll")
                    .show(ui, |ui| {
                        match state.inspector_mode {
                            0 => {
                                // Raw Text
                                ui.add(
                                    egui::TextEdit::multiline(&mut frame.payload.as_str())
                                        .font(egui::TextStyle::Monospace)
                                        .desired_width(f32::INFINITY)
                                        .desired_rows(6)
                                );
                            }
                            1 => {
                                // Hex Dump
                                let hex_dump = format_hex_dump(&frame.payload_bytes);
                                ui.add(
                                    egui::TextEdit::multiline(&mut hex_dump.as_str())
                                        .font(egui::TextStyle::Monospace)
                                        .desired_width(f32::INFINITY)
                                        .desired_rows(6)
                                );
                            }
                            _ => {
                                // Formatted JSON
                                let json_text = match serde_json::from_str::<serde_json::Value>(&frame.payload) {
                                    Ok(val) => serde_json::to_string_pretty(&val).unwrap_or(frame.payload.clone()),
                                    Err(_) => frame.payload.clone(),
                                };
                                ui.add(
                                    egui::TextEdit::multiline(&mut json_text.as_str())
                                        .font(egui::TextStyle::Monospace)
                                        .desired_width(f32::INFINITY)
                                        .desired_rows(6)
                                );
                            }
                        }
                    });
            } else {
                ui.add_space(20.0);
                ui.label(RichText::new("Select a WebSocket frame from the table above to inspect its raw payload.").size(11.0).color(TEXT_2));
                ui.add_space(20.0);
            }
        });
    });
}

fn format_hex_dump(bytes: &[u8]) -> String {
    let mut out = String::new();
    for (idx, chunk) in bytes.chunks(16).enumerate() {
        out.push_str(&format!("{:04x}: ", idx * 16));
        for b in chunk {
            out.push_str(&format!("{:02x} ", b));
        }
        if chunk.len() < 16 {
            for _ in 0..(16 - chunk.len()) {
                out.push_str("   ");
            }
        }
        out.push_str(" | ");
        for b in chunk {
            let ch = if b.is_ascii_graphic() || *b == b' ' { *b as char } else { '.' };
            out.push(ch);
        }
        out.push('\n');
    }
    out
}
