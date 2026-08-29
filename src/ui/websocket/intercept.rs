use egui::{self, RichText, Color32, Stroke, Rounding, FontFamily};
use crate::models::*;
use crate::theme::*;
use crate::proxy::store::{PENDING_WS_FRAMES, set_intercept_enabled};
use crate::proxy::websocket::protocol::WsRawFrame;

pub fn render(ui: &mut egui::Ui, state: &mut WsInterceptState) {
    ui.horizontal(|ui| {
        ui.label(RichText::new("WebSocket Interceptor").size(14.0).color(TEXT_0).strong());
        ui.add_space(20.0);

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
            set_intercept_enabled(state.enabled);

            // If turned OFF, flush any pending frames in queue so proxy threads unblock
            if !state.enabled {
                if let Some(pending) = state.active_pending.take() {
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
                    if let Ok(mut lock) = pending.responder.lock() {
                        if let Some(sender) = lock.take() {
                            let _ = sender.send(Some(default_frame));
                        }
                    }
                }

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
            }
        }
    });

    ui.add_space(8.0);
    ui.separator();
    ui.add_space(8.0);

    // Sync next pending frame from proxy store if no active frame currently loaded
    if state.active_pending.is_none() && state.enabled {
        if let Ok(mut lock) = PENDING_WS_FRAMES.lock() {
            if !lock.is_empty() {
                let pending = lock.remove(0);
                state.edited_payload = pending.payload.clone();
                state.edited_opcode = pending.opcode.clone();
                state.active_pending = Some(pending);
            }
        }
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

    let mut action_forward = false;
    let mut action_drop = false;

    if let Some(ref pending) = state.active_pending {
        let (dir_text, dir_color) = match pending.direction {
            WsDirection::ClientToServer => ("⬆️ Outbound Client Frame Paused", ACCENT_GREEN),
            WsDirection::ServerToClient => ("⬇️ Inbound Server Frame Paused", ACCENT_BLUE),
        };
        let connection_id = pending.connection_id;

        section_frame().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(dir_text).size(13.0).color(dir_color).strong());

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add(
                        egui::Button::new(RichText::new("▶️ Forward Frame").size(11.0).color(TEXT_0).strong())
                            .fill(ACCENT_BLUE)
                            .rounding(Rounding::same(4.0))
                    ).clicked() {
                        action_forward = true;
                    }
                    ui.add_space(6.0);
                    if ui.add(
                        egui::Button::new(RichText::new("✖ Drop Frame").size(11.0).color(ACCENT_RED).strong())
                            .fill(BG_RAISED)
                            .stroke(Stroke::new(1.0_f32, ACCENT_RED))
                            .rounding(Rounding::same(4.0))
                    ).clicked() {
                        action_drop = true;
                    }
                });
            });

            ui.add_space(6.0);
            ui.separator();
            ui.add_space(6.0);

            egui::Grid::new("ws_intercept_edit_grid")
                .num_columns(2)
                .spacing([16.0, 8.0])
                .show(ui, |ui| {
                    ui.label(RichText::new("Connection ID:").size(11.0).color(TEXT_1).strong());
                    ui.label(RichText::new(format!("WS #{}", connection_id)).size(11.0).color(TEXT_0).family(FontFamily::Monospace));
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

            ui.add_space(8.0);
            ui.label(RichText::new("Payload Editor:").size(11.0).color(TEXT_1).strong());
            ui.add_space(4.0);

            ui.add(
                egui::TextEdit::multiline(&mut state.edited_payload)
                    .font(egui::TextStyle::Monospace)
                    .desired_width(f32::INFINITY)
                    .desired_rows(10)
            );
        });
    } else {
        section_frame().show(ui, |ui| {
            ui.add_space(40.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new("Waiting for WebSocket frames...").size(13.0).color(ACCENT_CYAN).strong());
                ui.add_space(4.0);
                ui.label(RichText::new("No WebSocket frames are currently paused in the queue.").size(11.0).color(TEXT_2));
            });
            ui.add_space(40.0);
        });
    }

    if action_forward || action_drop {
        if let Some(pending) = state.active_pending.take() {
            let responder_mutex = pending.responder.clone();
            let mut sender_opt = None;
            if let Ok(mut lock) = responder_mutex.lock() {
                sender_opt = lock.take();
            }

            if let Some(sender) = sender_opt {
                if action_forward {
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
                } else if action_drop {
                    let _ = sender.send(None);
                }
            }
        }
    }
}
