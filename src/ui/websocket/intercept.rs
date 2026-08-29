use egui::{self, RichText, Color32, Stroke, Rounding, FontFamily, ScrollArea};
use crate::models::*;
use crate::theme::*;
use crate::proxy::store::{PENDING_WS_FRAMES, set_ws_intercept_enabled, PendingWsFrame};
use crate::proxy::websocket::protocol::WsRawFrame;

pub fn render(ui: &mut egui::Ui, state: &mut WsInterceptState) {
    let pending_count = if let Ok(lock) = PENDING_WS_FRAMES.lock() {
        lock.len()
    } else {
        0
    };

    let mut action_forward_all = false;
    let mut action_drop_all = false;

    // ── Top Toolbar ──────────────────────────────────────────────────
    ui.horizontal(|ui| {
        ui.label(RichText::new("WebSocket Interceptor").size(14.0).color(TEXT_0).strong());
        ui.add_space(10.0);

        // Toggle Intercept Status Button
        let (btn_text, btn_bg, btn_fg) = if state.enabled {
            ("🛡️ INTERCEPT IS ON", Color32::from_rgb(15, 60, 30), ACCENT_GREEN)
        } else {
            ("⏸️ INTERCEPT IS OFF", BG_RAISED, TEXT_2)
        };

        if ui.add(
            egui::Button::new(RichText::new(btn_text).size(12.0).color(btn_fg).strong())
                .fill(btn_bg)
                .stroke(Stroke::new(1.0_f32, if state.enabled { ACCENT_GREEN } else { BORDER }))
                .rounding(Rounding::same(6.0))
        ).clicked() {
            state.enabled = !state.enabled;
            set_ws_intercept_enabled(state.enabled);

            // If turned OFF, flush any pending frames in queue so proxy threads unblock
            if !state.enabled {
                action_forward_all = true;
            }
        }

        if state.enabled {
            ui.add_space(10.0);
            let badge_color = if pending_count > 0 { ACCENT_AMBER } else { TEXT_2 };
            ui.label(RichText::new(format!("📋 {} Frames Paused in Queue", pending_count)).size(11.0).color(badge_color).strong());

            if pending_count > 0 {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add(
                        egui::Button::new(RichText::new("❌ Drop All").size(10.0).color(ACCENT_RED).strong())
                            .fill(Color32::from_rgb(50, 15, 20))
                            .rounding(Rounding::same(4.0))
                    ).clicked() {
                        action_drop_all = true;
                    }

                    ui.add_space(6.0);

                    if ui.add(
                        egui::Button::new(RichText::new("⏩ Forward All").size(10.0).color(ACCENT_GREEN).strong())
                            .fill(Color32::from_rgb(15, 45, 25))
                            .rounding(Rounding::same(4.0))
                    ).clicked() {
                        action_forward_all = true;
                    }
                });
            }
        }
    });

    ui.add_space(6.0);
    ui.separator();
    ui.add_space(6.0);

    // ── Batch Action Handlers (Forward All / Drop All) ─────────────
    if action_forward_all {
        if let Ok(mut lock) = PENDING_WS_FRAMES.lock() {
            for pending in lock.drain(..) {
                let default_frame = WsRawFrame {
                    fin: true,
                    opcode_u8: match pending.opcode {
                        WsOpcode::Text => 0x1,
                        WsOpcode::Binary => 0x2,
                        WsOpcode::Close => 0x8,
                        WsOpcode::Ping => 0x9,
                        WsOpcode::Pong => 0xA,
                        _ => 0x1,
                    },
                    masked: false,
                    mask_key: None,
                    payload: pending.payload_bytes,
                };
                if let Ok(mut r_lock) = pending.responder.lock() {
                    if let Some(sender) = r_lock.take() {
                        let _ = sender.send(Some(default_frame));
                    }
                }
            }
        }
        state.selected_frame_id = None;
    }

    if action_drop_all {
        if let Ok(mut lock) = PENDING_WS_FRAMES.lock() {
            for pending in lock.drain(..) {
                if let Ok(mut r_lock) = pending.responder.lock() {
                    if let Some(sender) = r_lock.take() {
                        let _ = sender.send(None);
                    }
                }
            }
        }
        state.selected_frame_id = None;
    }

    if !state.enabled {
        section_frame().show(ui, |ui| {
            ui.add_space(40.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new("WebSocket Intercept is currently OFF.").size(13.0).color(TEXT_1).strong());
                ui.add_space(4.0);
                ui.label(RichText::new("Click 'INTERCEPT IS OFF' above to pause and modify live incoming and outgoing WebSocket frames.").size(11.0).color(TEXT_2));
            });
            ui.add_space(40.0);
        });
        return;
    }

    // ── Sync Selected Frame ID with current queue ────────────────────
    let pending_snapshot: Vec<PendingWsFrame> = if let Ok(lock) = PENDING_WS_FRAMES.lock() {
        lock.clone()
    } else {
        Vec::new()
    };

    if pending_snapshot.is_empty() {
        section_frame().show(ui, |ui| {
            ui.add_space(40.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new("Waiting for WebSocket frames...").size(13.0).color(ACCENT_CYAN).strong());
                ui.add_space(4.0);
                ui.label(RichText::new("No WebSocket frames are currently paused in the queue.").size(11.0).color(TEXT_2));
            });
            ui.add_space(40.0);
        });
        return;
    }

    // Ensure state.selected_frame_id points to a valid pending frame
    let selected_frame_exists = pending_snapshot.iter().any(|p| Some(p.id) == state.selected_frame_id);
    if !selected_frame_exists {
        let first = &pending_snapshot[0];
        state.selected_frame_id = Some(first.id);
        state.edited_payload = first.payload.clone();
        state.edited_opcode = first.opcode.clone();
    }

    let mut action_forward_selected = false;
    let mut action_drop_selected = false;

    // ── Workbench Split: Left Pending Queue List | Right Editor ──────
    ui.columns(2, |cols| {
        // Left Column: Paused Intercept Queue Stream
        cols[0].group(|ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Paused Intercept Queue").size(12.0).color(TEXT_0).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new(format!("{} items", pending_snapshot.len())).size(10.0).color(ACCENT_CYAN));
                });
            });

            ui.separator();
            ui.add_space(4.0);

            ScrollArea::vertical()
                .id_source("ws_intercept_queue_scroll")
                .show(ui, |ui| {
                    egui::Grid::new("ws_intercept_queue_grid")
                        .striped(true)
                        .num_columns(5)
                        .spacing([8.0, 4.0])
                        .show(ui, |ui| {
                            ui.label(RichText::new("#").size(10.0).color(TEXT_1).strong());
                            ui.label(RichText::new("Conn").size(10.0).color(TEXT_1).strong());
                            ui.label(RichText::new("Dir").size(10.0).color(TEXT_1).strong());
                            ui.label(RichText::new("Opcode").size(10.0).color(TEXT_1).strong());
                            ui.label(RichText::new("Payload Preview").size(10.0).color(TEXT_1).strong());
                            ui.end_row();

                            for pending in &pending_snapshot {
                                let is_selected = state.selected_frame_id == Some(pending.id);

                                let (dir_icon, dir_color) = match pending.direction {
                                    WsDirection::ClientToServer => ("⬆️ Client", ACCENT_GREEN),
                                    WsDirection::ServerToClient => ("⬇️ Server", ACCENT_BLUE),
                                };

                                if ui.add(
                                    egui::SelectableLabel::new(
                                        is_selected,
                                        RichText::new(format!("{}", pending.id)).size(10.0).family(FontFamily::Monospace)
                                    )
                                ).clicked() {
                                    state.selected_frame_id = Some(pending.id);
                                    state.edited_payload = pending.payload.clone();
                                    state.edited_opcode = pending.opcode.clone();
                                }

                                ui.label(RichText::new(format!("WS #{}", pending.connection_id)).size(10.0).color(TEXT_2).family(FontFamily::Monospace));
                                ui.label(RichText::new(dir_icon).size(10.0).color(dir_color).strong());
                                ui.label(RichText::new(pending.opcode.label()).size(10.0).color(ACCENT_CYAN).strong());

                                let payload_short = if pending.payload.len() > 30 {
                                    format!("{}...", &pending.payload[..30])
                                } else {
                                    pending.payload.clone()
                                };
                                ui.label(RichText::new(&payload_short).size(10.0).color(TEXT_0).family(FontFamily::Monospace));

                                ui.end_row();
                            }
                        });
                });
        });

        // Right Column: Selected Frame Content Editor & Action Controls
        cols[1].group(|ui| {
            let current_selected = pending_snapshot.iter().find(|p| Some(p.id) == state.selected_frame_id);

            if let Some(pending) = current_selected {
                let (dir_text, dir_color) = match pending.direction {
                    WsDirection::ClientToServer => ("⬆️ Outbound Client Frame", ACCENT_GREEN),
                    WsDirection::ServerToClient => ("⬇️ Inbound Server Frame", ACCENT_BLUE),
                };

                ui.horizontal(|ui| {
                    ui.label(RichText::new(dir_text).size(12.0).color(dir_color).strong());

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add(
                            egui::Button::new(RichText::new("▶️ Forward Selected").size(11.0).color(TEXT_0).strong())
                                .fill(ACCENT_BLUE)
                                .rounding(Rounding::same(4.0))
                        ).clicked() {
                            action_forward_selected = true;
                        }

                        ui.add_space(4.0);

                        if ui.add(
                            egui::Button::new(RichText::new("✖ Drop").size(11.0).color(ACCENT_RED).strong())
                                .fill(BG_RAISED)
                                .stroke(Stroke::new(1.0_f32, ACCENT_RED))
                                .rounding(Rounding::same(4.0))
                        ).clicked() {
                            action_drop_selected = true;
                        }
                    });
                });

                ui.separator();
                ui.add_space(4.0);

                egui::Grid::new("ws_intercept_single_edit_grid")
                    .num_columns(2)
                    .spacing([12.0, 6.0])
                    .show(ui, |ui| {
                        ui.label(RichText::new("Connection ID:").size(11.0).color(TEXT_1).strong());
                        ui.label(RichText::new(format!("WS #{}", pending.connection_id)).size(11.0).color(TEXT_0).family(FontFamily::Monospace));
                        ui.end_row();

                        ui.label(RichText::new("Opcode:").size(11.0).color(TEXT_1).strong());
                        ui.horizontal(|ui| {
                            let opcodes = [WsOpcode::Text, WsOpcode::Binary, WsOpcode::Ping, WsOpcode::Pong, WsOpcode::Close];
                            for op in opcodes {
                                if ui.selectable_label(state.edited_opcode == op, RichText::new(op.label()).size(10.0)).clicked() {
                                    state.edited_opcode = op;
                                }
                            }
                        });
                        ui.end_row();
                    });

                ui.add_space(6.0);
                ui.label(RichText::new("Payload Editor:").size(11.0).color(TEXT_1).strong());
                ui.add_space(4.0);

                ui.add(
                    egui::TextEdit::multiline(&mut state.edited_payload)
                        .font(egui::TextStyle::Monospace)
                        .desired_width(f32::INFINITY)
                        .desired_rows(12)
                );
            } else {
                ui.add_space(40.0);
                ui.label(RichText::new("Select a frame from the queue list on the left to inspect and edit.").size(11.0).color(TEXT_2));
            }
        });
    });

    // ── Single Frame Action Handlers (Forward Selected / Drop Selected) ──
    if (action_forward_selected || action_drop_selected) && state.selected_frame_id.is_some() {
        let target_id = state.selected_frame_id.unwrap();
        let mut target_pending = None;

        if let Ok(mut lock) = PENDING_WS_FRAMES.lock() {
            if let Some(pos) = lock.iter().position(|p| p.id == target_id) {
                target_pending = Some(lock.remove(pos));
            }
        }

        if let Some(pending) = target_pending {
            let responder_mutex = pending.responder.clone();
            let mut sender_opt = None;
            if let Ok(mut lock) = responder_mutex.lock() {
                sender_opt = lock.take();
            }

            if let Some(sender) = sender_opt {
                if action_forward_selected {
                    let opcode_u8 = match state.edited_opcode {
                        WsOpcode::Text => 0x1,
                        WsOpcode::Binary => 0x2,
                        WsOpcode::Close => 0x8,
                        WsOpcode::Ping => 0x9,
                        WsOpcode::Pong => 0xA,
                        _ => 0x1,
                    };
                    let payload_bytes = match state.edited_opcode {
                        WsOpcode::Binary => {
                            let clean_hex: String = state.edited_payload.chars().filter(|c| c.is_ascii_hexdigit()).collect();
                            if clean_hex.len() % 2 == 0 && !clean_hex.is_empty() {
                                (0..clean_hex.len())
                                    .step_by(2)
                                    .map(|i| u8::from_str_radix(&clean_hex[i..i + 2], 16).unwrap_or(0))
                                    .collect()
                            } else {
                                state.edited_payload.as_bytes().to_vec()
                            }
                        }
                        _ => state.edited_payload.as_bytes().to_vec(),
                    };
                    let modified_frame = WsRawFrame {
                        fin: true,
                        opcode_u8,
                        masked: false,
                        mask_key: None,
                        payload: payload_bytes,
                    };
                    let _ = sender.send(Some(modified_frame));
                } else if action_drop_selected {
                    let _ = sender.send(None);
                }
            }
        }

        state.selected_frame_id = None;
    }
}
