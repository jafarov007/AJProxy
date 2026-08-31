use eframe::egui::{self, TextStyle};
use crate::models::HttpEntry;
use crate::ui::syntax;

/// Raw Burp Suite Style Inspector Renderer (With Syntax Highlighting)
pub fn render_inspector_section(ui: &mut egui::Ui, entry: &HttpEntry, is_request: bool) {
    ui.vertical(|ui| {
        let raw_text = if is_request {
            let mut text = String::new();
            if !entry.request_headers.starts_with(&entry.method) {
                text.push_str(&format!("{} {} HTTP/1.1\r\n", entry.method, entry.path));
            }
            text.push_str(&entry.request_headers);
            if !text.ends_with("\r\n\r\n") && !text.ends_with("\n\n") {
                text.push_str("\r\n\r\n");
            }
            text.push_str(&entry.request_body);
            text
        } else {
            let mut text = String::new();
            text.push_str(&entry.response_headers);
            if !text.ends_with("\r\n\r\n") && !text.ends_with("\n\n") {
                text.push_str("\r\n\r\n");
            }
            text.push_str(&entry.response_body);
            text
        };

        let mut display_str = raw_text.as_str();
        let avail_w = ui.available_width();
        let mut layouter = move |ui: &egui::Ui, string: &str, _wrap_width: f32| {
            syntax::http_layouter(ui, string, avail_w)
        };

        egui::ScrollArea::vertical()
            .id_source(if is_request { "http_req_inspector_scroll" } else { "http_resp_inspector_scroll" })
            .show(ui, |ui| {
                ui.add(
                    egui::TextEdit::multiline(&mut display_str)
                        .font(TextStyle::Monospace)
                        .layouter(&mut layouter)
                        .desired_width(f32::INFINITY)
                        .desired_rows(18)
                );
            });
    });
}
