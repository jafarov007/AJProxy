use egui::{self, Color32, RichText, Rounding, ScrollArea, Stroke, TextStyle};
use crate::models::*;
use crate::theme::*;
use crate::comparer::{self, DiffItem};
use similar::ChangeTag;

pub fn render(ui: &mut egui::Ui, state: &mut ComparerState) {
    ui.vertical(|ui| {
        // ── Top Bar: Mode Selectors & Controls ─────────────────────────
        ui.horizontal(|ui| {
            ui.label(RichText::new("Diff Mode:").size(11.0).color(TEXT_2).strong());
            ui.add_space(4.0);

            for (mode, label) in &[
                (DiffMode::Words, "Words"),
                (DiffMode::Lines, "Lines"),
                (DiffMode::Bytes, "Bytes"),
            ] {
                let active = state.diff_mode == *mode;
                let color = if active { TEXT_0 } else { TEXT_2 };
                let bg = if active { BG_OVERLAY } else { Color32::TRANSPARENT };
                let stroke = if active { Stroke::new(1.0_f32, ACCENT_BLUE) } else { Stroke::NONE };

                if ui.add(
                    egui::Button::new(RichText::new(*label).size(11.0).color(color).strong())
                        .fill(bg)
                        .rounding(Rounding::same(3.0))
                        .stroke(stroke)
                ).clicked() {
                    state.diff_mode = mode.clone();
                }
            }

            ui.add_space(16.0);

            let is_ic = state.ignore_case;
            ui.checkbox(
                &mut state.ignore_case,
                RichText::new("Ignore Case").size(10.0).color(if is_ic { ACCENT_CYAN } else { TEXT_2 })
            ).on_hover_text("Ignore uppercase / lowercase differences");

            ui.add_space(6.0);

            let is_iw = state.ignore_whitespace;
            ui.checkbox(
                &mut state.ignore_whitespace,
                RichText::new("Ignore Whitespace").size(10.0).color(if is_iw { ACCENT_CYAN } else { TEXT_2 })
            ).on_hover_text("Ignore extra spaces and newlines");

            ui.add_space(16.0);

            if ui.add(
                egui::Button::new(RichText::new("Swap A / B").size(10.0).color(TEXT_1))
                    .fill(BG_RAISED)
                    .rounding(Rounding::same(3.0))
            ).on_hover_text("Swap Item A and Item B text").clicked() {
                std::mem::swap(&mut state.left_text, &mut state.right_text);
            }

            ui.add_space(4.0);

            if ui.add(
                egui::Button::new(RichText::new("Clear").size(10.0).color(ACCENT_RED))
                    .fill(BG_RAISED)
                    .rounding(Rounding::same(3.0))
            ).on_hover_text("Clear Item A and Item B").clicked() {
                state.left_text.clear();
                state.right_text.clear();
            }
        });

        ui.add_space(4.0);

        // ── Compute Diff & Render Summary Bar ─────────────────────────────
        let diff_res = comparer::compute_diff(state);

        ui.horizontal(|ui| {
            ui.label(RichText::new("Summary:").size(10.0).color(TEXT_2));
            ui.add_space(6.0);

            ui.label(
                RichText::new(format!("+ Added: {}", diff_res.added_count))
                    .size(10.0)
                    .color(ACCENT_GREEN)
                    .strong()
            );
            ui.add_space(10.0);

            ui.label(
                RichText::new(format!("- Deleted: {}", diff_res.deleted_count))
                    .size(10.0)
                    .color(ACCENT_RED)
                    .strong()
            );
            ui.add_space(12.0);

            ui.label(
                RichText::new(format!("Similarity: {:.1}%", diff_res.match_percentage))
                    .size(10.0)
                    .color(ACCENT_BLUE)
                    .strong()
            );
        });

        ui.add_space(6.0);

        // ── Full-Height Responsive Dual Panel Layout ──────────────────────
        let avail_h = ui.available_height();
        let editor_h = (avail_h * 0.40).max(100.0);
        let diff_h = (avail_h * 0.55).max(140.0);

        ui.columns(2, |cols| {
            // ── Left Column: Item A (Base / Original) ─────────────────────
            section_frame().show(&mut cols[0], |ui| {
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Item A (Base / Original)").size(11.0).color(TEXT_0).strong());
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(RichText::new(format!("{} chars", state.left_text.len())).size(10.0).color(TEXT_2));
                        });
                    });
                    ui.separator();

                    // Directly editable text box
                    ScrollArea::both()
                        .id_source("comparer_left_editor_scroll")
                        .max_height(editor_h)
                        .show(ui, |ui| {
                            ui.add_sized(
                                [ui.available_width(), editor_h - 10.0],
                                egui::TextEdit::multiline(&mut state.left_text)
                                    .hint_text("Paste or type Item A text here...")
                                    .font(TextStyle::Monospace)
                            );
                        });

                    ui.add_space(6.0);
                    ui.label(RichText::new("Diff Highlights (Item A):").size(10.0).color(TEXT_2).strong());
                    ui.separator();

                    // Burp Suite style formatted Diff View
                    ScrollArea::both()
                        .id_source("comparer_left_diff_scroll")
                        .max_height(diff_h)
                        .show(ui, |ui| {
                            render_diff_view(ui, &diff_res.left_items, true);
                        });
                });
            });

            // ── Right Column: Item B (Modified / Target) ──────────────────
            section_frame().show(&mut cols[1], |ui| {
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Item B (Modified / Target)").size(11.0).color(TEXT_0).strong());
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(RichText::new(format!("{} chars", state.right_text.len())).size(10.0).color(TEXT_2));
                        });
                    });
                    ui.separator();

                    // Directly editable text box
                    ScrollArea::both()
                        .id_source("comparer_right_editor_scroll")
                        .max_height(editor_h)
                        .show(ui, |ui| {
                            ui.add_sized(
                                [ui.available_width(), editor_h - 10.0],
                                egui::TextEdit::multiline(&mut state.right_text)
                                    .hint_text("Paste or type Item B text here...")
                                    .font(TextStyle::Monospace)
                            );
                        });

                    ui.add_space(6.0);
                    ui.label(RichText::new("Diff Highlights (Item B):").size(10.0).color(TEXT_2).strong());
                    ui.separator();

                    // Burp Suite style formatted Diff View
                    ScrollArea::both()
                        .id_source("comparer_right_diff_scroll")
                        .max_height(diff_h)
                        .show(ui, |ui| {
                            render_diff_view(ui, &diff_res.right_items, false);
                        });
                });
            });
        });
    });
}

fn render_diff_view(ui: &mut egui::Ui, items: &[DiffItem], is_left: bool) {
    if items.is_empty() {
        ui.label(RichText::new("No text to compare").size(10.0).color(TEXT_2));
        return;
    }

    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(0.0, 2.0);

        for item in items {
            let (bg_color, text_color) = match item.tag {
                ChangeTag::Delete if is_left => (Color32::from_rgb(80, 20, 30), ACCENT_RED),
                ChangeTag::Insert if !is_left => (Color32::from_rgb(20, 70, 35), ACCENT_GREEN),
                ChangeTag::Equal => (Color32::TRANSPARENT, TEXT_1),
                _ => (Color32::TRANSPARENT, TEXT_1),
            };

            if bg_color != Color32::TRANSPARENT {
                egui::Frame::none()
                    .fill(bg_color)
                    .rounding(Rounding::same(2.0))
                    .inner_margin(egui::Margin::symmetric(3.0, 1.0))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(&item.text)
                                    .font(TextStyle::Monospace.resolve(ui.style()))
                                    .size(11.0)
                                    .color(text_color)
                            );
                        });
                    });
            } else {
                ui.label(
                    RichText::new(&item.text)
                        .font(TextStyle::Monospace.resolve(ui.style()))
                        .size(11.0)
                        .color(text_color)
                );
            }
        }
    });
}
