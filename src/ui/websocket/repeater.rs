use std::collections::HashMap;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use egui::{self, RichText, Color32, ScrollArea, Stroke, Rounding, FontFamily};
use crate::models::*;
use crate::theme::*;
use crate::proxy::websocket::protocol::WsRawFrame;
use crate::proxy::websocket::repeater_client::spawn_repeater_client;

lazy_static::lazy_static! {
    static ref REPEATER_CLIENT_HANDLES: Arc<Mutex<HashMap<usize, Sender<WsRawFrame>>>> = Arc::new(Mutex::new(HashMap::new()));
    static ref REPEATER_INCOMING_QUEUE: Arc<Mutex<Vec<(usize, WsFrameEntry)>>> = Arc::new(Mutex::new(Vec::new()));
}

fn drain_incoming_messages(repeater_tabs: &mut [WsRepeaterTab]) {
    if let Ok(mut lock) = REPEATER_INCOMING_QUEUE.lock() {
        if !lock.is_empty() {
            for (tab_idx, frame) in lock.drain(..) {
                if tab_idx < repeater_tabs.len() {
                    repeater_tabs[tab_idx].log_messages.push(frame);
                }
            }
        }
    }
}

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

    // Drain any incoming WebSocket responses from background client threads into tab logs
    drain_incoming_messages(repeater_tabs);

    let active_conns = crate::proxy::store::get_ws_connections();

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
            if let Ok(mut lock) = REPEATER_CLIENT_HANDLES.lock() {
                lock.remove(&idx);
            }
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

    let tab_idx = *active_tab_idx;
    let tab = &mut repeater_tabs[tab_idx];

    // ── Target Connection Header Bar ─────────────────────────────────
    section_frame().show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Target Socket:").size(11.0).color(TEXT_1).strong());

            // Active Captured Proxy Connection Selector Dropdown
            let current_label = if tab.target_url.starts_with("WS #") {
                tab.target_url.clone()
            } else if active_conns.iter().any(|c| c.url == tab.target_url) {
                if let Some(c) = active_conns.iter().find(|c| c.url == tab.target_url) {
                    format!("⚡ WS #{} ({})", c.id, c.host)
                } else {
                    tab.target_url.clone()
                }
            } else {
                format!("🔌 Standalone ({})", if tab.target_url.is_empty() { "wss://..." } else { &tab.target_url })
            };

            egui::ComboBox::from_id_source(format!("ws_target_combo_{}", tab_idx))
                .selected_text(RichText::new(current_label).size(11.0).color(TEXT_0))
                .show_ui(ui, |ui| {
                    if ui.selectable_label(!tab.target_url.starts_with("WS #"), "🔌 Standalone Socket (Connect New Endpoint)").clicked() {
                        if tab.target_url.starts_with("WS #") {
                            tab.target_url = "wss://echo.websocket.events".into();
                            tab.is_connected = false;
                        }
                    }
                    if !active_conns.is_empty() {
                        ui.separator();
                        ui.label(RichText::new("Captured Proxy Tunnels:").size(10.0).color(TEXT_2));
                        for conn in &active_conns {
                            let label = format!("⚡ WS #{} - {} ({})", conn.id, conn.host, conn.status);
                            let conn_url_tag = format!("WS #{}", conn.id);
                            if ui.selectable_label(tab.target_url == conn_url_tag || tab.target_url == conn.url, &label).clicked() {
                                tab.target_url = conn.url.clone();
                                tab.is_connected = conn.status.starts_with("Active");
                            }
                        }
                    }
                });

            // Target URL text editor for standalone socket connection
            let matched_conn = active_conns.iter().find(|c| c.url == tab.target_url);

            if matched_conn.is_none() {
                ui.add_space(4.0);
                ui.add(
                    egui::TextEdit::singleline(&mut tab.target_url)
                        .hint_text("wss://echo.websocket.org")
                        .desired_width(240.0)
                );
            }

            // Feature 3: Connect / Reconnect Button
            let is_handle_connected = {
                if let Ok(lock) = REPEATER_CLIENT_HANDLES.lock() {
                    lock.contains_key(&tab_idx)
                } else {
                    false
                }
            };

            let is_online = tab.is_connected || is_handle_connected;

            let (conn_btn_text, conn_btn_bg, conn_btn_fg) = if is_online {
                ("❌ Disconnect", Color32::from_rgb(60, 15, 20), ACCENT_RED)
            } else {
                ("🔌 Reconnect / Connect Socket", ACCENT_BLUE, TEXT_0)
            };

            if ui.add(
                egui::Button::new(RichText::new(conn_btn_text).size(11.0).color(conn_btn_fg).strong())
                    .fill(conn_btn_bg)
                    .rounding(Rounding::same(4.0))
            ).clicked() {
                if is_online {
                    // Disconnect
                    if let Ok(mut lock) = REPEATER_CLIENT_HANDLES.lock() {
                        lock.remove(&tab_idx);
                    }
                    tab.is_connected = false;
                    let msg = "Disconnected socket connection.";
                    tab.log_messages.push(WsFrameEntry {
                        id: crate::proxy::store::next_ws_frame_id(),
                        connection_id: 0,
                        timestamp: chrono::Local::now().format("%H:%M:%S").to_string(),
                        direction: WsDirection::ServerToClient,
                        opcode: WsOpcode::Close,
                        length: msg.len(),
                        payload: msg.into(),
                        payload_bytes: msg.as_bytes().to_vec(),
                        is_final: true,
                    });
                } else {
                    // Connect / Reconnect
                    let url_to_connect = tab.target_url.clone();
                    let current_tab_idx = tab_idx;
                    match spawn_repeater_client(url_to_connect.clone(), move |frame| {
                        if let Ok(mut q) = REPEATER_INCOMING_QUEUE.lock() {
                            q.push((current_tab_idx, frame));
                        }
                    }) {
                        Ok(handle) => {
                            if let Ok(mut lock) = REPEATER_CLIENT_HANDLES.lock() {
                                lock.insert(current_tab_idx, handle.tx);
                            }
                            tab.is_connected = true;
                            let msg = format!("⚡ Connected online client socket to: {}", url_to_connect);
                            tab.log_messages.push(WsFrameEntry {
                                id: crate::proxy::store::next_ws_frame_id(),
                                connection_id: 0,
                                timestamp: chrono::Local::now().format("%H:%M:%S").to_string(),
                                direction: WsDirection::ServerToClient,
                                opcode: WsOpcode::Text,
                                length: msg.len(),
                                payload: msg,
                                payload_bytes: vec![],
                                is_final: true,
                            });
                        }
                        Err(err_msg) => {
                            tab.is_connected = false;
                            let msg = format!("❌ Socket Connection Error: {}", err_msg);
                            tab.log_messages.push(WsFrameEntry {
                                id: crate::proxy::store::next_ws_frame_id(),
                                connection_id: 0,
                                timestamp: chrono::Local::now().format("%H:%M:%S").to_string(),
                                direction: WsDirection::ServerToClient,
                                opcode: WsOpcode::Close,
                                length: msg.len(),
                                payload: msg,
                                payload_bytes: vec![],
                                is_final: true,
                            });
                        }
                    }
                }
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let (status_text, status_color) = if is_online {
                    ("● ONLINE", ACCENT_GREEN)
                } else {
                    ("● OFFLINE (CLOSED)", TEXT_2)
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
                        let is_handle_connected = {
                            if let Ok(lock) = REPEATER_CLIENT_HANDLES.lock() {
                                lock.contains_key(&tab_idx)
                            } else {
                                false
                            }
                        };

                        // Auto-reconnect if offline!
                        if !is_handle_connected && !tab.target_url.is_empty() {
                            let url_to_connect = tab.target_url.clone();
                            let current_tab_idx = tab_idx;
                            if let Ok(handle) = spawn_repeater_client(url_to_connect, move |frame| {
                                if let Ok(mut q) = REPEATER_INCOMING_QUEUE.lock() {
                                    q.push((current_tab_idx, frame));
                                }
                            }) {
                                if let Ok(mut lock) = REPEATER_CLIENT_HANDLES.lock() {
                                    lock.insert(current_tab_idx, handle.tx);
                                }
                                tab.is_connected = true;
                            }
                        }

                        let payload_bytes = match tab.send_opcode {
                            WsOpcode::Binary => {
                                // Try parsing hex string or raw bytes
                                let clean_hex: String = tab.payload_input.chars().filter(|c| c.is_ascii_hexdigit()).collect();
                                if clean_hex.len() % 2 == 0 && !clean_hex.is_empty() {
                                    (0..clean_hex.len())
                                        .step_by(2)
                                        .map(|i| u8::from_str_radix(&clean_hex[i..i + 2], 16).unwrap_or(0))
                                        .collect()
                                } else {
                                    tab.payload_input.as_bytes().to_vec()
                                }
                            }
                            _ => tab.payload_input.as_bytes().to_vec(),
                        };

                        let raw_frame = WsRawFrame {
                            fin: true,
                            opcode_u8: match tab.send_opcode {
                                WsOpcode::Text => 0x1,
                                WsOpcode::Binary => 0x2,
                                WsOpcode::Close => 0x8,
                                WsOpcode::Ping => 0x9,
                                WsOpcode::Pong => 0xA,
                                _ => 0x1,
                            },
                            masked: true,
                            mask_key: Some([0x12, 0x34, 0x56, 0x78]),
                            payload: payload_bytes.clone(),
                        };

                        // Send to active repeater socket connection if available
                        if let Ok(lock) = REPEATER_CLIENT_HANDLES.lock() {
                            if let Some(tx) = lock.get(&tab_idx) {
                                let _ = tx.send(raw_frame);
                            }
                        }

                        let conn_id = active_conns
                            .iter()
                            .find(|c| c.url == tab.target_url)
                            .map(|c| c.id)
                            .unwrap_or(0);

                        let msg = WsFrameEntry {
                            id: crate::proxy::store::next_ws_frame_id(),
                            connection_id: conn_id,
                            timestamp: chrono::Local::now().format("%H:%M:%S%.3f").to_string(),
                            direction: WsDirection::ClientToServer,
                            opcode: tab.send_opcode.clone(),
                            length: payload_bytes.len(),
                            payload: tab.payload_input.clone(),
                            payload_bytes,
                            is_final: true,
                        };

                        // Record into global WS History & active stream
                        crate::proxy::store::push_ws_frame(msg.clone());
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
