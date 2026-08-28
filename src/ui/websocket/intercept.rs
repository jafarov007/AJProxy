use egui::{self, RichText, Color32, Stroke, Rounding, FontFamily};
use crate::models::*;
use crate::theme::*;

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
        }
    });

    ui.add_space(8.0);
    ui.separator();
    ui.add_space(8.0);

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

    // Intercept IS ON!
    let mut clear_frame = false;

    if let Some(ref frame) = state.current_frame {
        let (dir_text, dir_color) = match frame.direction {
            WsDirection::ClientToServer => ("⬆️ Outbound Client Frame Paused", ACCENT_GREEN),
            WsDirection::ServerToClient => ("⬇️ Inbound Server Frame Paused", ACCENT_BLUE),
        };
        let connection_id = frame.connection_id;

        section_frame().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(dir_text).size(13.0).color(dir_color).strong());

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add(
                        egui::Button::new(RichText::new("▶️ Forward Frame").size(11.0).color(TEXT_0).strong())
                            .fill(ACCENT_BLUE)
                            .rounding(Rounding::same(4.0))
                    ).clicked() {
                        clear_frame = true;
                    }
                    ui.add_space(6.0);
                    if ui.add(
                        egui::Button::new(RichText::new("✖ Drop Frame").size(11.0).color(ACCENT_RED).strong())
                            .fill(BG_RAISED)
                            .stroke(Stroke::new(1.0_f32, ACCENT_RED))
                            .rounding(Rounding::same(4.0))
                    ).clicked() {
                        clear_frame = true;
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

    if clear_frame {
        state.current_frame = None;
    }
}
