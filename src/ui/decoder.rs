use egui::{self, Color32, RichText, Rounding, ScrollArea, Stroke, TextStyle};
use crate::models::*;
use crate::theme::*;
use crate::codec;

pub fn render(ui: &mut egui::Ui, state: &mut DecoderState) {
    ui.vertical(|ui| {
        // ── Top Bar: Algorithm Selectors & Action Buttons ────────────────
        ui.horizontal(|ui| {
            ui.label(RichText::new("Codec:").size(11.0).color(TEXT_2).strong());
            ui.add_space(4.0);

            for (enc, label) in &[
                (EncodingType::Base64, "Base64"),
                (EncodingType::URL, "URL"),
                (EncodingType::HTML, "HTML"),
                (EncodingType::Hex, "Hex"),
                (EncodingType::JWT, "JWT"),
                (EncodingType::MD5, "MD5"),
                (EncodingType::SHA1, "SHA-1"),
                (EncodingType::SHA256, "SHA-256"),
                (EncodingType::SHA512, "SHA-512"),
            ] {
                let active = state.encoding == *enc;
                let color = if active { TEXT_0 } else { TEXT_2 };
                let bg = if active { BG_OVERLAY } else { Color32::TRANSPARENT };
                let stroke = if active { Stroke::new(1.0_f32, ACCENT_BLUE) } else { Stroke::NONE };

                if ui.add(
                    egui::Button::new(RichText::new(*label).size(11.0).color(color).strong())
                        .fill(bg)
                        .rounding(Rounding::same(3.0))
                        .stroke(stroke)
                ).clicked() {
                    state.encoding = enc.clone();
                }
            }

            ui.add_space(16.0);

            // ── Primary Action Buttons: Encode & Decode ──────────────────
            if ui.add(
                egui::Button::new(RichText::new("🔒 Encode").size(11.0).color(ACCENT_BLUE).strong())
                    .fill(Color32::from_rgb(20, 35, 55))
                    .stroke(Stroke::new(1.0_f32, ACCENT_BLUE))
                    .rounding(Rounding::same(4.0))
            ).on_hover_text("Encode input using selected algorithm").clicked() {
                match codec::encode(&state.encoding, &state.input) {
                    Ok(res) => {
                        state.output = res;
                        state.error_msg.clear();
                    }
                    Err(err) => {
                        state.error_msg = err;
                    }
                }
            }

            ui.add_space(4.0);

            let is_hash = matches!(state.encoding, EncodingType::MD5 | EncodingType::SHA1 | EncodingType::SHA256 | EncodingType::SHA512);
            let decode_color = if is_hash { TEXT_2 } else { ACCENT_GREEN };
            let decode_btn = egui::Button::new(RichText::new("🔓 Decode").size(11.0).color(decode_color).strong())
                .fill(if is_hash { Color32::from_rgb(30, 30, 35) } else { Color32::from_rgb(15, 45, 30) })
                .stroke(Stroke::new(1.0_f32, decode_color))
                .rounding(Rounding::same(4.0));

            if ui.add_enabled(!is_hash, decode_btn)
                .on_hover_text(if is_hash { "Cryptographic hashes cannot be decoded" } else { "Decode input using selected algorithm" })
                .clicked()
            {
                match codec::decode(&state.encoding, &state.input) {
                    Ok(res) => {
                        state.output = res;
                        state.error_msg.clear();
                    }
                    Err(err) => {
                        state.error_msg = err;
                    }
                }
            }

            ui.add_space(16.0);

            // ── Utility Action Buttons ──────────────────────────────────
            if ui.add(
                egui::Button::new(RichText::new("↔ Swap").size(10.0).color(TEXT_1))
                    .fill(BG_RAISED)
                    .rounding(Rounding::same(3.0))
            ).on_hover_text("Swap Input and Output text").clicked() {
                std::mem::swap(&mut state.input, &mut state.output);
                state.error_msg.clear();
            }

            ui.add_space(4.0);

            if ui.add(
                egui::Button::new(RichText::new("📋 Copy Output").size(10.0).color(ACCENT_CYAN))
                    .fill(BG_RAISED)
                    .rounding(Rounding::same(3.0))
            ).on_hover_text("Copy Output to system clipboard").clicked() {
                ui.output_mut(|o| o.copied_text = state.output.clone());
            }

            ui.add_space(4.0);

            if ui.add(
                egui::Button::new(RichText::new("🧹 Clear").size(10.0).color(ACCENT_RED))
                    .fill(BG_RAISED)
                    .rounding(Rounding::same(3.0))
            ).on_hover_text("Clear Input and Output fields").clicked() {
                state.input.clear();
                state.output.clear();
                state.error_msg.clear();
            }
        });

        // ── Error Banner (if error occurred) ─────────────────────────────
        if !state.error_msg.is_empty() {
            ui.add_space(4.0);
            egui::Frame::none()
                .fill(Color32::from_rgb(50, 15, 20))
                .stroke(Stroke::new(1.0_f32, ACCENT_RED))
                .rounding(Rounding::same(4.0))
                .inner_margin(egui::Margin::symmetric(8.0, 4.0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("⚠️").size(12.0));
                        ui.label(RichText::new(&state.error_msg).size(11.0).color(ACCENT_RED).strong());
                    });
                });
        }

        ui.add_space(6.0);

        // ── Dual Column Input & Output Text Editors ───────────────────────
        ui.columns(2, |cols| {
            section_frame().show(&mut cols[0], |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Input Payload / String").size(11.0).color(TEXT_0).strong());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(RichText::new(format!("{} chars", state.input.len())).size(10.0).color(TEXT_2));
                    });
                });
                ui.separator();
                ScrollArea::both()
                    .id_source("decoder_input_scroll")
                    .show(ui, |ui| {
                        ui.add(
                            egui::TextEdit::multiline(&mut state.input)
                                .font(TextStyle::Monospace)
                                .code_editor()
                                .desired_width(f32::INFINITY)
                                .desired_rows(22)
                        );
                    });
            });

            section_frame().show(&mut cols[1], |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Output / Result").size(11.0).color(TEXT_0).strong());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(RichText::new(format!("{} chars", state.output.len())).size(10.0).color(TEXT_2));
                    });
                });
                ui.separator();
                ScrollArea::both()
                    .id_source("decoder_output_scroll")
                    .show(ui, |ui| {
                        ui.add(
                            egui::TextEdit::multiline(&mut state.output)
                                .font(TextStyle::Monospace)
                                .code_editor()
                                .desired_width(f32::INFINITY)
                                .desired_rows(22)
                        );
                    });
            });
        });
    });
}
