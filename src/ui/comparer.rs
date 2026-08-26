use egui::{self, RichText, ScrollArea, TextStyle};
use crate::models::*;
use crate::theme::*;
use crate::ui::syntax;

pub fn render(ui: &mut egui::Ui, state: &mut ComparerState) {
    // ── Controls ──────────────────────────────────────────────────
    ui.horizontal(|ui| {
        ui.checkbox(&mut state.word_level, "Word-level diff");
        ui.checkbox(&mut state.sync_scroll, "Synchronized scroll");

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button(RichText::new("Clear All").size(11.0).color(ACCENT_RED)).clicked() {
                state.left_text.clear();
                state.right_text.clear();
            }
        });
    });

    ui.add_space(4.0);
    let available_h = ui.available_height() - 10.0;

    // ── Full-Height Side-by-side using columns ────────────────────
    ui.columns(2, |cols| {
        section_frame().show(&mut cols[0], |ui| {
            ui.horizontal(|ui| {
                let label = if state.left_label.is_empty() { "Item A (Original)" } else { &state.left_label };
                ui.label(RichText::new(label).size(12.0).color(TEXT_0).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new(format!("{} lines", state.left_text.lines().count())).size(11.0).color(TEXT_2));
                });
            });
            ui.separator();

            let mut layouter = |ui: &egui::Ui, string: &str, wrap_width: f32| {
                syntax::http_layouter(ui, string, wrap_width)
            };

            ScrollArea::vertical()
                .id_source("comparer_left_scroll")
                .min_scrolled_height(available_h - 32.0)
                .show(ui, |ui| {
                    ui.add_sized(
                        [ui.available_width(), available_h - 32.0],
                        egui::TextEdit::multiline(&mut state.left_text)
                            .font(TextStyle::Monospace)
                            .layouter(&mut layouter)
                    );
                });
        });

        section_frame().show(&mut cols[1], |ui| {
            ui.horizontal(|ui| {
                let label = if state.right_label.is_empty() { "Item B (Modified)" } else { &state.right_label };
                ui.label(RichText::new(label).size(12.0).color(TEXT_0).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new(format!("{} lines", state.right_text.lines().count())).size(11.0).color(TEXT_2));
                });
            });
            ui.separator();

            let mut layouter = |ui: &egui::Ui, string: &str, wrap_width: f32| {
                syntax::http_layouter(ui, string, wrap_width)
            };

            ScrollArea::vertical()
                .id_source("comparer_right_scroll")
                .min_scrolled_height(available_h - 32.0)
                .show(ui, |ui| {
                    ui.add_sized(
                        [ui.available_width(), available_h - 32.0],
                        egui::TextEdit::multiline(&mut state.right_text)
                            .font(TextStyle::Monospace)
                            .layouter(&mut layouter)
                    );
                });
        });
    });
}
