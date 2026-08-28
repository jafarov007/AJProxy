use egui::{self, RichText, Color32, ScrollArea, Stroke, Rounding, FontFamily};
use crate::models::*;
use crate::theme::*;

pub fn render(
    ui: &mut egui::Ui,
    repeater_tabs: &mut Vec<WsRepeaterTab>,
    active_tab_idx: &mut usize,
) {
    if repeater_tabs.is_empty() {
        repeater_tabs.push(WsRepeaterTab::default());
    }

    if *active_tab_idx >= repeater_tabs.len() {
        *active_tab_idx = 0;
    }

    // ── WS Repeater Tab Bar ──────────────────────────────────────────
    ui.horizontal(|ui| {
        let mut to_remove = None;

        for (idx, tab) in repeater_tabs.iter().enumerate() {
            let active = idx == *active_tab_idx;
            let text_color = if active { TEXT_0 } else { TEXT_2 };
            let bg_color = if active { BG_RAISED } else { Color32::TRANSPARENT };

            let btn = ui.add(
                egui::Button::new(RichText::new(&tab.name).size(11.0).color(text_color).strong())
                    .fill(bg_color)
                    .stroke(if active { Stroke::new(1.0_f32, ACCENT_BLUE) } else { Stroke::NONE })
                    .rounding(Rounding::same(4.0))
            );

            if btn.clicked() {
                *active_tab_idx = idx;
            }

            if repeater_tabs.len() > 1 && active {
                if ui.add(egui::Button::new(RichText::new("✖").size(10.0).color(TEXT_2))).clicked() {
                    to_remove = Some(idx);
                }
            }
        }

        if let Some(idx) = to_remove {
            repeater_tabs.remove(idx);
            if *active_tab_idx >= repeater_tabs.len() {
                *active_tab_idx = repeater_tabs.len().saturating_sub(1);
            }
        }

        if ui.add(
            egui::Button::new(RichText::new("➕ New WS Tab").size(11.0).color(ACCENT_BLUE).strong())
                .fill(BG_RAISED)
                .rounding(Rounding::same(4.0))
        ).clicked() {
            repeater_tabs.push(WsRepeaterTab {
                name: format!("WS Tab {}", repeater_tabs.len() + 1),
                target_url: "wss://echo.websocket.events".into(),
                is_connected: false,
                send_opcode: WsOpcode::Text,
                payload_input: "{\"event\":\"ping\"}".into(),
                log_messages: vec![],
            });
            *active_tab_idx = repeater_tabs.len() - 1;
        }
    });

    ui.add_space(6.0);
    ui.separator();
    ui.add_space(6.0);

    let tab = &mut repeater_tabs[*active_tab_idx];

    // ── Target Connection Header Bar ─────────────────────────────────
    section_frame().show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Target WS URL:").size(11.0).color(TEXT_1).strong());
            ui.add(
                egui::TextEdit::singleline(&mut tab.target_url)
                    .hint_text("wss://example.com/socket")
                    .desired_width(380.0)
            );

            let (conn_btn_text, conn_btn_bg, conn_btn_fg) = if tab.is_connected {
                ("❌ Disconnect", Color32::from_rgb(60, 15, 20), ACCENT_RED)
            } else {
                ("🔌 Connect", ACCENT_BLUE, TEXT_0)
            };

            if ui.add(
                egui::Button::new(RichText::new(conn_btn_text).size(11.0).color(conn_btn_fg).strong())
                    .fill(conn_btn_bg)
                    .rounding(Rounding::same(4.0))
            ).clicked() {
                tab.is_connected = !tab.is_connected;
                let status_msg = if tab.is_connected { "Connected to WebSocket endpoint." } else { "Disconnected." };
                tab.log_messages.push(WsFrameEntry {
                    id: tab.log_messages.len() as u64 + 1,
                    connection_id: 0,
                    timestamp: chrono::Local::now().format("%H:%M:%S").to_string(),
                    direction: WsDirection::ServerToClient,
                    opcode: WsOpcode::Text,
                    length: status_msg.len(),
                    payload: status_msg.into(),
                    payload_bytes: status_msg.as_bytes().to_vec(),
                    is_final: true,
                });
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let (status_text, status_color) = if tab.is_connected {
                    ("ONLINE", ACCENT_GREEN)
                } else {
                    ("OFFLINE", TEXT_2)
                };
                ui.label(RichText::new(status_text).size(11.0).color(status_color).strong());
            });
        });
    });

    ui.add_space(6.0);

    // ── Workbench Split: Left Request Builder | Right Live Response Log ──
    ui.columns(2, |cols| {
        // Left Column: Send Frame Editor
        cols[0].group(|ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Send WebSocket Frame").size(12.0).color(TEXT_0).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add(
                        egui::Button::new(RichText::new("🚀 Send Frame").size(11.0).color(TEXT_0).strong())
                            .fill(ACCENT_BLUE)
                            .rounding(Rounding::same(4.0))
                    ).clicked() {
                        let msg = WsFrameEntry {
                            id: tab.log_messages.len() as u64 + 1,
                            connection_id: 1,
                            timestamp: chrono::Local::now().format("%H:%M:%S%.3f").to_string(),
                            direction: WsDirection::ClientToServer,
                            opcode: tab.send_opcode.clone(),
                            length: tab.payload_input.len(),
                            payload: tab.payload_input.clone(),
                            payload_bytes: tab.payload_input.as_bytes().to_vec(),
                            is_final: true,
                        };
                        tab.log_messages.push(msg);
                    }
                });
            });

            ui.separator();
            ui.add_space(4.0);

            ui.horizontal(|ui| {
                ui.label(RichText::new("Opcode:").size(11.0).color(TEXT_1).strong());
                let opcodes = [WsOpcode::Text, WsOpcode::Binary, WsOpcode::Ping, WsOpcode::Pong];
                for op in opcodes {
                    if ui.selectable_label(tab.send_opcode == op, RichText::new(op.label()).size(10.0)).clicked() {
                        tab.send_opcode = op;
                    }
                }
            });

            ui.add_space(6.0);
            ui.label(RichText::new("Payload Content:").size(11.0).color(TEXT_1).strong());
            ui.add_space(4.0);

            ui.add(
                egui::TextEdit::multiline(&mut tab.payload_input)
                    .font(egui::TextStyle::Monospace)
                    .desired_width(f32::INFINITY)
                    .desired_rows(12)
            );
        });

        // Right Column: Live Event Console Log
        cols[1].group(|ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Live Response Log").size(12.0).color(TEXT_0).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add(egui::Button::new(RichText::new("Clear Log").size(10.0).color(TEXT_2))).clicked() {
                        tab.log_messages.clear();
                    }
                });
            });

            ui.separator();
            ui.add_space(4.0);

            ScrollArea::vertical()
                .id_source("ws_repeater_log_scroll")
                .show(ui, |ui| {
                    if tab.log_messages.is_empty() {
                        ui.add_space(20.0);
                        ui.label(RichText::new("No messages sent or received yet.").size(11.0).color(TEXT_2));
                    } else {
                        for frame in &tab.log_messages {
                            let (dir_str, dir_color) = match frame.direction {
                                WsDirection::ClientToServer => ("⬆️ SENT", ACCENT_GREEN),
                                WsDirection::ServerToClient => ("⬇️ RECV", ACCENT_BLUE),
                            };

                            ui.horizontal(|ui| {
                                ui.label(RichText::new(&frame.timestamp).size(9.0).color(TEXT_2).family(FontFamily::Monospace));
                                ui.label(RichText::new(dir_str).size(10.0).color(dir_color).strong());
                                ui.label(RichText::new(frame.opcode.label()).size(9.0).color(ACCENT_CYAN).family(FontFamily::Monospace));
                            });
                            ui.label(RichText::new(&frame.payload).size(10.0).color(TEXT_0).family(FontFamily::Monospace));
                            ui.separator();
                        }
                    }
                });
        });
    });
}
