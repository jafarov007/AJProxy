use egui::{self, RichText, Rounding, Stroke, Color32, FontFamily, include_image};
use crate::models::*;
use crate::theme::*;

pub fn render(
    ui: &mut egui::Ui,
    active_tab: &mut Tab,
    proxy_running: &mut bool,
    listen_addr: &str,
    listen_port: u16,
) {
    ui.horizontal_centered(|ui| {
        ui.add_space(6.0);

        // Logo
        ui.add(
            egui::Image::new(include_image!("../../logo.png"))
                .fit_to_exact_size(egui::Vec2::new(22.0, 22.0))
                .rounding(Rounding::same(4.0))
        );
        ui.add_space(4.0);
        ui.label(
            RichText::new("AJProxy")
                .size(14.0)
                .color(TEXT_0)
                .strong()
                .family(FontFamily::Monospace),
        );

        ui.add_space(4.0);

        // Globe button for launching internal WebKitGTK/WebView2 proxied browser
        let globe_btn = ui.add(
            egui::Button::new(
                RichText::new("🌐 Browser")
                    .size(11.0)
                    .color(TEXT_0)
                    .strong()
            )
            .fill(BG_RAISED)
            .stroke(Stroke::new(1.0_f32, ACCENT_BLUE))
            .rounding(Rounding::same(12.0))
        );

        if globe_btn.clicked() {
            if let Err(e) = crate::browser::webview::launch_embedded_browser(listen_port, Some("https://jafarov007.github.io/")) {
                eprintln!("[AJProxy] Internal browser launch error: {}", e);
            }
        }

        if globe_btn.hovered() {
            egui::show_tooltip_text(
                ui.ctx(),
                globe_btn.id,
                format!("Launch embedded native browser (WebKitGTK/WebView2) proxied via 127.0.0.1:{}", listen_port),
            );
        }

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(6.0);

        // Navigation Tabs
        let tabs = [
            (Tab::Dashboard, "Dashboard"),
            (Tab::Traffic, "HTTP History"),
            (Tab::Intercept, "Intercept"),
            (Tab::Repeater, "Repeater"),
            (Tab::BruteForce, "Intruder"),
            (Tab::Decoder, "Decoder"),
            (Tab::Comparer, "Comparer"),
            (Tab::SiteMap, "Target Map"),
            (Tab::Modules, "Extensions"),
            (Tab::Settings, "Settings"),
        ];

        for (tab, label) in &tabs {
            let active = *active_tab == *tab;
            let text_color = if active { TEXT_0 } else { TEXT_2 };
            let bg_color = if active { BG_RAISED } else { Color32::TRANSPARENT };

            let r = ui.add(
                egui::Button::new(
                    RichText::new(*label).size(12.0).color(text_color).strong(),
                )
                .fill(bg_color)
                .rounding(Rounding::same(3.0))
                .stroke(if active { Stroke::new(1.0_f32, BORDER) } else { Stroke::NONE })
            );

            if r.clicked() {
                *active_tab = *tab;
            }
        }

        // Status Indicator & Window Controls
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(6.0);

            // Window Action Buttons (Explicit sequential order: Close -> Maximize -> Minimize)
            // Close Button
            if ui.add(
                egui::Button::new(
                    RichText::new("✖")
                        .size(13.0)
                        .color(ACCENT_RED)
                        .strong()
                )
                .fill(Color32::TRANSPARENT)
                .stroke(Stroke::NONE)
            ).clicked() {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            }

            ui.add_space(2.0);

            // Maximize / Restore Button
            if ui.add(
                egui::Button::new(
                    RichText::new("🗖")
                        .size(13.0)
                        .color(TEXT_1)
                        .strong()
                )
                .fill(Color32::TRANSPARENT)
                .stroke(Stroke::NONE)
            ).clicked() {
                let is_maximized = ui.ctx().input(|i| i.viewport().maximized.unwrap_or(false));
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Maximized(!is_maximized));
            }

            ui.add_space(2.0);

            // Minimize Button
            if ui.add(
                egui::Button::new(
                    RichText::new("🗕")
                        .size(13.0)
                        .color(TEXT_1)
                        .strong()
                )
                .fill(Color32::TRANSPARENT)
                .stroke(Stroke::NONE)
            ).clicked() {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Minimized(true));
            }

            ui.add_space(10.0);
            ui.separator();
            ui.add_space(10.0);

            // Proxy Status Pill
            let (status_text, status_bg, status_fg, status_stroke) = if *proxy_running {
                (
                    format!("RUNNING ({}:{})", listen_addr, listen_port),
                    Color32::from_rgb(10, 38, 22),
                    Color32::from_rgb(74, 222, 128),
                    Stroke::new(1.0_f32, Color32::from_rgb(34, 197, 94)),
                )
            } else {
                (
                    format!("STOPPED ({}:{})", listen_addr, listen_port),
                    BG_RAISED,
                    TEXT_2,
                    Stroke::new(1.0_f32, BORDER_DIM),
                )
            };

            let toggle_btn = ui.add(
                egui::Button::new(
                    RichText::new(status_text)
                        .size(11.0)
                        .color(status_fg)
                        .strong()
                        .family(FontFamily::Monospace),
                )
                .fill(status_bg)
                .stroke(status_stroke)
                .rounding(Rounding::same(10.0))
            );

            if toggle_btn.clicked() {
                *proxy_running = !*proxy_running;
            }

            if toggle_btn.hovered() {
                egui::show_tooltip_text(
                    ui.ctx(),
                    toggle_btn.id,
                    if *proxy_running { "Click to stop HTTP/HTTPS listener" } else { "Click to start HTTP/HTTPS listener" },
                );
            }
        });
    });
}
