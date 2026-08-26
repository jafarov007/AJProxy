use egui::{self, RichText, Rounding, Stroke};
use crate::models::*;
use crate::theme::*;

#[allow(dead_code)]
pub fn render(ui: &mut egui::Ui, filter: &mut FilterState) {
    ui.horizontal(|ui| {
        ui.label(RichText::new("Filter:").size(12.0).color(TEXT_1).strong());
        ui.add(
            egui::TextEdit::singleline(&mut filter.search_query)
                .hint_text("Filter URL, host, or query...")
                .desired_width(220.0)
        );

        ui.add_space(14.0);

        // Method Filter Buttons - High Contrast Active State
        ui.label(RichText::new("Method:").size(12.0).color(TEXT_1).strong());
        for method in &["ALL", "GET", "POST", "PUT", "DELETE"] {
            let active = filter.filter_method == *method;

            let (text_color, bg_color, stroke) = if active {
                let col = if *method == "ALL" { ACCENT_BLUE } else { method_color(method) };
                (TEXT_0, color_alpha(col, 70), Stroke::new(1.5_f32, col))
            } else {
                (TEXT_2, BG_RAISED, Stroke::new(0.5_f32, BORDER))
            };

            if ui.add(
                egui::Button::new(RichText::new(*method).size(11.0).color(text_color).strong())
                    .fill(bg_color)
                    .rounding(Rounding::same(3.0))
                    .stroke(stroke)
            ).clicked() {
                filter.filter_method = method.to_string();
            }
        }

        ui.add_space(14.0);

        // Status Code Filter Buttons
        ui.label(RichText::new("Status:").size(12.0).color(TEXT_1).strong());
        for s in &["ALL", "2xx", "3xx", "4xx", "5xx"] {
            let active = filter.filter_status == *s;

            let (text_color, bg_color, stroke) = if active {
                (TEXT_0, color_alpha(ACCENT_BLUE, 70), Stroke::new(1.5_f32, ACCENT_BLUE))
            } else {
                (TEXT_2, BG_RAISED, Stroke::new(0.5_f32, BORDER))
            };

            if ui.add(
                egui::Button::new(RichText::new(*s).size(11.0).color(text_color).strong())
                    .fill(bg_color)
                    .rounding(Rounding::same(3.0))
                    .stroke(stroke)
            ).clicked() {
                filter.filter_status = s.to_string();
            }
        }
    });
}
