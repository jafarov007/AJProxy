use egui::{self, RichText};
use crate::theme::*;

pub fn render(ui: &mut egui::Ui, proxy_running: bool, total_requests: usize, intercepted: usize) {
    ui.horizontal(|ui| {
        ui.add_space(8.0);

        let (dot, color) = if proxy_running {
            ("\u{25CF}", ACCENT_GREEN)
        } else {
            ("\u{25CB}", TEXT_2)
        };
        ui.label(RichText::new(dot).size(10.0).color(color));
        ui.label(RichText::new(format!("Requests: {}", total_requests)).size(10.0).color(TEXT_1));

        if intercepted > 0 {
            ui.label(RichText::new(format!("Queued: {}", intercepted)).size(10.0).color(ACCENT_AMBER));
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(8.0);
            ui.label(RichText::new("AJProxy v0.1.0").size(10.0).color(TEXT_2));
        });
    });
}
