use egui::{self, RichText, Rounding, Stroke, Color32, ScrollArea, TextStyle};
use crate::models::*;
use crate::theme::*;

pub fn render(ui: &mut egui::Ui, state: &mut DecoderState) {
    // ── Codec toolbar ─────────────────────────────────────────────
    ui.horizontal(|ui| {
        ui.label(RichText::new("Codec").size(10.0).color(TEXT_2));
        for (enc, label) in &[
            (EncodingType::Base64, "Base64"),
            (EncodingType::URL, "URL"),
            (EncodingType::HTML, "HTML"),
            (EncodingType::Hex, "Hex"),
            (EncodingType::JWT, "JWT"),
        ] {
            let active = state.encoding == *enc;
            let color = if active { TEXT_0 } else { TEXT_2 };
            if ui.add(
                egui::Button::new(RichText::new(*label).size(10.0).color(color))
                    .fill(if active { BG_OVERLAY } else { Color32::TRANSPARENT })
                    .rounding(Rounding::same(2.0))
                    .stroke(if active { Stroke::new(1.0_f32, ACCENT_BLUE) } else { Stroke::NONE })
            ).clicked() {
                state.encoding = enc.clone();
            }
        }

        ui.add_space(16.0);
        for (dir, label) in &[
            (TransformDirection::Encode, "Encode"),
            (TransformDirection::Decode, "Decode"),
        ] {
            let active = state.direction == *dir;
            let color = if active { ACCENT_BLUE } else { TEXT_2 };
            if ui.add(
                egui::Button::new(RichText::new(*label).size(10.0).color(color))
                    .fill(Color32::TRANSPARENT)
                    .stroke(Stroke::NONE)
            ).clicked() {
                state.direction = dir.clone();
            }
        }
    });

    ui.add_space(4.0);

    // ── Input / Output split ──────────────────────────────────────
    ui.columns(2, |cols| {
        section_frame().show(&mut cols[0], |ui| {
            ui.label(RichText::new("Input").size(11.0).color(TEXT_0).strong());
            ui.separator();
            ScrollArea::vertical()
                .id_source("decoder_input_scroll")
                .show(ui, |ui| {
                    ui.add(
                        egui::TextEdit::multiline(&mut state.input)
                            .font(TextStyle::Monospace)
                            .code_editor()
                            .desired_width(f32::INFINITY)
                    );
                });
        });

        section_frame().show(&mut cols[1], |ui| {
            ui.label(RichText::new("Output").size(11.0).color(TEXT_0).strong());
            ui.separator();
            ScrollArea::vertical()
                .id_source("decoder_output_scroll")
                .show(ui, |ui| {
                    ui.add(
                        egui::TextEdit::multiline(&mut state.output)
                            .font(TextStyle::Monospace)
                            .code_editor()
                            .desired_width(f32::INFINITY)
                    );
                });
        });
    });
}
